use serde::{Serialize, Deserialize};

use crate::{network::{Network, Client, generate_packet, decrypt_layer, Server, IDProvider}, config::{NUM_MIXES, NUM_CLIENTS}, G1_COMPRESSED_SIZE, ToVariableLengthBytes};
use std::{fs::File, convert::TryInto};
use std::io::{Read, Write};

use crate::keys::PublicKey;
use crate::keys::SecretKey;

/// The base folder for all files
pub const BASE_FOLDER: &str = "";

/*
To later decrypt + run through mixes we need this crypto info: layer, mix id + key
To verify validity in every mix we also need: network generic info
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
        serialize_info_to_file::<Vec<Vec<u8>>>(&packets[i as usize], &filename).unwrap();
    }

    let filename = "network";
    //serialize_info_to_file::<Network>(&network, filename).unwrap();
    serialize_network(&network, filename).unwrap();
}

pub fn get_init_packets(mix_id: u16) -> Vec<Vec<u8>>{
    let filename = format!("packets_{}", mix_id);
    let packets: Vec<Vec<u8>> = deserialize_info_from_file(&filename).unwrap();
    return packets;
}


pub fn process_init_packets(init_packets: Vec<Vec<u8>>, id: u16, network: &Network, layer: u64) -> Vec<Vec<Vec<u8>>> {
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); NUM_MIXES.into()];
    for packet in init_packets {
        let (dec_packet, next) = decrypt_layer(packet, id.into(), network, layer);
        packets[next as usize].push(dec_packet);
    }
    return packets;
}

pub fn get_network_info() -> Network {
    let filename = "network";
    let network: Network = deserialize_network(filename).unwrap();
    return network;
}

pub fn serialize_info_to_file<T: Serialize>(data: &T, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = format!("{}{}", BASE_FOLDER, filename);
    let json = serde_json::to_string::<T>(data)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn deserialize_info_from_file<T: for<'a> Deserialize<'a>>(filename: &str) -> Result<T, serde_json::Error> {
    let path = format!("{}{}", BASE_FOLDER, filename);
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    //println!("{}", contents.as_str());
    let result: Result<T, serde_json::Error> = serde_json::from_str::<T>(&(contents.as_str()));
    return result;
}

/*
The implementation of serde for PublicKey and SecretKey doesn't work,
Therefore below are wrapper functions to allow for marshalling of the Network struct specifically.
 */


#[derive(Debug, Serialize, Deserialize)]
pub struct SerialNetwork{
    pub serial_id_provider_0: Vec<u8>,
    pub serial_id_provider_1: Vec<u8>,
    pub sys_rand: i32,
    pub round_id: u32,
    /// Amount of servers in the network
    pub size: u64,
    pub servers: Vec<Server>,
}

pub fn serialize_network(data: &Network, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let serial_network = SerialNetwork {
        serial_id_provider_0: data.id_provider.bbs_keys.0.to_bytes_compressed_form(),
        serial_id_provider_1: data.id_provider.bbs_keys.1.to_bytes_compressed_form().to_vec(),
        // serial_id_provider_0: data.id_provider.bbs_keys.0.clone(),
        // serial_id_provider_1: data.id_provider.bbs_keys.1.clone(),
        sys_rand: data.sys_rand,
        round_id: data.round_id,
        size: data.size,
        servers: data.servers.clone(),
    };
    return serialize_info_to_file::<SerialNetwork>(&serial_network, filename);
}

pub fn deserialize_network(filename: &str) -> Result<Network, serde_json::Error> {
    let serial_network = deserialize_info_from_file::<SerialNetwork>(filename).unwrap();
    let secret_key: Result<[u8;32], _> = serial_network.serial_id_provider_1.as_slice().try_into();
    let network: Network = Network { 
        id_provider: IDProvider { 
            bbs_keys: (
                PublicKey::from_bytes_compressed_form(serial_network.serial_id_provider_0.as_slice()).unwrap(),
                SecretKey::from(secret_key.unwrap())
            )
            },
        sys_rand: serial_network.sys_rand,
        round_id: serial_network.round_id, 
        size: serial_network.size, 
        servers: serial_network.servers, 
    };
    return Ok(network);
}