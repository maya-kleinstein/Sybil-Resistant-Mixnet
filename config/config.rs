use bbs::config::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_config().await
}
