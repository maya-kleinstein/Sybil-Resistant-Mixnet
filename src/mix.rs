use crate::config::*;
use crate::marshal::info::{get_init_packets, get_network_info, process_init_packets};
use crate::marshal::logs::RESULTS;
use crate::marshal::SHUTDOWN_FILE;
use crate::network::{decrypt_layer, verify_batch, verify_packet, Network, Packet};
use futures::future::join_all;
use log::*;
use mix_service::mix_client::MixClient;
use mix_service::mix_server::{Mix, MixServer};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use rand::seq::SliceRandom;
use rand::thread_rng;
use rayon::prelude::*;
use statrs::distribution::{Binomial, DiscreteCDF};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

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
    output_buffer: Mutex<HashMap<u32, (Vec<Vec<Packet>>, u32)>>,
    // Maps between mix id to connection
    channels: Arc<Mutex<HashMap<u32, MixClient<Channel>>>>,
    network_info: Network,
    notify: Arc<Semaphore>,
    time: Mutex<Instant>,
}

impl MyServer {
    /// Create instance of MyServer
    pub fn new(mix_ips: Vec<IpAddr>, id: u16) -> Self {
        return MyServer {
            id,
            mix_ips,
            output_buffer: Mutex::new(HashMap::new()),
            channels: Arc::new(Mutex::new(HashMap::new())),
            network_info: get_network_info(),
            notify: Arc::new(Semaphore::new(0)),
            time: Mutex::new(Instant::now()),
        };
    }
}

#[tonic::async_trait]
impl Mix for MyServer {
    async fn add(&self, request: Request<AddRequest>) -> Result<Response<AddResponse>, Status> {
        self.parse_input(request).await;

        self.handle_middle_layer().await;

        // Notify config
        self.notify.add_permits(1);
        let reply = AddResponse {};
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

            messages = self.output_all().await;

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
    /// Process (decrypt) incoming stream to proper output buffer
    async fn parse_input(&self, request: Request<AddRequest>) -> () {
        let add_req = request.into_inner();
        info!(
            "mix {} got {} packets FROM layer {}",
            self.id,
            add_req.packets.len(),
            add_req.layer
        );
        // Decrypt packets - Verify as well in case of MixnetVerification::Verify
        let dec_packets =
            decrypt_incoming_packets(add_req.packets, self.id, add_req.layer, &self.network_info);

        let mut buffer_guard = self.output_buffer.lock().await;
        // Update counter
        (*buffer_guard)
            .entry(add_req.layer + 1)
            .or_insert((vec![vec![].into(); Into::<usize>::into(*NUM_MIXES)], 0))
            .1 += 1;

        // Insert decrypted packets to output_buffer
        for (dec_packet, next) in dec_packets {
            (*buffer_guard).entry(add_req.layer + 1).and_modify(|e| {
                e.0[next as usize].push(dec_packet);
            });
        }
    }

    /// Send add from current layer to all mixes
    async fn handle_middle_layer(&self) -> () {
        let mut mix_tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));

        let mut guard = self.output_buffer.lock().await;
        // Check if it is a middle layer
        if !is_middle_layer(&*guard) {
            return;
        }
        // Start timer when first packet recv'd
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
            // Verify packets if needed
            handle_verify_on_output(
                &mut packets,
                &self.network_info,
                layer,
                self.id,
                Some(i),
                edge_case_index,
                total_outgoing,
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
        let add_req = AddRequest {
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
            .add(Request::new(add_req))
            .await
            .expect("Failed to send add");
    }

    /// Process all layers into single output message
    async fn output_all(&self) -> Vec<Vec<u8>> {
        let buffer_guard = self.output_buffer.lock().await;
        let send_layer = (*buffer_guard).keys().min().expect("HashMap is Empty");
        let mut packets = (0..*NUM_MIXES)
            .map(|i| (*buffer_guard).get(send_layer).unwrap().0[i as usize].to_vec())
            .flatten()
            .collect::<Vec<Packet>>();
        handle_verify_on_output(
            &mut packets,
            &self.network_info,
            send_layer.clone(),
            self.id,
            None,
            0,
            0,
        );

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

/// Decrypt incoming packets
pub fn decrypt_incoming_packets(
    packets: Vec<Vec<u8>>,
    id: u16,
    layer: u32,
    network_info: &Network,
) -> Vec<(Packet, u64)> {
    let dec_packets = packets
        .par_iter()
        .filter_map(|i| decrypt_layer(i, id.into(), network_info, layer as u64))
        .collect();
    return dec_packets;
}

/// Verify outgoing packets when MIX_VERIFICATION is set to BatchVerify OR OnlyVerifyEdgeCases
fn handle_verify_on_output(
    packets: &mut Vec<Packet>,
    network_info: &Network,
    layer: u32,
    src: u16,
    dst: Option<u16>, // None if output is to client
    edge_case_index: usize,
    total_outgoing: usize,
) {
    match *MIX_VERIFICATION {
        MixnetVerification::BatchVerify => verify_batch(&packets, network_info, (layer - 1) as u64),
        MixnetVerification::OnlyVerifyEdgeCases => match dst {
            // In the case of a middle layer outputing to mix
            Some(dst) => {
                if dst == edge_case_index as u16
                    && is_out_of_bounds(packets.len(), total_outgoing.clone())
                {
                    info!("mix {} is verifying edge case to mix {}", src, dst);
                    // Verify all packets, "throw away" all non valid packets
                    verify_outgoing_packets(packets, network_info, layer);
                }
            }
            // In the case of last layer outputing to config ("clients")
            None => (),
        },
        _ => (),
    }
}

/// Verifies outgoing packets, drops invalid ones.
fn verify_outgoing_packets(packets: &mut Vec<Packet>, network_info: &Network, layer: u32) {
    let retain_flags: Vec<bool> = packets
        .par_iter()
        .map(|packet| verify_packet(packet, network_info, (layer - 1) as u64).1)
        .collect();

    let mut valid_packets: Vec<Packet> =
        Vec::with_capacity(retain_flags.iter().filter(|&&item| item).count());

    for (packet, flag) in packets.drain(..).zip(retain_flags.into_iter()) {
        if flag {
            valid_packets.push(packet);
        }
    }

    *packets = valid_packets;
}

fn is_middle_layer(output_buf: &HashMap<u32, (Vec<Vec<Packet>>, u32)>) -> bool {
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

// TODO: the exact "limit" can be pre-calculated in the setup stage, why not - ya know?
/// Returns if the amount of packets "i" is to be considered questionable
fn is_out_of_bounds(i: usize, total: usize) -> bool {
    let p = 1_f64 / (*NUM_MIXES as f64);
    let binomial = Binomial::new(p, total as u64).unwrap();
    // cdf = Prob(Bin(n,p) <= i)
    let cdf = binomial.cdf(i as u64);
    let result = (1_f64 - cdf) < *EDGE_LIMIT && i > total / (*NUM_MIXES as usize);
    if result {
        info!(
            "OVER BOUND! number of packets: {}, total outgoing: {}, probability: {}",
            i,
            total,
            (1_f64 - cdf)
        );
    }
    result
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
