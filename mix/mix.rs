use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix::{mix_server::Mix};
use mix::{AddMessagesRequest, AddMessagesResponse};
use mix::{GetMessagesRequest, GetMessagesResponse};
use bbs::network::Server as crypto_server;
use bbs::network::Network;
use tokio_stream::wrappers::ReceiverStream;
use std::env;
use local_ip_address::local_ip;
use std::net::SocketAddr;
use serde_json;
use std::{thread, time};

use crate::mix::mix_server::MixServer;

pub mod mix {
  tonic::include_proto!("mix");
}

#[derive(Debug)]
pub struct MixService {
    address: SocketAddr,
    crypto: crypto_server,
    network: Network,


}

#[tonic::async_trait]
impl Mix for MixService {
    type GetMessagesStream = ReceiverStream<Result<GetMessagesResponse, Status>>;

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
    ) -> Result<Response<Self::GetMessagesStream>, Status> {
        // TODO: get all the messages from previous mixes or clients (mixes info based on public file)
        std::unimplemented!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    // in command line argument get my name/ID, look for my config file according to my id.
    let mut mix_id = env::args().nth(1).expect("Please specify the mix id as the first argument");
    let mut tmp_mix_id = mix_id.clone();

    // get ip address
    let my_ip = local_ip().unwrap();
    let addr = SocketAddr::new(my_ip, 3141);
    
    // write address to predetermined file "mix_id_addr.json"
    let addr_str: &str = "_addr.json";
    mix_id.push_str(addr_str);
    std::fs::write(
        mix_id,
        serde_json::to_string_pretty(&addr).unwrap(),
    ).unwrap();


    // get crypto info from predetermined file "mix_id_crypto.json"
    let crypto_str: &str = "_crypto.json";
    // removing last 5 letters of mix_id
    tmp_mix_id.truncate(tmp_mix_id.len() - addr_str.len());
    tmp_mix_id.push_str(crypto_str);
    let crypto_str = std::fs::read_to_string(tmp_mix_id).unwrap();
    let crypto: crypto_server = serde_json::from_str(&crypto_str).unwrap();

    // get network info from predetermined file "network.json"
    let network_str : &str = "network.json";
    let network : Network = serde_json::from_str(&network_str).unwrap();

    // make new service, wait a few seconds (like 10 - to ensure all servers managed to load and listen for clients)
    let mix_service = MixService {address: addr, crypto: crypto, network: network};
    Server::builder().add_service(MixServer::new(mix_service)).serve(addr).await?;
    thread::sleep(time::Duration::from_secs(5));

    // tell the service to dial to all other mixes and coordinator!

    // Coordinator tells me to start round by using AddMessages

    // Wait for coordinator to tell me: exit.

    Ok(())
}


/*
Pro tips for later:
- make sure the servers have the same keys in different rounds. 
- make sure you dont generate keys if you already ran the program once.
- make sure you reuse the same onion messages. (different onions for different rounds though).

Coordinator creates messages for the test.
calls AddMessages for each server and give it the messages for the round.

coordinator then calls getMessages for the current round, and is blocked until all 
servers have answered it.

writes the time the round took.

verifies that all messages in the responses are equal to what it sent in the beggining (without the onion layers)

*/