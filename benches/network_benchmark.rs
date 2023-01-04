use std::collections::{BTreeSet, BTreeMap};

use bbs::{network::*, SignatureMessage, HashElem};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Things I want to test: Time to generate a ticket, time to verify a *ticket* and signature, Time to decrypt a packet


fn generate_packet_benchmark(c: &mut Criterion) {
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(2));
    let client = black_box(Client::new(&network));
    c.bench_function("generate_packet", |b| b.iter(|| generate_packet(data.clone(), &client, &network)));
}


fn decrypt_packet_benchmark(c: &mut Criterion){
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(2));
    let client = black_box(Client::new(&network));
    let (enc_data, first_server) = generate_packet(data, &client, &network);
    c.bench_function("decrypt_packet", |b| b.iter(|| decrypt_packet(enc_data.clone(), first_server, &network)));
}


// NOTICE! that this is for a single layer, multiply by 3 to get time for full packet verification
fn verify_layer_benchmark(c: &mut Criterion){
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(2));
    let client = black_box(Client::new(&network));
    let (mut packet, _x) = generate_layer(data, &client, &network, 0);
    // Set up msg.'s info before decrypting
    let messages = black_box(vec![
        SignatureMessage::hash(b"Testing"),
    ]);
    
    let mut revealed_indices = black_box(BTreeSet::new());
    revealed_indices.insert(0);

    let mut revealed_msgs = black_box(BTreeMap::new());
    for i in &revealed_indices {
        revealed_msgs.insert(i.clone(), messages[*i]);
    }
    c.bench_function("verify_layer", |b| b.iter(|| verify_packet(&mut packet, &network, &revealed_msgs, 0)));
}


criterion_group!(benches, generate_packet_benchmark, decrypt_packet_benchmark, verify_layer_benchmark);
criterion_main!(benches);