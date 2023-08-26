use crate::{
    config::*,
    keys::{PublicKey, SecretKey},
    mix::decrypt_incoming_packets,
    network::{
        generate_bad_packet, generate_packet, ticket_server_map_generator, Client, IDProvider,
        Network, Server,
    },
    ToVariableLengthBytes,
};
use chrono::NaiveDateTime;
use log::*;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::MAIN_SEPARATOR;
use std::thread::sleep;
use std::time::Duration;

/*
This file contains all functions related to marshalling data to and from files.
This includes: INFO, IPS, and LOGS files.
INFO: all pre-computed data
IPS: all IP addresses for initial setup
LOGS: all logs from runs
*/

lazy_static! {
    /// The base folder for all files
    pub static ref BASE_FOLDER: String = format!("data{}", MAIN_SEPARATOR);
    /// The folder for all IP files
    pub static ref IPS_FOLDER: String = format!("ips{}", MAIN_SEPARATOR);
    /// The folder for all info files
    pub static ref INFO_FOLDER: String = format!("info{}", MAIN_SEPARATOR);
    /// The folder for all logs
    pub static ref LOGS_FOLDER: String = format!("logs{}", MAIN_SEPARATOR);
}

pub fn serialize_data_to_file<T: Serialize>(
    data: &T,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = format!("{}{}", *BASE_FOLDER, filename);
    let json = serde_json::to_string::<T>(data)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn deserialize_data_from_file<T: for<'a> Deserialize<'a>>(
    filename: &str,
) -> Result<T, serde_json::Error> {
    let path = format!("{}{}", *BASE_FOLDER, filename);
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let result: Result<T, serde_json::Error> = serde_json::from_str::<T>(&(contents.as_str()));
    return result;
}

/*
INFO management
*/

/// Write all heavy computation info to predetermined files
pub fn setup_info(config_info: ConfigInfo) {
    // Write config data to file
    let filename = format!("{}config_info", *INFO_FOLDER);
    serialize_data_to_file::<ConfigInfo>(&config_info, &filename).unwrap();

    // Generate all data needed to test the mixnet
    let network = Network::new(
        config_info.num_mixes.into(),
        config_info.num_layers,
        config_info.mix_verification,
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
        } else {
            (packet, first_server) = generate_packet(data, &client, &network);
        }
        packets[first_server as usize].push(packet);
    }

    // Write packets to intended files
    for i in 0..config_info.num_mixes {
        let filename = format!("{}packets_{}", *INFO_FOLDER, i);
        serialize_data_to_file::<Vec<Vec<u8>>>(&packets[i as usize], &filename).unwrap();
    }

    let filename = "network_info";
    //serialize_info_to_file::<Network>(&network, filename).unwrap();
    serialize_network(&network, filename).unwrap();
}

pub fn get_init_packets(mix_id: u16) -> Vec<Vec<u8>> {
    let filename = format!("{}packets_{}", *INFO_FOLDER, mix_id);
    let packets: Vec<Vec<u8>> = deserialize_data_from_file(&filename).unwrap();
    return packets;
}

pub fn process_init_packets(
    init_packets: Vec<Vec<u8>>,
    network: &Network,
    id: u16,
    layer: u64,
) -> Vec<Vec<Vec<u8>>> {
    let mut packets: Vec<Vec<Vec<u8>>> = vec![vec![].into(); Into::<usize>::into(*NUM_MIXES)];

    let dec_packets = decrypt_incoming_packets(init_packets, id, layer as u32, network);

    // Insert decrypted packets to output_buffer
    for (dec_packet, next) in dec_packets {
        packets[next as usize].push(dec_packet.data);
    }
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
        servers: serial_network.servers,
    };
    return Ok(network);
}

/*
LOGS management
*/
/// Initializes a logger that outputs everything to both stdout and the file at file_path
pub fn init_logger(file_path: &str) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}]{}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file(format!(
            "{}{}{}",
            *BASE_FOLDER, *LOGS_FOLDER, file_path
        ))?)
        .chain(std::io::stdout())
        .level(LevelFilter::Off)
        .level_for("bbs", LevelFilter::Trace)
        .apply()?;
    Ok(())
}

/*
NOTES:
Below are functions that we'll eventually need in order to generate final logs and delete unnecessary files
at the end of a run.
Config will run them through a "manage_files()" function that will run these in the following order:
- rename_ip_logs: rename all IP logs to Mix ID logs
- merge_log_files: merge all log files into one (in order of timestamps)
- delete_ip_files: delete all files in date\ips
- delete_old_log_files: delete all log files that AREN'T merged ones (they'll have a format for there name)

    WRITE TESTS FOR THIS!!!!!
*/

/// Rename IP logs to Mix ID logs
pub fn rename_ip_logs() {}

/// Merge all log files into one
pub fn merge_log_files(filenames: Vec<&str>) {
    // Read each file, parse the timestamp, and push the items into the heap.
    for (file_index, file_path) in filenames.iter().enumerate() {
        let file = File::open(file_path).unwrap();
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.unwrap();
            let timestamp_str = &line[1..24];
            let format = "%Y-%m-%d %H:%M:%S%.3f";
            let timestamp = NaiveDateTime::parse_from_str(timestamp_str, format).unwrap();
        }
    }
}

/// Delete all files in date\ips
pub fn delete_ip_files() {
    let paths = fs::read_dir(format!("{}{}", *BASE_FOLDER, *IPS_FOLDER)).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        fs::remove_file(path).unwrap();
    }
}

/*
IP management
*/

/// Get's all mixes IP's sorted, and my mix's index
pub fn init_mix_ips() -> io::Result<(Vec<IpAddr>, u16)> {
    let my_ip = write_my_ip_to_file()?;
    let mut ips = get_all_ips_from_files()?;

    ips.sort();
    let index = ips.iter().position(|&r| r == my_ip).unwrap();

    debug!("All IPs: {:?}", ips);
    debug!("My ID: {}", index);

    Ok((ips, index as u16))
}

pub fn get_all_ips_from_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips = get_cur_ip_files()?;
    while ips.len() as u16 != *NUM_MIXES {
        let missing_ips = format!("Could only find: {:?}", ips);
        warn!("{:?}", missing_ips);
        sleep(Duration::from_millis(10));
        ips = get_cur_ip_files()?;
    }
    Ok(ips)
}

pub fn write_my_ip_to_file() -> io::Result<IpAddr> {
    let my_ip = get_my_ip()?;
    let filename = format!("{}{}", *IPS_FOLDER, my_ip);
    serialize_data_to_file::<IpAddr>(&my_ip, &filename).unwrap();
    Ok(my_ip)
}

pub fn get_my_ip() -> io::Result<IpAddr> {
    // Connect to a public server to discover our external IP address
    let socket = TcpStream::connect("google.com:80")?;
    Ok(socket.local_addr()?.ip())
}

fn get_cur_ip_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = Vec::new();
    for entry in fs::read_dir(format!("{}{}", *BASE_FOLDER, *IPS_FOLDER))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name() {
                if let Some(filename_str) = filename.to_str() {
                    let ip = filename_str.parse::<IpAddr>();
                    if ip.is_ok() {
                        ips.push(ip.unwrap());
                    }
                }
            }
        }
    }
    Ok(ips)
}
