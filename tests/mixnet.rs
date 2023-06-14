use bbs::config::*;
use bbs::marshal::*;
use bbs::mixnet::*;
use bbs::network::Network;
use futures::future::join_all;
use std::thread;
use tokio as tokio1;


#[test]
fn system_test() {
    tokio1::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            run_system().await;
        })
}


#[test]
fn marshalling_test() {
    setup_files();
    for i in 0..NUM_MIXES {
        let packets = get_init_packets(i);
        println!("{} recv'd {} packets", i, packets.len());
    }
}


#[test]
fn how_tf_tasks_work(){
    tokio1::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut tasks = Vec::new();

            for i in 0..20 {
                tasks.push(tokio::spawn(async move {
                    println!("{}", i);
                }));
            }
            join_all(tasks).await;
        })
}

#[test]
fn how_tf_threads_work(){
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


#[test]
fn marshal_network_test(){
    let x = Network::new(NUM_MIXES.into());
    let filename = "testt";
    serialize_network(&x, filename).unwrap();
    let network: Network = deserialize_network(filename).unwrap();
    let y = network;
    // TODO: The public keys aren't acutally equal?? wtf...
    assert_eq!(x.id_provider.bbs_keys.1, y.id_provider.bbs_keys.1);
    assert_eq!(x.id_provider.bbs_keys.0, y.id_provider.bbs_keys.0);
    let _ = std::fs::remove_file("testt");
}
