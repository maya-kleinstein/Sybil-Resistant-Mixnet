use crate::config::*;
use crate::marshal::info::{get_init_packets, get_network_info};
use crate::marshal::logs::RESULTS;
use crate::marshal::SHUTDOWN_FILE;
use crate::network::{decrypt_setup_layer, verify_setup_packet, Connections, Network, Packet, SetupPacket};
use futures::future::join_all;
use log::*;
use mix_service::mix_client::MixClient;
use mix_service::mix_server::{Mix, MixServer};
use mix_service::{PacketRequest, PacketResponse, GetRequest, GetResponse};
use mixnet_request::{MixnetPacketType, MixnetPacket};
use rand::seq::SliceRandom;
use rand::thread_rng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

pub mod mixnet_request;

/// Service created from proto file
pub mod mix_service {
    tonic::include_proto!("mix");
}

// TODO: add typedef for vec<u8> raw_packet

#[derive(Debug)]
/// Mix server struct
pub struct MyServer {
    id: u16,
    mix_ips: Vec<IpAddr>,
    // Maps between layer to (layer output buffer, layer add request counter)
    output_buffer: Mutex<HashMap<u32, (Vec<Vec<MixnetPacketType>>, u32)>>,
    // Maps between mix id to connection
    channels: Arc<Mutex<HashMap<u32, MixClient<Channel>>>>,
    network_info: Network,
    connections: Arc<RwLock<Connections>>,
    notify: Arc<Semaphore>,
    time: Mutex<Instant>,
}

impl MyServer {
    /// Create instance of MyServer
    pub fn new(mix_ips: Vec<IpAddr>, id: u16) -> Self {
        let network_info = get_network_info();
        let network_size = network_info.size;
        return MyServer {
            id,
            mix_ips,
            output_buffer: Mutex::new(HashMap::new()),
            channels: Arc::new(Mutex::new(HashMap::new())),
            network_info,
            connections: Arc::new(RwLock::new(Connections::new(network_size))),
            notify: Arc::new(Semaphore::new(0)),
            time: Mutex::new(Instant::now()),
        };
    }
}

#[tonic::async_trait]
impl Mix for MyServer {
    async fn setup(&self, request: Request<PacketRequest>) -> Result<Response<PacketResponse>, Status> {
        self.parse_input::<SetupPacket>(request).await;

        self.handle_middle_layer::<SetupPacket>().await;

        // Notify config
        self.notify.add_permits(1);
        let reply = PacketResponse {};
        Ok(Response::new(reply))
    }

    async fn add(&self, request: Request<PacketRequest>) -> Result<Response<PacketResponse>, Status> {
        self.parse_input::<Vec<u8>>(request).await;

        self.handle_middle_layer::<Vec<u8>>().await;

        // Notify config
        self.notify.add_permits(1);
        let reply = PacketResponse {};
        Ok(Response::new(reply))
    }

    async fn get(&self, _request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        info!("mix {} got a get request", self.id);
        let mut messages = Vec::new();
        for round in 0..*NUM_ROUNDS {
            // Wait til the mix is done getting all add requests for this round
            let amount_to_acquire = (*NUM_MIXES as u32) * ((*NUM_LAYERS - 1) as u32);
            let _ = self
                .notify
                .acquire_many(amount_to_acquire)
                .await
                .unwrap()
                .forget();

            messages = self.output_last_layer(round).await;

            // Measure time for this round
            self.measure_time(round).await;

            if round < *NUM_ROUNDS - 1 {
                // Run next round
                info!("mix {} is starting round {}", self.id, round + 1);
                start_mix_round(&self.mix_ips, self.id).await;
            }
        }
        // Note: messages should be the same for each round so it doesn't matter which one we use
        let reply = GetResponse { messages };
        Ok(Response::new(reply))
    }
}

impl MyServer {
    /// Process (decrypt) incoming setup stream to proper output buffer
    async fn parse_input<T: MixnetPacket>(&self, request: Request<PacketRequest>) -> () {
        let req = request.into_inner();
        info!(
            "mix {} got {} packets FROM layer {}",
            self.id,
            req.packets.len(),
            req.layer
        );
        // Decrypt packets - Verify as well in case of MixnetVerification::Verify
        let dec_packets = T::decrypt_incoming_packets(
                req.packets, 
                self.id as u64, 
                &self.network_info, 
                &self.connections,
                req.layer as u64
            ).await;

        let mut buffer_guard = self.output_buffer.lock().await;
        // Update counter
        (*buffer_guard)
            .entry(req.layer + 1)
            .or_insert((vec![vec![].into(); Into::<usize>::into(*NUM_MIXES)], 0))
            .1 += 1;

        // Insert decrypted packets to output_buffer
        for (dec_packet, next) in dec_packets {
            (*buffer_guard).entry(req.layer + 1).and_modify(|e| {
                e.0[next as usize].push(dec_packet);
            });
        }
    }

