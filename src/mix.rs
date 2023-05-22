use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex};
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
        println!("Mix {} got an add request: {:?}", self.id, request);
        // Process incoming stream to proper output buffer (decrypt)
        let input_stream = request.into_inner().message().await.unwrap().into_iter();
        for packets in input_stream {
            for packet in packets.packets {
                let (dec_packet, next) = decrypt_layer(packet, self.id.into(), &self.network_info, 0);
                let mut guard = self.output_buffer.lock().await;
                (*guard)[next as usize].push(dec_packet);
            }
        }

        /* 
        Check if at the "end of layer", If so:
        batch verify/other optimizations
        Send add to all the other mixes accordingly
        */
        let mut guard = self.layer.lock().await;
        (*guard) += 1;
        if *guard == NUM_MIXES {
            *guard = 0;
            // TODO: clear output_buff
        }
        // Notify config when you're done
        self.notify.add_permits(1);

        let reply = AddResponse {};
        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<Self::GetStream>, Status>  {
        println!("Mix {} got a get request: {:?}", self.id, request);
        // let mut i = 0;
        // while i < (NUM_MIXES as u64)*NUM_LAYERS {
        //     let _ = self.notify.acquire().await;
        //     self.notify.
        //     i += 1;
        // }
        let _ = self.notify.acquire_many((NUM_MIXES as u32)*(NUM_LAYERS as u32)).await.unwrap();
        let messages = vec![
            GetResponse { messages: vec![vec![0x01, 0x02, 0x03]] },
        ];
        let stream = futures::stream::iter(messages.into_iter().map(Ok));

        /* 
        TODO: before responding, make sure that you "restart" the mix for a new epoch
        */ 
        return Ok(Response::new(Box::new(Box::pin(stream))));
    }
}

async fn establish_conn(i : u16, id: u16, packets: Vec<Vec<u8>>) {
    let mut client =
        MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await.unwrap();

    println!("Mix {} connected to {} mix", id, i);

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