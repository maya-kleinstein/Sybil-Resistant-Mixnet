use std::sync::{Arc};//, Condvar, Mutex};
use std::thread::{sleep};
use std::time;
use tokio::sync::Semaphore;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix_service::mix_server::{MixServer, Mix};
use mix_service::{AddRequest, AddResponse, GetRequest, GetResponse};
use crate::mix_service::mix_client::MixClient;
use futures::Stream;

const BASE_PORT: u16 = 50500;
const NUM_MIXES: u16 = 3;

pub mod mix_service {
    tonic::include_proto!("mix");
}

#[derive(Debug)]
pub struct MyServer {
    notify:Arc<Semaphore>,
}

#[tonic::async_trait]
impl Mix for MyServer {
    // TODO: type GetStream = ReceiverStream<Result<GetResponse, Status>>;
    type GetStream = Box<dyn Stream<Item = Result<GetResponse, Status>> + Send + Unpin>;

    async fn add(
        &self,
        request: tonic::Request<Streaming<AddRequest>>,
    ) -> Result<Response<AddResponse>, Status> {
        println!("Got an add request: {:?}", request);
        
        self.notify.add_permits(1);
        
        let reply = AddResponse {};

        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<Self::GetStream>, Status>  {
        println!("Got a get request: {:?}", request);
        let mut i = 0;
        while i < NUM_MIXES {
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


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("no pattern given");
    let id: u16 = arg.parse().unwrap();

    let mix = MyServer {
        notify: Arc::new(Semaphore::new(0)),
    };

    println!("#### Start #####");

    let server_thread = tokio::spawn(async move {
        Server::builder()
            .add_service(MixServer::new(mix))
            .serve(format!("[::1]:{}", BASE_PORT + id).parse().unwrap()).await.unwrap();
    });

    println!("#### Server Up #####");

    // Servers up and get requests recieved
    sleep(time::Duration::from_secs(10));

    for i in 0..NUM_MIXES {
        let mut client =
            MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await?;
        
        println!("#### Connected to {} mix #####", i);
        
        let add_req = vec![AddRequest { packets: vec![vec![0x01]] }];
        let _response = client.add(Request::new(futures::stream::iter(add_req.clone()))).await?;
        // println!("RESPONSE={:?}", _response);
    }

    server_thread.await.unwrap();
    Ok(())
}
