use bbs::config::MixnetVerification::{self};
use bbs::network::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Things I want to test: Time to generate a ticket, time to verify a *ticket* and signature, Time to decrypt a packet

pub fn decrypt_setup_packet_layer_benchmark(c: &mut Criterion) {
    let network = black_box(Network::new(2, 3, MixnetVerification::NoVerification, true));
    let mut client = black_box(Client::new(&network));
    let (enc_data, first_server) = black_box(generate_setup_packet(&mut client, &network));  
    c.bench_function("decrypt_setup_packet_layer", |b| {
        b.iter(||  {
            decrypt_setup_layer(&enc_data, first_server, &network, 0);
        })
    });
}

pub fn decrypt_data_packet_layer_benchmark(c: &mut Criterion) {
    let data = black_box(vec![1; 128]);
    let network = black_box(Network::new(2, 3, MixnetVerification::NoVerification, true));
    let mut client = black_box(Client::new(&network));
    let mut conns = Connections::new(network.size);
    let (enc_data, first_server) = black_box(generate_setup_packet(&mut client, &network));  
    let _ = decrypt_setup_packet(enc_data, first_server, &network, &mut conns);
    let enc_data_packet = black_box(generate_data_packet(data, &client, &network));
    c.bench_function("decrypt_data_packet_layer", |b| {
        b.iter(||  {
            decrypt_data_packet_layer(&enc_data_packet, first_server, &conns, 0).unwrap();
        })
    });
}

criterion_group!(packet_benches, decrypt_data_packet_layer_benchmark, decrypt_setup_packet_layer_benchmark);
criterion_main!(packet_benches);
