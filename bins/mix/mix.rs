use std::net::IpAddr;

use bbs::{config::*, data_manager::ips::*, data_manager::logs::*, mix::*};

#[tokio::main]
pub async fn main() -> Result<(), &'static str> {
    let id: u16;
    let mix_ips: Vec<IpAddr>;
    init_logger(&get_my_ip().unwrap().to_string()).unwrap();

    let remote_arg = std::env::args().nth(1).expect("no remote classifier given");
    match remote_arg.as_str() {
        "remote" => (mix_ips, id) = init_mix_ips().unwrap(),
        "local" => {
            let id_arg = std::env::args().nth(2).expect("no id given");
            id = id_arg.parse().unwrap();
            mix_ips = vec!["127.0.0.1".parse::<IpAddr>().unwrap(); *NUM_MIXES as usize];
        }
        _ => {
            return Err("You didn't specify a remote classifier, please specify Remote OR Local");
        }
    }

    run_mix(mix_ips, id).await;
    Ok(())
}
