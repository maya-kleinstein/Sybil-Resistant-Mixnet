use bbs::{mix::*, marshal::*};


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_info = get_config_info();
    let id_arg = std::env::args().nth(1).expect("no id given");
    let id: u16 = id_arg.parse().unwrap();
    run_mix(config_info, id).await;
    Ok(())
}