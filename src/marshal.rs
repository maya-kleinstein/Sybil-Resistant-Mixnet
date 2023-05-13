use crate::{network::{Network, Client, generate_packet}, config::{NUM_MIXES, NUM_CLIENTS}};
use std::fs::File;
use std::io::{Read, Write};
use serde::{Serialize, Deserialize};
use serde_json;

/// The base folder for all files
pub const BASE_FOLDER: &str = "";

/*
To later decrypt + run through mixes we need: layer, mix id + key
To verify validity in every mix also needs: network generic info
    */ 

/// Write all heavy computation data to predetermined files
pub fn setup_files(){
    // Generate all data needed to test the mixnet
    let network = Network::new(NUM_MIXES.into());
    let mut clients: Vec<Client> = Vec::new();
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); NUM_MIXES.into()];
    for _ in 0..NUM_CLIENTS {
        let data = vec![0x91, 0x92, 0x93];
        let client = Client::new(&network);
        let (packet, first_server) = generate_packet(data, &client, &network);
        clients.push(client);
        packets[first_server as usize].push(packet);
    }
    
    // Write packets to intended files
    for i in 0..NUM_MIXES {
        let filename = format!("packets_{}", i);
        serialize_info_to_file(&packets[i as usize], &filename).unwrap();
    }

    let filename = "network";
    serialize_info_to_file(&network, &filename).unwrap();
}

pub fn get_init_packets(mix_id: u16) -> Vec<Vec<u8>>{
    let filename = format!("packets_{}", mix_id);
    let packets: Vec<Vec<u8>> = deserialize_info_from_file(&filename).unwrap();
    return packets;
}

pub fn get_network_info() -> Network {
    let filename = "network";
    deserialize_info_from_file(&filename).unwrap()
}

fn serialize_info_to_file<T: Serialize>(data: &T, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = format!("{}{}", BASE_FOLDER, filename);
    let json = serde_json::to_string(data)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

fn deserialize_info_from_file<T: for<'a> Deserialize<'a>>(filename: &str) -> Result<T, Box<dyn std::error::Error>> {
    let path = format!("{}{}", BASE_FOLDER, filename);
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let data: T = serde_json::from_str(&contents)?;
    Ok(data)
}