use bbs::config::*;
use bbs::data_manager::*;

pub fn main() {
    let config_info = ConfigInfo {
        base_port: 8000,
        num_mixes: 2,
        num_clients: 100,
        percentage_bad_clients: 1.0,
        num_layers: 5,
        first_middle_layer: 2,
        mix_verification: MixnetVerification::NoVerification,
        num_rounds: 1,
        edge_limit: 0.3,
    };

    setup_info(config_info);
}
