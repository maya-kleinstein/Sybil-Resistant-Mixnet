use crate::{network::{Network, Client, generate_packet}, config::{NUM_MIXES, NUM_CLIENTS}};

/// Write all heavy computation data to predetermined files
pub fn setup_files(){
    // Generate all data needed to test the mixnet
    let network = Network::new(NUM_MIXES.into());
    let mut clients: Vec<Client> = Vec::new();
    let mut packets: Vec<(Vec<u8>, u64)> = Vec::new();
    let mut client: Client;
    let mut packet: (Vec<u8>, u64);
    for _ in 0..NUM_CLIENTS {
        let data = vec![0x91, 0x92, 0x93];
        client = Client::new(&network);
        packet = generate_packet(data, &client, &network);
        clients.push(client);
        packets.push(packet);
    }
    
    // Write network, packets to 2 seperate files.
}


pub fn get_network(){

}

pub fn get_client_info(){

}