use futures::future::join_all;

use crate::marshal::*;
use crate::mix::*;
use crate::config::*;

/// Run mixnet locally
pub async fn run_system() {
    let config_info = get_config_info();

    let mut tasks = vec![];
    for mix_id in 0..config_info.num_mixes {
        tasks.push(run_mix(config_info, mix_id));
    }

    futures::join!(async {
        join_all(tasks).await;
    }, async {
        run_config(config_info).await;
    });
}