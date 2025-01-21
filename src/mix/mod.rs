use crate::config::*;
use crate::marshal::info::{get_init_packets, get_init_setup_packets, get_network_info};
use crate::marshal::logs::RESULTS;
use crate::marshal::SHUTDOWN_FILE;
use crate::network::{decrypt_setup_layer, verify_setup_packet, Connections, Network, SetupPacket};
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
        let num_total_rounds = *NUM_SETUP_ROUNDS + *NUM_DATA_ROUNDS;
        for round in 0..num_total_rounds {
            // Wait til the mix is done getting all requests for this round
            let amount_to_acquire = (*NUM_MIXES as u32) * ((*NUM_LAYERS - 1) as u32) + 1;
            let _ = self
                .notify
                .acquire_many(amount_to_acquire)
                .await
                .unwrap()
                .forget();

            messages = self.output_last_layer(round).await;

            // Measure time for this round
            self.measure_time(round).await;

            if round < num_total_rounds - 1 {
                // Run next round
                // TODO: fix this so it is not hardcoded which rounds are setup and which are data
                // TODO: fix bug when NUM_ROUNDS is 0 - this will cause infinite rounds
                info!("mix {} is starting round {}", self.id, round + 1);
                if round < *NUM_SETUP_ROUNDS - 1 { // first 3 rounds are setup rounds
                    start_mix_round::<SetupPacket>(&self.mix_ips, self.id).await;
                } else { 
                    start_mix_round::<Vec<u8>>(&self.mix_ips, self.id).await;
                }
            }
        }
        // Note: messages should be the same for each round so it doesn't matter which one we use
        // TODO: above assumption is now wrong, fix that.
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

        // Send to all mixes
        let sending_layer = (*guard).keys().min().expect("HashMap is Empty").clone();
        let mut layer_output_buffer = (*guard).remove(&sending_layer).unwrap();
        drop(guard);

        // For edge case mixnet verification
        let total_outgoing: usize = layer_output_buffer.0.iter().map(|packets| packets.len()).sum::<usize>();

        for i in (0..*NUM_MIXES).rev() {
            let mut packets = layer_output_buffer.0.pop().unwrap();
            info!(
                "mix {} sent to mix {} {} packets FROM layer {}",
                self.id,
                i,
                packets.len(),
                sending_layer
            );
            // Verify packets if needed - AKA if packet type is SetupPacket
            T::handle_verify_on_output(
                    &mut packets,
                    &self.network_info,
                    sending_layer,
                    self.id,
                    i,
                    total_outgoing
            );
            packets.shuffle(&mut thread_rng());
            let mut packets_data = Vec::new();
            while let Some(packet) = packets.pop() {
                match packet {
                    MixnetPacketType::Packet(data) => packets_data.push(data),
                    MixnetPacketType::SetupPacket(setup_packet) => packets_data.push(setup_packet.data),
                }
            }
            let task = self.send_to_mix::<T>(i, sending_layer, packets_data);
            mix_tasks.push(task);
        }
        /*
            Start timer after the "first middle layer" (after the first layer) is done processing
            This is BEFORE the packets are sent, just after they're processed
         */
        if sending_layer == *FIRST_MEASURED_LAYER {
            let mut time_guard = self.time.lock().await;
            *time_guard = Instant::now();
        }
        join_all(mix_tasks).await;
    }

    // Send packets from mix layer "layer" to mix "dst", if setup packet send setup, else add
    async fn send_to_mix<T: MixnetPacket>(&self, dst: u16, layer: u32, packets: Vec<Vec<u8>>) -> () {
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
        // Call setup if T is a setup packet, Call add if T is Vec<u8>
        if T::is_setup_packet() {
            channel.setup(Request::new(packet_request)).await.expect("Failed to send setup");
        } else {
            channel.add(Request::new(packet_request)).await.expect("Failed to send add");
        };
    }

    /// Process all layers into single output message
    async fn output_last_layer(&self) -> Vec<Vec<u8>> {
        let mut guard = self.output_buffer.lock().await;
        let send_layer = (*guard).keys().min().expect("HashMap is Empty").clone();
        let send_layer_packets = (*guard).remove(&send_layer).unwrap();
        drop(guard);
        let mut packets = (0..*NUM_MIXES)
            .map(|i| send_layer_packets.0[i as usize].to_vec())
            .flatten()
            .collect::<Vec<MixnetPacketType>>();

        let mut messages = Vec::new();
        while let Some(packet) = packets.pop() {
            match packet {
                MixnetPacketType::Packet(data) => messages.push(data),
                MixnetPacketType::SetupPacket(setup_packet) => messages.push(setup_packet.data),
            }
        }
        // Shuffle the mix output
        messages.shuffle(&mut thread_rng());
        return messages;
    }

    // Print time since last measurement
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

fn is_middle_layer(output_buf: &HashMap<u32, (Vec<Vec<MixnetPacketType>>, u32)>) -> bool {
    if output_buf.is_empty() {
        return false;
    }
    let current_layer = output_buf.keys().min().expect("HashMap is Empty").clone();
    let counter: u32 = output_buf.get(&current_layer).unwrap().1;

    return (current_layer == 1 && counter == 1) || 
        (counter % (*NUM_MIXES as u32) == 0 && (current_layer as u64) != *NUM_LAYERS);
}

pub fn get_edge_limit(users_amount: usize) -> u64 {
    // calculate n/m + sqrt(nlog(m)/m) rounded up - this is a load balancing constant
    let n = users_amount as f64;
    let m = *NUM_MIXES as f64;
    let edge_limit = (n / m + (n * m.log(2.0) / m).sqrt()).ceil() as u64;
    return edge_limit;
}

// TODO: the exact "limit" can be pre-calculated in the setup stage, why not - ya know?
/// Returns if the amount of packets "i" is to be considered questionable
fn is_out_of_bounds(i: usize, total: usize) -> bool {
    let is_over_edge = i > get_edge_limit(total) as usize;
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

fn send_init_packets<T: MixnetPacket>(mix_ips: &Vec<IpAddr>, id: u16) -> JoinHandle<()> {
    let init_buffer : Vec<Vec<u8>>;
    if T::is_setup_packet() {
        init_buffer = get_init_setup_packets(id);
    } else {
        init_buffer = get_init_packets(id);
    }
    let dst_ip = mix_ips[id as usize];
    let task = tokio::spawn(async move {
        let mut conn = connect_to_server(&dst_ip, id).await;

        info!("mix {} connected to {} mix", id, id);
    
        let add_req = PacketRequest {
            packets: init_buffer,
            layer: 0,
        };
    
        if T::is_setup_packet() {
            conn.setup(Request::new(add_req))
                .await
                .expect("Failed to send setup");
        } else {
            conn.add(Request::new(add_req))
            .await
            .expect("Failed to send add");
        }
    });
    return task;
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

// TODO: change this function to implement "send_init_packets" with the channels self has
//       This is possible since start_mix_round is only called by get which has access to the channels 
//       This way time measurement for next layers can be done straight after the previous round is done
async fn start_mix_round<T: MixnetPacket>(mix_ips: &Vec<IpAddr>, id: u16) {
    send_init_packets::<T>(mix_ips, id).await.unwrap();
}

/// runs Mix
pub async fn run_mix(mix_ips: Vec<IpAddr>, id: u16) {
    let mix_task = send_init_packets::<SetupPacket>(&mix_ips, id);
    let server_task = start_server(mix_ips, id);
    server_task.await;
    mix_task.await.unwrap();
}
