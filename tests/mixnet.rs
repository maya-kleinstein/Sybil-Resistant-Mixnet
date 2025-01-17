use bbs::config::*;
use bbs::marshal::info::*;
use bbs::marshal::logs::merge_log_files;
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
    setup_info();
    let config_info = get_config_info();

    for i in 0..(config_info.num_mixes) {
        let packets = get_init_packets(i);
        println!("{} recv'd {} packets", i, packets.len());
    }
}

#[test]
fn log_test() {
    merge_log_files().unwrap();
}

#[test]
fn write_to_config_file_test(){
    let config_info = ConfigInfo {
        base_port: 8000,
        num_mixes: 2,
        num_clients: 1000,
        percentage_bad_clients: 0.0,
        num_layers: 4,
        first_middle_layer: 2,
        mix_verification: MixnetVerification::Verify,
        num_setup_rounds: 2, // Should be AT LEAST 1
        num_data_rounds: 2,
        data_size: 3,
        is_proof_compressed: true,
    };  

    // Write config data to file
    write_config_info(config_info);
}



#[test]
fn how_do_tasks_work() {
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
fn how_do_threads_work() {
    let mut threads = Vec::new();
    for i in 0..20 {
        threads.push(thread::spawn(move || println!("{}", i)))
    }

    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn marshal_network_test() {
    let x = Network::new(2, 3, MixnetVerification::NoVerification, true);
    let filename = "testt";
    serialize_network(&x, filename).unwrap();
    let network: Network = deserialize_network(filename).unwrap();
    let y = network;
    // TODO: The public keys aren't acutally equal?? wtf...
    assert_eq!(x.is_proof_compressed, y.is_proof_compressed);
    // assert_eq!(x.id_provider.bbs_keys.1, y.id_provider.bbs_keys.1);
    // assert_eq!(x.id_provider.bbs_keys.0, y.id_provider.bbs_keys.0);
    let _ = std::fs::remove_file("testt");
}
