use std::sync::Arc;
use futures::future::join_all;
use tokio::sync::Semaphore;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix_service::mix_server::{MixServer, Mix};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use futures::Stream;
use crate::config::*;
use mix_service::mix_client::MixClient;
use tokio::time::{sleep, Duration};

/// Service created from proto file
pub mod mix_service {
    tonic::include_proto!("mix");
}

#[derive(Debug)]
/// Mix server struct
pub struct MyServer {
    id : u16,
    notify:Arc<Semaphore>,
}

// TODO: semaphore is overkill, do lock

impl MyServer{
    /// Create instance of MyServer
    pub fn new(id: u16) -> Self {
        return MyServer { id, notify: Arc::new(Semaphore::new(0)) }
    }
}

#[tonic::async_trait]
impl Mix for MyServer {
    type GetStream = Box<dyn Stream<Item = Result<GetResponse, Status>> + Send + Unpin>;

    async fn add(
        &self,
        request: Request<Streaming<AddRequest>>,
    ) -> Result<Response<AddResponse>, Status> {
        println!("I, mix {} got an add request: {:?}", self.id, request);
        
        self.notify.add_permits(1);
        
        let reply = AddResponse {};

        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<Self::GetStream>, Status>  {
        println!("I, mix {} got a get request: {:?}", self.id, request);
        let mut i = 0;
        while i < NUM_MIXES {
            // 
            let _ = self.notify.acquire().await;
            i += 1;
        }
        let messages = vec![
            GetResponse { messages: vec![vec![0x01, 0x02, 0x03]] },
        ];
        let stream = futures::stream::iter(messages.into_iter().map(Ok));
        return Ok(Response::new(Box::new(Box::pin(stream))));
    }
}

/// runs Mix
pub async fn run_mix(id: u16){
    let mix = MyServer::new(id);

    let mix_str = "\"".repeat(id.into());
    println!("{}Start mix {}{}", mix_str, id, mix_str);

    let server_task = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve(format!("[::1]:{}", BASE_PORT + id).parse().unwrap())
    });

    println!("{}Server Up mix {}{}", mix_str, id, mix_str);
    // server.Listen()
    // waitgroup.done()
    sleep(Duration::from_secs(5)).await;
    // waitgroup.WaitAll()
    let mut mix_tasks = Vec::with_capacity(NUM_MIXES.into());
    // TODO: wrong in general mix, the connecting and sending should also be done concurrently.
    for i in 0..NUM_MIXES {
        let task = tokio::spawn(async move {
            println!("{} {}", i, id);
            let mut client =
                MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await.unwrap();
        
            println!("Mix {} connected to {} mix", id, i);
        
            let add_req = vec![AddRequest { packets: vec![vec![0x01]] }];
            client.add(Request::new(futures::stream::iter(add_req.clone()))).await.expect("Failed to send add");
        });

        mix_tasks.push(task);
    }

    server_task.await.unwrap().await.expect("Failed to run Server");
    join_all(mix_tasks).await;
}