use bbs::network::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};


// Things I want to test: Time to generate a ticket, time to verify a *ticket* and signature, Time to decrypt a packet


fn generate_packet_benchmark(c: &mut Criterion) {
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(5));
    let client = black_box(Client::new(&network));
    c.bench_function("generate_packet", |b| b.iter(|| generate_packet(data.clone(), &client, &network)));
}


fn decrypt_packet_benchmark(c: &mut Criterion){
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(5));
    let client = black_box(Client::new(&network));
    let (enc_data, first_server) = generate_packet(data, &client, &network);
    c.bench_function("decrypt_packet", |b| b.iter(|| decrypt_packet(enc_data.clone(), first_server, &network)));
}

//TODO: note to self that this function will only be relevant after the crypto will be done
// fn verify_packet_benchmark(c: &mut Criterion){

// }



criterion_group!(benches, generate_packet_benchmark, decrypt_packet_benchmark);
criterion_main!(benches);