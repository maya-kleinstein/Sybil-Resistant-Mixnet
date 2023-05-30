use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex, oneshot::Receiver};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix_service::mix_server::{MixServer, Mix};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use mix_service::mix_client::MixClient;
use futures::{Stream, FutureExt};
use futures::future::join_all;
use crate::config::*;
use crate::marshal::{get_init_packets, get_network_info, process_init_packets};
use crate::network::{Network, decrypt_layer};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};

/// Service created from proto file
pub mod mix_service {
    tonic::include_proto!("mix");
}

#[derive(Debug)]
/// Mix server struct
pub struct MyServer {
    id : u16,
    // Maps between layer to (layer output buffer, layer add request counter)
    output_buffer: Mutex<HashMap<u32, (Vec<Vec<Vec<u8>>>, u32)>>,
    network_info: Network,
    notify:Arc<Semaphore>,
}

impl MyServer{
    /// Create instance of MyServer
    pub fn new(id: u16) -> Self {
        return MyServer { 
            id,
            output_buffer: Mutex::new(HashMap::new()),
            network_info: get_network_info(),
            notify: Arc::new(Semaphore::new(0)),
        }
    }
}

#[tonic::async_trait]
impl Mix for MyServer {
    type GetStream = Box<dyn Stream<Item = Result<GetResponse, Status>> + Send + Unpin>;

    async fn add(
        &self,
        request: Request<Streaming<AddRequest>>,
    ) -> Result<Response<AddResponse>, Status> {
        println!("mix {} got an add request: {:?}", self.id ,request);
        self.parse_input(request).await;

        if self.check_if_middle_layer().await {
            self.send_middle_layer().await;
        }
    
        // Notify config
        self.notify.add_permits(1);
        let reply = AddResponse {};
        Ok(Response::new(reply))
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<Self::GetStream>, Status>  {
        println!("mix {} got a get request: {:?}", self.id, request);
        // Wait til the mix is done getting add requests for all layers
        let _ = self.notify.acquire_many((NUM_MIXES as u32)*((NUM_LAYERS - 1) as u32)).await.unwrap();
        let messages = self.output_all().await;
        let stream = futures::stream::iter(messages.into_iter().map(Ok));
        return Ok(Response::new(Box::new(Box::pin(stream))));
    }
}

impl MyServer {
    /// Process (decrypt) incoming stream to proper output buffer 
    async fn parse_input(
        &self,
        request: Request<Streaming<AddRequest>>,
    ) -> () {
        let input_stream = request.into_inner().message().await.unwrap().into_iter();
        for packets in input_stream {
            let mut buffer_guard = self.output_buffer.lock().await;
            (*buffer_guard).entry(packets.layer + 1)
                    .or_insert((vec![vec![].into(); NUM_MIXES.into()], 0))
                    .1 += 1;
            for packet in packets.packets {
                let (dec_packet, next) = decrypt_layer(packet, self.id.into(), &self.network_info, 0);
                // Insert decrypted packet to output_buffer
                (*buffer_guard).entry(packets.layer + 1)
                    .and_modify(|e| { 
                        e.0[next as usize].push(dec_packet);
                    });  
            }
        }
    }

    /// Send add from current layer to all mixes
    async fn send_middle_layer(
        &self,
    ) -> () {      
        let mut mix_tasks = Vec::with_capacity(NUM_MIXES.into());
        {
            // IMPORTANT NOTE: this is in a seperate scope in order to release guard and avoid deadlock
            let mut guard = self.output_buffer.lock().await;
            // TODO: verify!!!
            // Verify + Send to all mixes
            let layer = (*guard).keys().min().expect("HashMap is Empty").clone();
            let mut output_buffer = (*guard).remove(&layer).unwrap();
            for i in (0..NUM_MIXES).rev() {
                println!("packets for layer {} sent from mix {} to {}", layer + 1, self.id, i);
                // TODO: Shuffle before sending!
                let packets = output_buffer.0.pop().unwrap();
                let id = self.id;
                let task = tokio::spawn(async move {
                    connect_and_send(i, id, packets, layer).await;
                });
                mix_tasks.push(task);
            }
        }
        join_all(mix_tasks).await;
    }

    /// Process all layers into single output message
    async fn output_all(
        &self,
    ) -> Vec<GetResponse> {
        let buffer_guard = self.output_buffer.lock().await;
        let send_layer = (*buffer_guard).keys().min().expect("HashMap is Empty");
        let messages: Vec<GetResponse> = vec![
            GetResponse {
                messages: (0..NUM_MIXES).map(|i| 
                    (*buffer_guard).get(send_layer).unwrap().0[i as usize].to_vec()
                ).flatten().collect::<Vec<Vec<u8>>>()
            }
        ];
        return messages;
    }

    async fn check_if_middle_layer(
        &self,
    ) -> bool {
        let guard = self.output_buffer.lock().await;
        let current_layer = (*guard).keys().min().expect("HashMap is Empty").clone();
        let counter: u32 = (*guard).get(&current_layer).unwrap().1;
        return counter % (NUM_MIXES as u32) == 0 &&
                (current_layer as u64) != NUM_LAYERS;

    }
}

async fn connect_and_send(dst : u16, src: u16, packets: Vec<Vec<u8>>, layer: u32) {
    // TODO: save connections between the mixes
    let mut client =
        MixClient::connect(format!("http://[::1]:{}", BASE_PORT + dst)).await.unwrap();

    println!("mix {} connected to {} mix", src, dst);

    let add_req = vec![AddRequest {
        packets: packets,
        layer: layer
    }];
    client.add(Request::new(futures::stream::iter(add_req.clone()))).await.expect("Failed to send add");
}


async fn start_server(id: u16, rx: Receiver<u8>) {
    let mix = MyServer::new(id);
    println!("Start mix {}", id);
    // Measuring time
    let start = Instant::now();
    let task = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve_with_shutdown(format!("[::1]:{}", BASE_PORT + id).parse().unwrap(), rx.map(drop))
    });
    task.await.unwrap().await.expect("Failed to Start Server");

    // Measuring time
    let duration = start.elapsed();
    println!("Time elapsed in expensive_function() is: {:?}", duration);
}


/// runs Mix
pub async fn run_mix(id: u16, rx: Receiver<u8>){
    let server_task = start_server(id, rx);
    let mut mix_tasks = Vec::with_capacity(NUM_MIXES.into());
    let mut init_buffer = process_init_packets(get_init_packets(id), id, &get_network_info(), 0);
    for i in (0..NUM_MIXES).rev() {
        let packets = init_buffer.pop().unwrap();
        let task = tokio::spawn(async move {
            connect_and_send(i, id, packets, 1).await;
        });
        mix_tasks.push(task);
    }

    server_task.await;
    join_all(mix_tasks).await;
}