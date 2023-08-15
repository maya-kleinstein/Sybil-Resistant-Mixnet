use serde::{Serialize, Deserialize};

use crate::{
    network::{Network, Client, Server, IDProvider, generate_bad_packet, generate_packet, ticket_server_map_generator},
    config::{ConfigInfo, MixnetVerification},
    mix::decrypt_incoming_packets,
    ToVariableLengthBytes
};
use std::{fs::File, convert::TryInto};
use std::io::{Read, Write};

use crate::keys::PublicKey;
use crate::keys::SecretKey;

/// The base folder for all files
pub const BASE_FOLDER: &str = "";

/*
To later decrypt + run through mixes we need this crypto info: layer, mix id + key
To verify validity in every mix we also need: generic network info
*/ 

/// Write all heavy computation data to predetermined files
pub fn setup_files(config_info: ConfigInfo){
    // Write config data to file
    let filename = "config_info";
    serialize_info_to_file::<ConfigInfo>(&config_info, filename).unwrap();

    // Generate all data needed to test the mixnet
    let network = Network::new(
        config_info.num_mixes.into(), 
        config_info.num_layers,
        config_info.mix_verification
    );
    
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); config_info.num_mixes.into()];

    // get ticket server mapping
    let mapping = ticket_server_map_generator(config_info.num_mixes.into());
    let mut bad_tickets_vec = vec![];
    for i in 0..config_info.num_layers {
        bad_tickets_vec.push(mapping.get(&(i % 2)).unwrap().clone());
    }

    for i in 0..config_info.num_clients {
        let data = vec![i as u8; 3];
        let client = Client::new(&network);
        let (packet, first_server): (Vec<u8>, u64);
        if i < ((config_info.num_clients as f64) * config_info.percentage_bad_clients) as u64 {
            (packet, first_server) = generate_bad_packet(data, &client, &network, &bad_tickets_vec);
        }
        else {
            (packet, first_server) = generate_packet(data, &client, &network);
        }
        packets[first_server as usize].push(packet);
    }
    
    // Write packets to intended files
    for i in 0..config_info.num_mixes {
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


pub fn process_init_packets(
    init_packets: Vec<Vec<u8>>, 
    config_info: &ConfigInfo,
    network: &Network, 
    id: u16, 
    layer: u64
) -> Vec<Vec<Vec<u8>>> {
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); config_info.num_mixes.into()];
    
    let dec_packets = decrypt_incoming_packets(init_packets, id, layer as u32, network);

    // Insert decrypted packets to output_buffer
    for (dec_packet, next) in dec_packets {
        packets[next as usize].push(dec_packet.data);
    } 
    return packets;
}

pub fn get_network_info() -> Network {
    let filename = "network";
    let network: Network = deserialize_network(filename).unwrap();
    return network;
}

pub fn get_config_info() -> ConfigInfo {
    let filename = "config_info";
    let config_info: ConfigInfo = deserialize_info_from_file(&filename).unwrap();
    return config_info;
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
    pub num_layers: u64,
    pub mix_verification: MixnetVerification,
    pub servers: Vec<Server>,
}

pub fn serialize_network(data: &Network, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let serial_network = SerialNetwork {
        serial_id_provider_0: data.id_provider.bbs_keys.0.to_bytes_compressed_form(),
        serial_id_provider_1: data.id_provider.bbs_keys.1.to_bytes_compressed_form().to_vec(),
        sys_rand: data.sys_rand,
        round_id: data.round_id,
        size: data.size,
        num_layers: data.num_layers,
        mix_verification: data.mix_verification,
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
        num_layers: serial_network.num_layers,
        mix_verification: serial_network.mix_verification,
        servers: serial_network.servers, 
    };
    return Ok(network);
}