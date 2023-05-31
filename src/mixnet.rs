use futures::future::join_all;

use crate::mix::*;
use crate::config::*;

/// Run mixnet locally
pub async fn run_system() {
    let mut tasks = vec![];
    for i in 0..NUM_MIXES {
        tasks.push(run_mix(i));
    }

    futures::join!(async {
        join_all(tasks).await;
    }, async {
        run_config().await;
    });
}