use mix_client::mix_client::MixClient;
use mix_client::GetRequest;
use tonic::transport::Channel;

const BASE_PORT: u16 = 50500;
const NUM_MIXES: u16 = 3;

pub mod mix_client {
    tonic::include_proto!("mix");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: error here cause the mix i+1 can only be "getted" after mix i
    let mut _mixes: Vec<MixClient<Channel>> = Vec::with_capacity(NUM_MIXES.into());
    for i in 0..NUM_MIXES{
        let mix = MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await?;
        _mixes.push(mix);
        println!("connected to mix {}", i);
        let request = tonic::Request::new(GetRequest {});
        let response = _mixes[i as usize].get(request);
        println!("Response from mix {} ={:?}", i, response.await?);
    }
    Ok(())
}
