use bbs::mix::*;


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id_arg = std::env::args().nth(1).expect("no id given");
    let id: u16 = id_arg.parse().unwrap();
    run_mix(id).await;
    Ok(())
}