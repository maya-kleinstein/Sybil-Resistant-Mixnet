use bbs::mix::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("no pattern given");
    let id: u16 = arg.parse().unwrap();
    run_mix(id).await;
    Ok(())
}