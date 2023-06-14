use mix_client::mix_client::MixClient;
use mix_client::GetRequest;

// use crate::network::{self, Network};
use futures::future::join_all;

/// Choose the verification format for the mixnet
pub enum MixnetVerification{
    NoVerification,
    Verify,
    BatchVerify,
    VerifyEdgeCases,
}

/// The port for the first mix
pub const BASE_PORT: u16 = 50600;
/// The number of mixes
pub const NUM_MIXES: u16 = 2;
/// The number of expected clients
pub const NUM_CLIENTS: u64 = 10;
/// The number of layers in the mixnet
pub const NUM_LAYERS: u64 = 4;
/// The mixnet verification type
pub const MIX_VERIFICATION: MixnetVerification = MixnetVerification::Verify;
/// The number of rounds to run
pub const NUM_ROUNDS: u64 = 1;


pub mod mix_client {
    tonic::include_proto!("mix");
}

pub async fn run_config(){
    //sleep(Duration::from_secs(2)).await;
    let mut tasks = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES {
        let mut mix = MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await.unwrap();
        println!("CONFIG connected to mix {}", i);
        let task = tokio::spawn(async move {
            let request = tonic::Request::new(GetRequest {});
            let response = mix.get(request).await.unwrap();
            let x = response.into_inner().message().await.unwrap();
            for y in x.into_iter() {
                for z in y.messages {
                    println!("Config recv'd get response from mix {}: {:?}", i, z);
                }
            }
        });
        tasks.push(task);
    }
    join_all(tasks).await;
}