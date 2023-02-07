use tonic::{Request, Response, Status};
use client::{client_server::Client};
use client::{GenerateMessagesRequest, GenerateMessagesResponse};
use client::{SubmitMessagesRequest, SubmitMessagesResponse};
use bbs::network::Client as crypto;
use bbs::network::Network;
use bbs::network::{Packet, generate_packet};

pub mod client {
  tonic::include_proto!("client");
}

#[derive(Debug)]
pub struct ClientService {
    crypto: crypto,
    address: String,
    network: Network,
}

#[tonic::async_trait]
impl Client for ClientService {
    async fn generate_messages(
        &self,
        request: Request<GenerateMessagesRequest>
    ) -> Result<Response<GenerateMessagesResponse>, Status> {
        // TODO: generate message for next round, increase round number by 1
        // let data = vec![1,2,3];
        // let packet = generate_packet(data, &self.crypto, &self.network);
        Ok(Response::new(client::GenerateMessagesResponse {}))
    }
    
    async fn submit_messages(
        &self,
        request: Request<SubmitMessagesRequest>
    ) -> Result<Response<SubmitMessagesResponse>, Status> {
        // TODO: send message of current round to the server from db
        // let r = request.into_inner();
        Ok(Response::new(client::SubmitMessagesResponse {}))
    }

}

pub fn main(){
    // TODO: get address, port and signature from predetermined file
    // TODO: get network info (to generate packets...IDprovider, network size, etc.) from predetermined file
    // TODO: create client and run it, connecting the service to the server

    // Something like this:
    // let clt_service = ClientService {crypto, address, network_info, round: 0};
    // Server::builder().add_service(ClientServer::new(clt_service)).serve(address).await?;
    // OK(())
    
    println!("Please work!");
}