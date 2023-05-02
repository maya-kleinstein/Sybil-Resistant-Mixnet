use mix_client::mix_client::MixClient;
use mix_client::GetRequest;

/// The port for the first mix
pub const BASE_PORT: u16 = 50590;
/// The number of mixes
pub const NUM_MIXES: u16 = 3;

pub mod mix_client {
    tonic::include_proto!("mix");
}

pub async fn run_config() -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES{
        let mut mix = MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await?;
        println!("CONFIG connected to mix {}", i);
        let request = tonic::Request::new(GetRequest {});
        tasks.push(tokio::spawn(async move {
            let response = mix.get(request);
            response.await
        }));
    }

    for task in tasks {
        let response = task.await.unwrap();
        println!("CONFIG recvd response: {:?}", response);
    }

    Ok(())
}