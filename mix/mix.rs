use bbs::mix::*;
use tokio::sync::oneshot;

// #[tokio::main]
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id_arg = std::env::args().nth(1).expect("no id given");
    let id: u16 = id_arg.parse().unwrap();
    let (tx, rx) = oneshot::channel::<u8>();
    // run_mix(id, rx).await;
    // return Ok(tx);
    Ok(())
}