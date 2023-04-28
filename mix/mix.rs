use std::thread::sleep;
use std::time;
use tonic::Request;
use bbs::mix::*;
use bbs::config::*;
use bbs::mix::mix_service::{mix_client::MixClient, AddRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("no pattern given");
    let id: u16 = arg.parse().unwrap();

    let mix = MyServer::new(id);

    println!("#### Start mix {} #####", id);

    let server_thread = run_service(mix);

    println!("#### Server Up mix {} #####", id);

    // Servers up and get requests recieved
    sleep(time::Duration::from_secs(10));

    for i in 0..NUM_MIXES {
        let mut client =
            MixClient::connect(format!("http://[::1]:{}", BASE_PORT + i)).await?;
        
        println!("#### Mix {} connected to {} mix #####", id, i);
        
        let add_req = vec![AddRequest { packets: vec![vec![0x01]] }];
        let _response = client.add(Request::new(futures::stream::iter(add_req.clone()))).await?;
    }
    
    server_thread.await.unwrap();
    Ok(())
}