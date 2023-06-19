use crate::mix::mix_service::GetRequest;
use tonic::Request;

use futures::future::join_all;

use crate::mix::connect_to_server;

/// Choose the verification format for the mixnet
pub enum MixnetVerification{
    NoVerification,
    Verify,
    BatchVerify,
    VerifyEdgeCases,
}

/// The port for the first mix
pub const BASE_PORT: u16 = 50700;
/// The number of mixes
pub const NUM_MIXES: u16 = 2;
/// The number of expected clients
pub const NUM_CLIENTS: u64 = 10;
/// The number of layers in the mixnet
pub const NUM_LAYERS: u64 = 5;
/// The first "middle" layer
pub const FIRST_MIDDLE_LAYER : u32 = 2;
/// The mixnet verification type
pub const MIX_VERIFICATION: MixnetVerification = MixnetVerification::BatchVerify;
/// The number of rounds to run
pub const NUM_ROUNDS: u64 = 1;


pub async fn run_config(){
    let mut tasks = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES {
        let mut mix = connect_to_server(i).await;
        println!("CONFIG connected to mix {}", i);
        let task = tokio::spawn(async move {
            let request = Request::new(GetRequest {});
            let response = mix.get(request).await.unwrap().into_inner();
            println!("Config recv'd get response from mix {}: {:?}", i, response);
        });
        tasks.push(task);
    }
    join_all(tasks).await;
}