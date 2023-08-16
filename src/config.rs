use crate::{mix::mix_service::GetRequest, marshal::get_config_info};
use crate::mix::connect_to_server;
use tonic::Request;
use futures::future::join_all;
use statrs::distribution::{Binomial, DiscreteCDF};
use serde::{Serialize, Deserialize};

lazy_static! {
    pub static ref CONFIG_INFO : ConfigInfo = get_config_info();
    
    pub static ref BASE_PORT : u16 = CONFIG_INFO.base_port;
    pub static ref NUM_MIXES : u16 = CONFIG_INFO.num_mixes;
    pub static ref NUM_CLIENTS : u64 = CONFIG_INFO.num_clients;
    pub static ref PERCENTAGE_BAD_CLIENTS : f64 = CONFIG_INFO.percentage_bad_clients;
    pub static ref NUM_LAYERS : u64 = CONFIG_INFO.num_layers;
    pub static ref FIRST_MIDDLE_LAYER : u32 = CONFIG_INFO.first_middle_layer;
    pub static ref MIX_VERIFICATION : MixnetVerification = CONFIG_INFO.mix_verification;
    pub static ref NUM_ROUNDS : u32 = CONFIG_INFO.num_rounds;
    pub static ref EDGE_LIMIT : f64 = CONFIG_INFO.edge_limit;
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
pub enum MixnetVerification{
    NoVerification,
    Verify,
    BatchVerify,
    OnlyVerifyEdgeCases,
}


pub async fn run_config(){
    let mut tasks = Vec::with_capacity(Into::<usize>::into(*NUM_MIXES));
    for i in 0..*NUM_MIXES {
        let mut mix = connect_to_server(i).await;
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
pub fn is_out_of_bounds(i: usize, total: usize) -> bool {
    let p = (1 as f64)/(*NUM_MIXES as f64);
    let binomial = Binomial::new(p, total as u64).unwrap();
    // cdf = Prob(Bin(n,p) <= i)
    let cdf = binomial.cdf(i as u64);
    let result = (1 as f64 - cdf) < *EDGE_LIMIT && i > total/(*NUM_MIXES as usize);
    if result {
        println!("OVER BOUND! number of packets: {}, total outgoing: {}, probability: {}",
            i, total, (1 as f64 - cdf)
        );
    }
    result
}