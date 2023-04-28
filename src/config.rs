/// The port for the first mix
pub const BASE_PORT: u16 = 50550;
/// The number of mixes
pub const NUM_MIXES: u16 = 3;

pub mod mix_client {
    tonic::include_proto!("mix");
}