use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex, MutexGuard};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix_service::mix_server::{MixServer, Mix};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use mix_service::mix_client::MixClient;
use futures::Stream;
use futures::future::join_all;
use crate::config::*;
use crate::marshal::{get_init_packets, get_network_info, process_init_packets};
use crate::network::{Network, decrypt_layer};

/// Service created from proto file
pub mod mix_service {
    tonic::include_proto!("mix");
}

#[derive(Debug)]
/// Mix server struct
pub struct MyServer {
    id : u16,
    output_buffer: Mutex<Vec<Vec<Vec<u8>>>>,
    network_info: Network,
    layer: Arc<Mutex<u16>>,
    notify:Arc<Semaphore>,
}

impl MyServer{
    /// Create instance of MyServer
    pub fn new(id: u16) -> Self {
        return MyServer { 
            id,
            output_buffer: Mutex::new(vec![vec![].into(); NUM_MIXES.into()]),
            network_info: get_network_info(),
            layer: Arc::new(Mutex::new(0)),
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
        println!("mix {} got an add request: {:?}", self.id, request);
        // Process incoming stream to proper output buffer (decrypt)
        // TODO: sometimes layers cross eachother, fix by using map for each layer + layer field in AddRequest
        let input_stream = request.into_inner().message().await.unwrap().into_iter();
        for packets in input_stream {
            for packet in packets.packets {
                let (dec_packet, next) = decrypt_layer(packet, self.id.into(), &self.network_info, 0);
                let mut buffer_guard = self.output_buffer.lock().await;
                (*buffer_guard)[next as usize].push(dec_packet);
            }
        }

        // Check if at the "end of layer", 
        // If so: batch verify/other optimizations send add to all mixes/get
        let mut mix_tasks = Vec::with_capacity(NUM_MIXES.into());
        
        // let mut layer_guard = self.layer.lock().await;
        // (*layer_guard) += 1;
        // // Check if at end of layer AND not last layer
        // if *layer_guard % NUM_MIXES == 0 && *layer_guard != ((NUM_LAYERS - 1) as u16)*NUM_MIXES {
        let check: bool;
        {
            let mut layer_guard = self.layer.lock().await;
            (*layer_guard) += 1;
            check = *layer_guard % NUM_MIXES == 0 && *layer_guard != ((NUM_LAYERS - 1) as u16)*NUM_MIXES;
        }
        if check {
             // TODO: verify!!!
            // verify + send through add to other mixes
            let mut buffer_guard = self.output_buffer.lock().await;
            // TODO: Shuffle buffer using real random NOT poser random
            for i in (0..NUM_MIXES).rev() {
                println!("AHHH mix {} send to {}", self.id, i);
                let packets = (*buffer_guard).pop().unwrap();
                let id = self.id;
                let task = tokio::spawn(async move {
                    establish_conn(i, id, packets).await;
                    println!("Am I here yet?");
                });
                mix_tasks.push(task);
            }
            *buffer_guard = vec![vec![].into(); NUM_MIXES.into()];
            // If last layer: do nothing, it'll be sent through get to config
        }
        // Notify config when you're done
        self.notify.add_permits(1);

        join_all(mix_tasks).await;
        let reply = AddResponse {};
        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<Self::GetStream>, Status>  {
        println!("mix {} got a get request: {:?}", self.id, request);
        // Wait til the mix is done getting add requests for this layer
        let _ = self.notify.acquire_many((NUM_MIXES as u32)*((NUM_LAYERS - 1) as u32)).await.unwrap();
        // Process last layer
        let buffer_guard = self.output_buffer.lock().await;
        let messages: Vec<GetResponse> = (0..NUM_MIXES)
            .map(|i| GetResponse { messages: (*buffer_guard[i as usize]).to_vec() })
            .collect();
        // Send this back to Config
        let stream = futures::stream::iter(messages.into_iter().map(Ok));
        return Ok(Response::new(Box::new(Box::pin(stream))));
    }
}

async fn establish_conn(i : u16, id: u16, packets: Vec<Vec<u8>>) {
    let mut client =
        MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await.unwrap();

    println!("mix {} connected to {} mix", id, i);

    let add_req = vec![AddRequest { packets }];
    client.add(Request::new(futures::stream::iter(add_req.clone()))).await.expect("Failed to send add");
}


async fn start_server(id: u16) {
    let mix = MyServer::new(id);
    println!("Start mix {}", id);
    let task = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve(format!("[::1]:{}", BASE_PORT + id).parse().unwrap())
    });

    task.await.unwrap().await.expect("Failed to Start Server");
}


/// runs Mix
pub async fn run_mix(id: u16){
    let server_task = start_server(id);
    let mut mix_tasks = Vec::with_capacity(NUM_MIXES.into());
    let mut init_buffer = process_init_packets(get_init_packets(id), id, &get_network_info(), 0);
    for i in (0..NUM_MIXES).rev() {
        let packets = init_buffer.pop().unwrap();
        let task = tokio::spawn(async move {
            establish_conn(i, id, packets).await;
        });
        mix_tasks.push(task);
    }

    server_task.await;
    join_all(mix_tasks).await;
}