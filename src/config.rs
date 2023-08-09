use crate::mix::mix_service::GetRequest;
use crate::mix::connect_to_server;
use tonic::Request;
use futures::future::join_all;
use statrs::distribution::{Binomial, DiscreteCDF};


/// Choose the verification format for the mixnet
pub enum MixnetVerification{
    NoVerification,
    Verify,
    BatchVerify,
    OnlyVerifyEdgeCases,
}

/// The port for the first mix
pub const BASE_PORT: u16 = 50700;
/// The number of mixes
pub const NUM_MIXES: u16 = 2;
/// The number of expected clients
pub const NUM_CLIENTS: u64 = 100;
/// The percentage of malicious clients
pub const PERCENTAGE_BAD_CLIENTS: f64 = 1.0;
/// The number of layers in the mixnet
pub const NUM_LAYERS: u64 = 6;
/// The first "middle" layer
pub const FIRST_MIDDLE_LAYER : u32 = 2;
/// The mixnet verification type
pub const MIX_VERIFICATION: MixnetVerification = MixnetVerification::NoVerification;
/// The number of rounds to run
pub const NUM_ROUNDS: u64 = 1;
/// The percentage of cases to be considered "out of bounds" for edge OnlyVerifyEdgeCases
pub const EDGE_LIMIT: f64 = 0.3;


pub async fn run_config(){
    let mut tasks = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES {
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
    let p = (1 as f64)/(NUM_MIXES as f64);
    let binomial = Binomial::new(p, total as u64).unwrap();
    // cdf = Prob(Bin(n,p) <= i)
    let cdf = binomial.cdf(i as u64);
    let result = (1 as f64 - cdf) < EDGE_LIMIT && i > total/(NUM_MIXES as usize);
    if result {
        println!("OVER BOUND! number of packets: {}, total outgoing: {}, probability: {}",
            i, total, (1 as f64 - cdf)
        );
    }
    result
}