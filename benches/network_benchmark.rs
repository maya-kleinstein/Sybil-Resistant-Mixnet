use bbs::network::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};


// Things I want to test: Time to generate a ticket, time to verify a ticket and signature, Time to decrypt a packet


fn generate_packet_benchmark(c: &mut Criterion) {
    let data = black_box(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let network = black_box(Network::new(5));
    let client = black_box(Client::new(&network));
    c.bench_function("generate_packet", |b| b.iter(|| generate_packet(data.clone(), &client, &network)));
}

criterion_group!(benches, generate_packet_benchmark);
criterion_main!(benches);