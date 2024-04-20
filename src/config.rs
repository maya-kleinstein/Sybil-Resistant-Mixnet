use std::net::IpAddr;
use std::thread::sleep;
use std::time::Duration;

use crate::marshal::manage_files;
use crate::mix::{connect_to_server, get_edge_limit};
use crate::{marshal::info::get_config_info, mix::mix_service::GetRequest};
use futures::future::join_all;
use log::*;
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
    pub static ref EDGE_LIMIT: u64 = get_edge_limit(CONFIG_INFO.num_clients, CONFIG_INFO.num_mixes);
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
/// Choose the verification format for the mixnet
pub enum MixnetVerification {
    NoVerification,
    Verify,
    BatchVerify,
    OnlyVerifyEdgeCases,
}

pub async fn run_config(mix_ips: Vec<IpAddr>) {
    // Connect, send get request and recv get response from all mixes
    let mut tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));
    for i in 0..*NUM_MIXES {
        debug!("Connecting to mix {} at addr {}", i, mix_ips[i as usize]);
        let mut mix = connect_to_server(&mix_ips[i as usize], i).await;
        info!("Config connected to mix {}", i);
        let task = tokio::spawn(async move {
            let request = Request::new(GetRequest {});
            let response = mix.get(request).await.unwrap().into_inner();
            info!(
                "{}",
                format!(
                    "recv'd get response from mix {} of size: {}",
                    i,
                    response.messages.len()
                )
            );
            // TODO: check messages correctness
        });
        tasks.push(task);
    }
    join_all(tasks).await;

    sleep(Duration::from_secs(3));
    // Generate final log, clear all IP files
    manage_files();
}
