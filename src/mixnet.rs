use futures::future::join_all;
use tokio::sync::oneshot;

use crate::mix::*;
use crate::config::*;

/// Run mixnet locally
pub async fn run_system() {
    let mut tasks = vec![];
    let mut sender_handles = vec![];
    for i in 0..NUM_MIXES {
        let (tx, rx) = oneshot::channel::<u8>();
        tasks.push(run_mix(i, rx));
        sender_handles.push(tx);
    }

    futures::join!(async {
        join_all(tasks).await;
    }, async {
        run_config(sender_handles).await;
    });
}