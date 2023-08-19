use std::net::{IpAddr, TcpStream};
use std::thread::sleep;
use std::time::Duration;
use std::{fs, io};

use crate::marshal::serialize_info_to_file;
use crate::mix::connect_to_server;
use crate::{marshal::get_config_info, mix::mix_service::GetRequest};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tonic::Request;

lazy_static! {
    pub static ref CONFIG_INFO: ConfigInfo = get_config_info();
    pub static ref BASE_PORT: u16 = CONFIG_INFO.base_port;
    pub static ref NUM_MIXES: u16 = CONFIG_INFO.num_mixes;
    pub static ref NUM_CLIENTS: u64 = CONFIG_INFO.num_clients;
    pub static ref PERCENTAGE_BAD_CLIENTS: f64 = CONFIG_INFO.percentage_bad_clients;
    pub static ref NUM_LAYERS: u64 = CONFIG_INFO.num_layers;
    pub static ref FIRST_MIDDLE_LAYER: u32 = CONFIG_INFO.first_middle_layer;
    pub static ref MIX_VERIFICATION: MixnetVerification = CONFIG_INFO.mix_verification;
    pub static ref NUM_ROUNDS: u32 = CONFIG_INFO.num_rounds;
    pub static ref EDGE_LIMIT: f64 = CONFIG_INFO.edge_limit;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConfigInfo {
    pub base_port: u16,
    pub num_mixes: u16,
    pub num_clients: u64,
    pub percentage_bad_clients: f64,
    pub num_layers: u64,
    pub first_middle_layer: u32,
    pub mix_verification: MixnetVerification,
    pub num_rounds: u32,
    pub edge_limit: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
/// Choose the verification format for the mixnet
pub enum MixnetVerification {
    NoVerification,
    Verify,
    BatchVerify,
    OnlyVerifyEdgeCases,
}

pub async fn run_config() {
    let mut tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));
    for i in 0..*NUM_MIXES {
        let mut mix = connect_to_server(i).await;
        println!("CONFIG connected to mix {}", i);
        let task = tokio::spawn(async move {
            let request = Request::new(GetRequest {});
            let response = mix.get(request).await.unwrap().into_inner();
            println!(
                "Config recv'd get response from mix {} of size: {}",
                i,
                response.messages.len()
            );
        });
        tasks.push(task);
    }
    join_all(tasks).await;
}

/// Get's all mixes IP's sorted, and my mix's index
pub fn get_all_ips() -> io::Result<(Vec<IpAddr>, usize)> {
    let my_ip = get_my_ip()?;
    let filename = format!("{}", my_ip);
    serialize_info_to_file::<IpAddr>(&my_ip, &filename).unwrap();

    let mut ips = get_num_ip_files()?;
    while ips.len() as u16 != *NUM_MIXES {
        println!("Could only find: {:?}", ips);
        sleep(Duration::from_millis(10));
        ips = get_num_ip_files()?;
    }

    ips.sort();
    let index = ips.iter().position(|&r| r == my_ip).unwrap();

    println!("All IPs: {:?}", ips);
    println!("My ID: {}", index);

    Ok((ips, index))
}

fn get_my_ip() -> io::Result<IpAddr> {
    // Connect to a public server to discover our external IP address
    let socket = TcpStream::connect("google.com:80")?;
    Ok(socket.local_addr()?.ip())
}

fn get_num_ip_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = Vec::new();
    for entry in fs::read_dir(".")? {
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
