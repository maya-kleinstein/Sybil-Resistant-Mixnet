use crate::{
    keys::{PublicKey, SecretKey},
    marshal::*,
    network::{
        generate_bad_setup_packet, generate_data_packet, generate_setup_packet, ticket_server_map_generator, Client, IDProvider, Network, Server
    },
    ToVariableLengthBytes,
};
use serde::{Deserialize, Serialize};
use std::{convert::TryInto, sync::Arc, sync::Mutex as StdMutex};
use rayon::prelude::*;

/// Write all heavy computation info to predetermined files
pub fn setup_info() {
    let config_info: ConfigInfo = get_config_info();

    // Generate all data needed to test the mixnet
    let network = Network::new(
        config_info.num_mixes.into(),
        config_info.num_layers,
        config_info.mix_verification,
        config_info.is_proof_compressed,
    );

    let setup_packets: Arc<StdMutex<Vec<Vec<Vec<u8>>>>> = Arc::new(
        StdMutex::new(vec![vec![].into(); config_info.num_mixes.into()])
    );
    let data_packets: Arc<StdMutex<Vec<Vec<Vec<u8>>>>> = Arc::new(
        StdMutex::new(vec![vec![].into(); config_info.num_mixes.into()])
    );

    // get ticket server mapping
    let mapping = ticket_server_map_generator(config_info.num_mixes.into());
    let mut bad_tickets_vec = vec![];
    /* NOTE: notice all bad packets are generated with the same path
        The path is a zig-zag between mix server 0 and 1 (i%2)
        */
    let attacked_mix = (config_info.num_mixes - 1) as u64;
    for i in 0..config_info.num_layers {
        if i % 2 == 0 {
            bad_tickets_vec.push(mapping.get(&0).unwrap().clone());
        } else {
            bad_tickets_vec.push(mapping.get(&attacked_mix).unwrap().clone());
        }
    }

    let num_bad_clients = ((config_info.num_clients as f64) * config_info.percentage_bad_clients) as u64;

    (0..config_info.num_clients).into_par_iter().for_each(|i| {
        let data = vec![i as u8; config_info.data_size as usize];
        let mut client = Client::new(&network);
        let (setup_packet, first_server): (Vec<u8>, u64);
        if i < num_bad_clients {
            println!("Generating bad packet {}", i);
            (setup_packet, first_server) = generate_bad_setup_packet(&mut client, &network, &bad_tickets_vec);
        } else {
            println!("Generating packet {}", i);
            (setup_packet, first_server) = generate_setup_packet(&mut client, &network);
        }
        let packet = generate_data_packet(data, &client, &network);

        // Safely update shared data
        {
            let mut setup_packets_lock = setup_packets.lock().unwrap();
            setup_packets_lock[first_server as usize].push(setup_packet);
        }
        {
            let mut packets_lock = data_packets.lock().unwrap();
            packets_lock[first_server as usize].push(packet);
        }
    });

    // Write packets to files
    let setup_packets = Arc::try_unwrap(setup_packets).unwrap().into_inner().unwrap();
    let packets = Arc::try_unwrap(data_packets).unwrap().into_inner().unwrap();

    for i in 0..config_info.num_mixes {
        let setup_packets_filename = format!("{}setup_packets_{}", *INFO_FOLDER, i);
        serialize_data_to_file::<Vec<Vec<u8>>>(&setup_packets[i as usize], &setup_packets_filename).unwrap();
        let data_packets_filename = format!("{}data_packets_{}", *INFO_FOLDER, i);
        serialize_data_to_file::<Vec<Vec<u8>>>(&packets[i as usize], &data_packets_filename).unwrap();
    }

    let network_filename = "network_info";
    //serialize_info_to_file::<Network>(&network, filename).unwrap();
    serialize_network(&network, network_filename).unwrap();
}

pub fn get_init_setup_packets(mix_id: u16) -> Vec<Vec<u8>> {
    let filename = format!("{}setup_packets_{}", *INFO_FOLDER, mix_id);
    let packets: Vec<Vec<u8>> = deserialize_data_from_file(&filename).unwrap();
    return packets;
}

pub fn get_init_data_packets(mix_id: u16) -> Vec<Vec<u8>> {
    let filename = format!("{}data_packets_{}", *INFO_FOLDER, mix_id);
    let packets: Vec<Vec<u8>> = deserialize_data_from_file(&filename).unwrap();
    return packets;
}

pub fn get_network_info() -> Network {
    let filename = "network_info";
    let network: Network = deserialize_network(filename).unwrap();
    return network;
}

pub fn get_config_info() -> ConfigInfo {
    let filename = format!("{}config_info", *INFO_FOLDER);
    let config_info: ConfigInfo = deserialize_data_from_file(&filename).unwrap();
    return config_info;
}

pub fn write_config_info(config_info: ConfigInfo) {
    // Write config data to file
    let filename = format!("{}config_info", *INFO_FOLDER);
    serialize_data_to_file::<ConfigInfo>(&config_info, &filename).unwrap();
}

/*
The implementation of serde for PublicKey and SecretKey doesn't work,
Therefore below are wrapper functions to allow for marshalling of the Network struct specifically.
 */

#[derive(Debug, Serialize, Deserialize)]
pub struct SerialNetwork {
    pub serial_id_provider_0: Vec<u8>,
    pub serial_id_provider_1: Vec<u8>,
    pub sys_rand: i32,
    pub round_id: u32,
    /// Amount of servers in the network
    pub size: u64,
    /// Amount of layers in the network
    pub layers: u64,
    /// Verification type
    pub mix_verification: MixnetVerification,
    pub is_proof_compressed: bool,
    pub servers: Vec<Server>,
}

pub fn serialize_network(data: &Network, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let serial_network = SerialNetwork {
        serial_id_provider_0: data.id_provider.bbs_keys.0.to_bytes_compressed_form(),
        serial_id_provider_1: data
            .id_provider
            .bbs_keys
            .1
            .to_bytes_compressed_form()
            .to_vec(),
        sys_rand: data.sys_rand,
        round_id: data.round_id,
        size: data.size,
        layers: data.layers,
        mix_verification: data.mix_verification,
        is_proof_compressed: data.is_proof_compressed,
        servers: data.servers.clone(),
    };
    let filename = format!("{}{}", *INFO_FOLDER, filename);
    return serialize_data_to_file::<SerialNetwork>(&serial_network, &filename);
}

pub fn deserialize_network(filename: &str) -> Result<Network, serde_json::Error> {
    let filename = format!("{}{}", *INFO_FOLDER, filename);
    let serial_network = deserialize_data_from_file::<SerialNetwork>(&filename).unwrap();
    let secret_key: Result<[u8; 32], _> = serial_network.serial_id_provider_1.as_slice().try_into();
    let network: Network = Network {
        id_provider: IDProvider {
            bbs_keys: (
                PublicKey::from_bytes_compressed_form(
                    serial_network.serial_id_provider_0.as_slice(),
                )
                .unwrap(),
                SecretKey::from(secret_key.unwrap()),
            ),
        },
        sys_rand: serial_network.sys_rand,
        round_id: serial_network.round_id,
        size: serial_network.size,
        layers: serial_network.layers,
        mix_verification: serial_network.mix_verification,
        is_proof_compressed: serial_network.is_proof_compressed,
        servers: serial_network.servers,
    };
    return Ok(network);
}