    /// Send setup from current layer to all mixes
    async fn handle_middle_layer<T: MixnetPacket>(&self) -> () {
        let mut mix_tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));

        let mut guard = self.output_buffer.lock().await;
        // Check if it is a middle layer and that all packets for this layer have been recv'd
        if !is_middle_layer(&*guard) {
            return;
        }
        // Start timer when last packet of first layer recv'd
        if (*guard).keys().min().unwrap().clone() == *FIRST_MIDDLE_LAYER {
            let mut time_guard = self.time.lock().await;
            *time_guard = Instant::now();
        }
        // Send to all mixes
        let layer = (*guard).keys().min().expect("HashMap is Empty").clone();
        let mut output_buffer = (*guard).remove(&layer).unwrap();
        drop(guard);

        // For edge case mixnet verification
        let (edge_case_index, total_outgoing) = get_edge_case_info(&output_buffer.0);

        for i in (0..*NUM_MIXES).rev() {
            let mut packets = output_buffer.0.pop().unwrap();
            info!(
                "mix {} sent to mix {} {} packets FROM layer {}",
                self.id,
                i,
                packets.len(),
                layer
            );
            // Verify packets if needed - AKA if packet type is SetupPacket
            T::handle_verify_on_output(
                    &mut packets,
                    &self.network_info,
                    layer,
                    self.id,
                    Some(i),
                    edge_case_index,
                    total_outgoing
            );
            packets.shuffle(&mut thread_rng());
            let mut packets_data = Vec::new();
            while !packets.is_empty() {
                packets_data.push(packets.pop().unwrap().data);
            }
            let task = self.send_to_mix(i, layer, packets_data);
            mix_tasks.push(task);
        }
        join_all(mix_tasks).await;
    }

    async fn send_to_mix(&self, dst: u16, layer: u32, packets: Vec<Vec<u8>>) -> () {
        let packet_request = PacketRequest {
            packets: packets,
            layer: layer,
        };
        let mut channels_guard = self.channels.lock().await;
        let mut channel = (*channels_guard)
            .entry(dst.into())
            .or_insert(
                MixClient::connect(format!(
                    "http://{}:{}",
                    self.mix_ips[dst as usize],
                    *BASE_PORT + dst
                ))
                .await
                .unwrap(),
            )
            .clone();
        drop(channels_guard);
        channel
            .add(Request::new(packet_request))
            .await
            .expect("Failed to send add");
    }

    /// Process all layers into single output message
    async fn output_last_layer(&self, round: u32) -> Vec<Vec<u8>> {
        let buffer_guard = self.output_buffer.lock().await;
        let send_layer = (*buffer_guard).keys().min().expect("HashMap is Empty");
        let mut packets = (0..*NUM_MIXES)
            .map(|i| (*buffer_guard).get(send_layer).unwrap().0[i as usize].to_vec())
            .flatten()
            .collect::<Vec<MixnetPacketType>>();
        if round == 1 { // This is the setup packet round
            SetupPacket::handle_verify_on_output(
                &mut packets,
                &self.network_info,
                send_layer.clone(),
                self.id,
                None,
                0,
                0,
            );
        }
        let mut messages = Vec::new();
        while !packets.is_empty() {
            messages.push(packets.pop().unwrap().data);
        }
        // Shuffle the mix output
        messages.shuffle(&mut thread_rng());
        return messages;
    }

    async fn measure_time(&self, round: u32) {
        let time_guard = self.time.lock().await;
        info!(
            " {} Round {} took mix {} {:?} seconds",
            RESULTS,
            round,
            self.id,
            time_guard.elapsed()
        );
        drop(time_guard);
    }
}

async fn connect_and_send(dst_ip: IpAddr, dst: u16, src: u16, packets: Vec<Vec<u8>>, layer: u32) {
    // Try connecting until success
    let mut conn = connect_to_server(&dst_ip, dst).await;

    info!("mix {} connected to {} mix", src, dst);

    let add_req = AddRequest {
        packets: packets,
        layer: layer,
    };

    conn.add(Request::new(add_req))
        .await
        .expect("Failed to send add");
}

/// Verifies outgoing packets, drops invalid ones.
fn verify_outgoing_setup_packets(packets: &mut Vec<MixnetPacketType>, network_info: &Network, layer: u32) {
    *packets = packets
        .par_iter()
        .filter_map(|packet| {
            if let MixnetPacketType::SetupPacket(setup_packet) = packet {
                // Verify the setup packet
                if verify_setup_packet(setup_packet, network_info, (layer - 1) as u64).1 {
                    return Some(packet.clone());
                }
            }
            None
        })
        .collect();
}

