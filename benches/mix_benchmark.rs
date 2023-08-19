use criterion::*;
//use bbs::mixnet::run_system;

// IMPORTANT NOTE TODO: this doesn't work, no idea why...oof
/// Trying to bench system performance
pub fn system_bench(_c: &mut Criterion) {
    // let rt = tokio::runtime::Runtime::new().unwrap();
    // c.bench_function("run_system", move |b| {
    //     b.to_async(&rt).iter(|| async {
    //         black_box(run_system().await);
    //     } )
    // });
    ()
}

criterion_group! {
    name = mix_benches;
    config = Criterion::default().sample_size(10);
    targets = system_bench
}
criterion_main!(mix_benches);
