use bbs::mix::*;
use futures::TryFutureExt;
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let arg = std::env::args().nth(1).expect("no pattern given");
    let id: u16 = arg.parse().unwrap();
    run_mix(id);
}