use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use mix::{mix_server::Mix, mix_client::MixClient};
use mix::{AddMessagesRequest, AddMessagesResponse};
use mix::{GetMessagesRequest, GetMessagesResponse};
use bbs::network::{Server as crypto_server, decrypt_layer, decrypt_packet};
use bbs::network::Network;
use tokio_stream::wrappers::ReceiverStream;
use std::env;
use local_ip_address::local_ip;
use std::net::SocketAddr;
use serde_json;
use std::{thread, time};
use std::sync::atomic::AtomicBlock;
use rand::seq::SliceRandom;

use crate::mix::mix_server::MixServer;

pub mod mix {
  tonic::include_proto!("mix");
}

#[derive(Debug)]
pub struct MixService {
    my_id: u64,
    address: SocketAddr,
    crypto: crypto_server,
    network: Network,
    connections: Vec<Channel>,
    recv_count: u64,
    output_buffer: Vec<Vec<Vec<u8>>>,
}

// TODO: if in feature decide to do multiple rounds, will need to change things.

#[tonic::async_trait]
impl Mix for MixService {
    type GetMessagesStream = ReceiverStream<Result<GetMessagesResponse, Status>>;

    async fn add_messages(
        &self,
        request: tonic::Request<Streaming<AddMessagesRequest>>
    ) -> Result<Response<AddMessagesResponse>, Status> {
        let cur_layer = self.recv_count.div_euclid(self.network.size);
        let mut stream = request.into_inner();
        while let Some(add_msg_request) = stream.try_next().await? {
            // decrypt packets from this source and add them to the correct buffer
            let packets = add_msg_request.packets;
            for packet in packets {
                let (decrypted_packet, next_mix) = decrypt_layer(packet, self.my_id, &self.network, cur_layer);
                self.output_buffer[next_mix as usize].push(decrypted_packet);
            }
        }
        // TODO: verify packets (already done in decrypt layer right now... fix later to allow for batching)
        // TODO: no need for atomic block... fix later
        let atomic_block = AtomicBlock::new();
        let result = atomic_block.run(|| {
            self.recv_count += 1;
            if self.recv_count % self.network.size == 0 {
                for i in 0..self.output_buffer.len(){
                    // mix the output
                    self.output_buffer[i].shuffle(&mut rand::thread_rng());

                    // send the buffer using rpc of choice.
                    let mut client = MixClient::new(self.connections[i]);
                    let request = tonic::Request::new(AddMessagesRequest {
                        packets : self.output_buffer[i]
                    });
                    let response = client.send(request).await?.into_inner();
                    // Clear output buffer

                }
                self.output_buffer.clear();
            }
        });

        Ok(Response::new(mix::AddMessagesResponse {}))
    }
            // ATOMICALLY: layer_count += 1
        // IF layer count % amount_mixes == 0 (got messages from everyone)
        /* ATOMICALLY: 
            verify all messages in buffer (at first one by one, fix later),
            mix buffer,
            send messages to next mix using AddMessages rpc 
            UNLESS it's last layer, then use getMessages to return to coordinator
        */ 

    
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>
    ) -> Result<Response<Self::GetMessagesStream>, Status> {
        // This function is called when all output buffers are ready
        // Send in a stream all packets,
        std::unimplemented!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    // in command line argument get my name/ID, look for my config file according to my id.
    let mix_id = env::args().nth(1).expect("Please specify the mix id as the first argument");
    let tmp_mix_id = mix_id.clone();
    let mix_id_int = mix_id.parse::<u64>().unwrap();

    // get ip address
    let my_ip = local_ip().unwrap();
    let addr = SocketAddr::new(my_ip, 3141);
    
    // write address to predetermined file "mix_id_addr.json"
    write_address_to_mix_file(mix_id, &addr);

    // get crypto info from predetermined file "mix_id_crypto.json"
    let crypto = get_crypto_info_from_file(tmp_mix_id);

    // get network info from predetermined file "network.json"
    let network_str : &str = "network.json";
    let network : Network = serde_json::from_str(&network_str).unwrap();

    // Wait for others to write to their files and then fetch ip's to connect to
    let mut channels = Vec::new();
    thread::sleep(time::Duration::from_secs(5));
    create_mixnet_channels(&mut channels, network.size);

    // make new service, wait a few seconds to ensure all servers managed to load and listen for clients
    // NOTE: output_buffer is of size size, it can be of size-1 but for simplicity I chose otherwise
    let mix_service = MixService {
        address: addr,
        crypto: crypto,
        network: network,
        connections: channels,
        my_id: mix_id_int,
        recv_count: 0,
        output_buffer: (0..network.size).map(|_| Vec::new()).collect(),
    };

    Server::builder().add_service(MixServer::new(mix_service)).serve(addr).await?;
    thread::sleep(time::Duration::from_secs(5));

    Ok(())
}


fn write_address_to_mix_file(mut mix_id: String, addr: &SocketAddr){
    let addr_str: &str = "_addr.json";
    mix_id.push_str(addr_str);
    std::fs::write(
        mix_id,
        serde_json::to_string_pretty(&addr).unwrap(),
    ).unwrap();
}

fn get_crypto_info_from_file(mut mix_id: String) -> crypto_server {
    let addr_str: &str = "_addr.json";
    let crypto_str: &str = "_crypto.json";
    mix_id.push_str(crypto_str);
    let crypto_str = std::fs::read_to_string(mix_id).unwrap();
    let crypto: crypto_server = serde_json::from_str(&crypto_str).unwrap();
    return crypto;
}

fn create_mixnet_channels(channels: &mut Vec<Channel>, network_size: u64){
    let addr_str: &str = "_addr.json";
    for i in 0..network_size {
        let mut mix_id = i.to_string();
        mix_id.push_str(addr_str);
        let cur_mix_addr_str = std::fs::read_to_string(mix_id).unwrap();
        let cur_mix_addr: SocketAddr = serde_json::from_str(&addr_str).unwrap();
        let channel = Channel::from_shared(cur_mix_addr.to_string()).unwrap().connect_lazy();
        channels.push(channel);
    }
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