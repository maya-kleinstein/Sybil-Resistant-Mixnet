use crate::mix::mix_service::GetRequest;
use crate::mix::connect_to_server;
use tonic::Request;
use futures::future::join_all;
use statrs::distribution::{Binomial, DiscreteCDF};
use serde::{Serialize, Deserialize};

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
pub enum MixnetVerification{
    NoVerification,
    Verify,
    BatchVerify,
    OnlyVerifyEdgeCases,
}


pub async fn run_config(config_info: ConfigInfo){
    let mut tasks = Vec::with_capacity(config_info.num_mixes.into());
    for i in 0..config_info.num_mixes {
        let mut mix = connect_to_server(&config_info, i).await;
        println!("CONFIG connected to mix {}", i);
        let task = tokio::spawn(async move {
            let request = Request::new(GetRequest {});
            let response = mix.get(request).await.unwrap().into_inner();
            println!("Config recv'd get response from mix {} of size: {}", i, response.messages.len());
        });
        tasks.push(task);
    }
    join_all(tasks).await;
}

/// Returns if the amount of packets "i" is to be considered questionable 
pub fn is_out_of_bounds(config_info: &ConfigInfo, i: usize, total: usize) -> bool {
    let p = (1 as f64)/(config_info.num_mixes as f64);
    let binomial = Binomial::new(p, total as u64).unwrap();
    // cdf = Prob(Bin(n,p) <= i)
    let cdf = binomial.cdf(i as u64);
    let result = (1 as f64 - cdf) < config_info.edge_limit && i > total/(config_info.num_mixes as usize);
    if result {
        println!("OVER BOUND! number of packets: {}, total outgoing: {}, probability: {}",
            i, total, (1 as f64 - cdf)
        );
    }
    result
}