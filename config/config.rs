use bbs::config::*;
use bbs::config::mix_client::mix_client::MixClient;
use bbs::config::mix_client::GetRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES{
        let mut mix = MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await?;
        println!("connected to mix {}", i);
        let request = tonic::Request::new(GetRequest {});
        tasks.push(tokio::spawn(async move {
            let response = mix.get(request);
            response.await
        }));
    }

    for task in tasks {
        let response = task.await.unwrap();
        println!("Response Config Recv'd: {:?}", response);
    }

    Ok(())
}
