use bbs::mix::*;
use bbs::config::*;
use futures::future::join_all;
use std::thread;


#[tokio::test]
async fn system_test(){
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


#[tokio::test]
async fn how_tf_tasks(){
    let mut tasks = Vec::new();

    for i in 0..20 {
        tasks.push(tokio::spawn(async move {
            println!("{}", i);
        }));
    }
    join_all(tasks).await;
}

#[test]
fn how_tf_threads(){
    let mut threads = Vec::new();
    for i in 0..20 {
        threads.push(thread::spawn( move || {
            println!("{}", i)
        }))
    }

    for t in threads {
        t.join().unwrap();
    }
}