fn is_middle_layer(output_buf: &HashMap<u32, (Vec<Vec<MixnetPacketType>>, u32)>) -> bool {
    if output_buf.is_empty() {
        return false;
    }
    let current_layer = output_buf.keys().min().expect("HashMap is Empty").clone();
    let counter: u32 = output_buf.get(&current_layer).unwrap().1;

    return counter % (*NUM_MIXES as u32) == 0 && (current_layer as u64) != *NUM_LAYERS;
}

/// returns the number of outgoing packets
/// and the server that will get the maximum amount of packets
fn get_edge_case_info(output_buffer: &Vec<Vec<Packet>>) -> (usize, usize) {
    let mut max_index = 0;
    let mut max_size = 0;
    let mut total_outgoing = 0;
    for (i, packets) in output_buffer.iter().enumerate() {
        total_outgoing += packets.len();
        if packets.len() > max_size {
            max_size = packets.len();
            max_index = i;
        }
    }
    return (max_index, total_outgoing);
}

pub fn get_edge_limit(n: u64, m: u16) -> u64 {
    // calculate n/m + sqrt(nlog(m)/m) rounded up
    let n = n as f64;
    let m = m as f64;
    let edge_limit = (n / m + (n * m.log(2.0) / m).sqrt()).ceil() as u64;
    return edge_limit;
}

// TODO: the exact "limit" can be pre-calculated in the setup stage, why not - ya know?
/// Returns if the amount of packets "i" is to be considered questionable
fn is_out_of_bounds(i: usize, total: usize) -> bool {
    let is_over_edge = i > *EDGE_LIMIT as usize;
    if is_over_edge {
        info!(
            "OVER BOUND! number of packets: {}, total outgoing: {}",
            i,
            total
        );
    }
    is_over_edge
}

async fn wait_for_shutdown(id: u16) {
    let my_shutdown_path = format!("{}{}", *SHUTDOWN_FILE, id);
    // In case of leftovers from previous run
    let _ = std::fs::remove_file(&my_shutdown_path);
    loop {
        if tokio::fs::metadata(&my_shutdown_path).await.is_ok() {
            // rm shutdown file for next run
            let _ = std::fs::remove_file(&my_shutdown_path);
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn process_init_packets(
    init_packets: Vec<Vec<u8>>,
    network: &Network,
    id: u16,
    layer: u64,
) -> Vec<Vec<Vec<u8>>> {
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); Into::<usize>::into(*NUM_MIXES)];

    let dec_packets = decrypt_incoming_packets(init_packets, id, layer as u32, network);

    // Insert decrypted packets to output_buffer
    for (dec_packet, next) in dec_packets {
        packets[next as usize].push(dec_packet.data);
    }
    return packets;
}

fn send_init_packets(mix_ips: &Vec<IpAddr>, id: u16) -> Vec<JoinHandle<()>> {
    let mut mix_tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));
    let mut init_buffer = process_init_packets(get_init_packets(id), &get_network_info(), id, 0);
    for dst in (0..*NUM_MIXES).rev() {
        let packets = init_buffer.pop().unwrap();
        let dst_ip = mix_ips[dst as usize];
        let task = tokio::spawn(async move {
            connect_and_send(dst_ip, dst, id, packets, 1).await;
        });
        mix_tasks.push(task);
    }
    return mix_tasks;
}

/// Try connecting to server dst until success
pub async fn connect_to_server(dst_ip: &IpAddr, dst: u16) -> MixClient<Channel> {
    let mut conn_result =
        MixClient::connect(format!("http://{}:{}", dst_ip, *BASE_PORT + dst)).await;
    loop {
        match conn_result {
            Ok(ref _result) => {
                break;
            }
            Err(err) => {
                warn!("Failed to connect to mix {}: {:?}", dst, err);
                sleep(Duration::from_millis(500)).await;
                conn_result =
                    MixClient::connect(format!("http://{}:{}", dst_ip, *BASE_PORT + dst)).await;
            }
        }
    }
    return conn_result.unwrap();
}

async fn start_server(mix_ips: Vec<IpAddr>, id: u16) {
    let my_ip = mix_ips[id as usize];
    let mix = MyServer::new(mix_ips, id);
    info!("mix {} started", id);
    let task = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve_with_shutdown(
                format!("{}:{}", my_ip, *BASE_PORT + id).parse().unwrap(),
                async move {
                    // // Wait for a SIGINT signal to shutdown
                    // tokio::signal::ctrl_c().await.unwrap();
                    wait_for_shutdown(id).await;
                },
            )
    });

    task.await.unwrap().await.expect("Failed to Start Server");
}

async fn start_mix_round(mix_ips: &Vec<IpAddr>, id: u16) {
    let mix_tasks = send_init_packets(mix_ips, id);
    join_all(mix_tasks).await;
}

/// runs Mix
pub async fn run_mix(mix_ips: Vec<IpAddr>, id: u16) {
    let mix_tasks = send_init_packets(&mix_ips, id);
    let server_task = start_server(mix_ips, id);
    server_task.await;
    join_all(mix_tasks).await;
}
