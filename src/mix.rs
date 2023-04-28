use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix_service::mix_server::{MixServer, Mix};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use futures::Stream;
use crate::config::*;

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

/// Run MyServer instance as Server
pub fn run_service(mix: MyServer) -> JoinHandle<()>{
    let id = mix.id;
    let server_thread = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve(format!("[::1]:{}", BASE_PORT + id).parse().unwrap()).await.unwrap();
    });

    return server_thread;
}