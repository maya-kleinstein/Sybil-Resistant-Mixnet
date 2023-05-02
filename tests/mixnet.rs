use bbs::mix::*;
use bbs::config::*;
use std::{thread, time};


#[tokio::test]
async fn system_test(){
    for i in 0..NUM_MIXES {
        run_mix(i).await.unwrap();
        thread::sleep(time::Duration::from_secs(1));
    }
    run_config().await.unwrap();
}