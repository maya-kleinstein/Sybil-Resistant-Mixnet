use bbs::config::*;
use bbs::marshal::*;

#[tokio::main]
pub async fn main() {
    let config_info = get_config_info();
    run_config(config_info).await
}
