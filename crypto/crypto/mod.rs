/// Number of bytes in scalar compressed form
pub const FR_COMPRESSED_SIZE: usize = 32;
/// Number of bytes in scalar uncompressed form
pub const FR_UNCOMPRESSED_SIZE: usize = 48;
/// Number of bytes in G1 X coordinate
pub const G1_COMPRESSED_SIZE: usize = 48;
/// Number of bytes in G1 X and Y coordinates
pub const G1_UNCOMPRESSED_SIZE: usize = 96;
/// Number of bytes in G2 X (a, b) coordinate
pub const G2_COMPRESSED_SIZE: usize = 96;
/// Number of bytes in G2 X(a, b) and Y(a, b) coordinates
pub const G2_UNCOMPRESSED_SIZE: usize = 192;

#[macro_use]
mod macros;
/// Proof messages
#[macro_use]
pub mod messages;
/// Macros and classes used for creating proofs of knowledge
#[macro_use]
pub mod pok_vc;
/// The errors that BBS+ throws
pub mod errors;
/// Represents steps taken by the issuer to create a BBS+ signature
/// whether its 2PC or all in one
pub mod issuer;
/// BBS+ key classes
pub mod keys;
/// Methods and structs for creating signature proofs of knowledge
pub mod pok_sig;
/// Represents steps taken by the prover to receive a BBS+ signature
/// and generate ZKPs
pub mod prover;
/// Methods and structs for creating signatures
pub mod signature;
/// Represents steps taken by the verifier to request signature proofs of knowledge
/// and selective disclosure proofs
pub mod verifier;
/// Represents Communication Network
pub mod network;
/// Methods and structs for creating tickets proof of knowledge
pub mod pok_ticket;