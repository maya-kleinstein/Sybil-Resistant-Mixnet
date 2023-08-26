use std::net::IpAddr;

use futures::future::join_all;

use crate::config::*;
use crate::data_manager::init_logger;
use crate::mix::*;

/// Run mixnet locally
pub async fn run_system() {
    init_logger("local").unwrap();

    let mut tasks = vec![];
    for mix_id in 0..*NUM_MIXES {
        // these mix_ips would only work locally (obviously)
        let mix_ips = vec!["127.0.0.1".parse::<IpAddr>().unwrap(); *NUM_MIXES as usize];
        tasks.push(run_mix(mix_ips, mix_id));
    }

    futures::join!(
        async {
            join_all(tasks).await;
        },
        async {
            let mix_ips = vec!["127.0.0.1".parse::<IpAddr>().unwrap(); *NUM_MIXES as usize];
            run_config(mix_ips).await;
        }
    );
}
