use futures::future::join_all;

use crate::config::*;
use crate::mix::*;

/// Run mixnet locally
pub async fn run_system() {
    let mut tasks = vec![];
    for mix_id in 0..*NUM_MIXES {
        tasks.push(run_mix(mix_id));
    }

    futures::join!(
        async {
            join_all(tasks).await;
        },
        async {
            run_config().await;
        }
    );
}
