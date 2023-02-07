use tonic::{Request, Response, Status, Streaming};
use mix::{mix_server::Mix};
use mix::{AddMessagesRequest, AddMessagesResponse};
use mix::{GetMessagesRequest, GetMessagesResponse};
use mix::{StartRoundRequest, StartRoundResponse};
use bbs::network::Server;
use bbs::network::Network;
use bbs::network::Packet;

pub mod mix {
  tonic::include_proto!("mix");
}

#[derive(Debug)]
pub struct MixService {
    address: String,
    crypto: Server,
    network: Network,
}

#[tonic::async_trait]
impl Mix for MixService {
    async fn add_messages(
        &self,
        request: tonic::Request<Streaming<AddMessagesRequest>>
    ) -> Result<Response<AddMessagesResponse>, Status> {
        // TODO: send all messages to the next mixes or clients (mixes info based on public file)
        Ok(Response::new(mix::AddMessagesResponse {}))
    }
    
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>
    ) -> Result<Response<GetMessagesResponse>, Status> {
        // TODO: get all the messages from previous mixes or clients (mixes info based on public file)
        Ok(Response::new(mix::GetMessagesResponse {messages: vec![]}))
    }

    async fn start_round(
        &self,
        request: Request<StartRoundRequest>
    ) -> Result<Response<StartRoundResponse>, Status> {
        // TODO: start a new round, send to next layer mixes to update that "I'm moving to the next round"
        Ok(Response::new(mix::StartRoundResponse {}))
    }
}

pub fn main(){
    // TODO: get address, port and signature from predetermined file
    // TODO: get network info (to generate packets...IDprovider, network size, etc.) from predetermined file
    // TODO: create mix and run it, connecting the service to the server

    // Something like this:
    // let mix_service = MixService {...};
    // Server::builder().add_service(MixServer::new(mix_service)).serve(address).await?;
    // OK(())

    
    println!("Please work!");
}