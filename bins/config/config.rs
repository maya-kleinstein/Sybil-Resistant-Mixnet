use std::net::IpAddr;

use bbs::{config::*, marshal::ips::*, marshal::logs::*};

#[tokio::main]
pub async fn main() -> Result<(), &'static str> {
    let mix_ips: Vec<IpAddr>;
    let remote_arg = std::env::args().nth(1).expect("no remote classifier given");
    init_logger("config").unwrap();

    match remote_arg.as_str() {
        "remote" => mix_ips = get_all_ips_from_files().unwrap(),
        "local" => {
            mix_ips = vec!["127.0.0.1".parse::<IpAddr>().unwrap(); *NUM_MIXES as usize];
        }
        _ => {
            return Err("You didn't specify a remote classifier, please specify Remote OR Local");
        }
    }
    run_config(mix_ips).await;
    Ok(())
}
