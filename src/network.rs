use crate::prelude::{*, PublicKey};
use std::convert::TryInto;
use blake2::Blake2b;
use pairing_plus::{CurveProjective, CurveAffine};
use pairing_plus::bls12_381::{G1, Fr, G1Uncompressed};
use pairing_plus::hash_to_curve::HashToCurve;
use pairing_plus::hash_to_field::ExpandMsgXmd;
use rand_4net::Rng;
use rsa::{RsaPrivateKey, RsaPublicKey, PaddingScheme};
use rsa::PublicKey as PublicKeyForRSAEnc;
use sodiumoxide::crypto::secretbox;
use serde::{Serialize, Deserialize};
use blake2::VarBlake2b;
use blake2::digest::{Input, VariableOutput};
use std::time::Instant;

/// Network module
/// Contains network related functionality
/// Entities Included: ID provider, Server, Client, Tickets, etc.

/// IDprovider configuration
pub struct IDProvider{
    keys: (PublicKey, SecretKey),
}

/// Server configuration
#[derive(Clone)]
pub struct Server{
    rsa_keys: (RsaPublicKey, RsaPrivateKey),
}

impl Server {
    #[cfg(test)]
    fn new() -> Server {
        Server {
            rsa_keys: generate_rsa_keys(),
        }
    }
}

/// Client configuration
pub struct Client {
    signature: Signature,
}

// Ticket configuration
#[derive(Serialize, Deserialize)]
struct TicketValues{
    layer: u64,
    round_id: u32,
    sys_rand: i32,
}

/// Packet configuration
#[derive(Serialize, Deserialize)]
pub struct Packet{
    ticket: Vec<u8>,
    proof: Vec<u8>, 
    data: Vec<u8>,
}

/// Network configuration
pub struct Network{
    id_provider: IDProvider,
    sys_rand: i32,
    round_id: u32,
    size: u64,
    servers: Vec<Server>,
}


//TODO: eventually take care of private-public values (e.g. Network can see all private keys)

/// Generate a packet from the client to the network with the given data
pub fn generate_packet(data: Vec<u8>, client: &Client, network: &Network) -> (Vec<u8>, u64){
    let mut data: Vec<u8> = data;
    let mut x: u64 = 0;
    let proof_messages = vec![
        pm_revealed!(b"Testing"),
    ];
    // Generating Fiat Shamir Signature PoK
    let sig_pok = PoKOfSignature::init(&client.signature, &network.id_provider.keys.0, proof_messages.as_slice()).unwrap();
    let challenge_prover = ProofChallenge::hash(&sig_pok.to_bytes());
    let proof = sig_pok.gen_proof(&challenge_prover).unwrap();
    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in (0..network.size-1).rev(){
        // t = b^s, where b=H0(layer, RoundID, SysRand) and s is part of signature
        let t_affine_uncompressed = calculate_ticket(i, network.round_id, network.sys_rand, client.signature.s);
        // server x = H(t) % network size, H: {0,1}^* -> Zp
        x = calculate_next_server(t_affine_uncompressed, network.size);
        // Generating PoK for t=b^s given Signature (A,e,s)       
        // TODO: Generate proof of knowledge for t=b^s as well!
        let packet = Packet{
            ticket: t_affine_uncompressed.as_ref().to_vec(),
            proof: proof.to_bytes(false),
            data,
        };
        let encoded_packet = bincode::serialize(&packet).unwrap();
        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        data = encrypt_packet(encoded_packet, &network.servers[x as usize].rsa_keys.0);
    }
    return (data, x);
}


/// Creating a new network of size size
pub fn create_network(network: &mut Network, size: u64){
    let keys: (PublicKey, SecretKey) = Issuer::new_keys(1).unwrap();

    let mut rng = rand_4net::thread_rng();
    let bits = 2048;
    let mut rsa_private_key:RsaPrivateKey;
    let mut rsa_public_key:RsaPublicKey;

    let id_provider = IDProvider{
        keys: keys,
    };

    network.id_provider = id_provider;
    network.sys_rand = rand_4net::thread_rng().gen();
    network.round_id = 0;
    
    for i in 0..size{
        rsa_private_key = RsaPrivateKey::new(&mut rng, bits).unwrap();
        rsa_public_key = RsaPublicKey::from(&rsa_private_key);
    
        let server = Server{
            rsa_keys: (rsa_public_key, rsa_private_key),
        };
        network.servers[i as usize] = server;
    }
}


// Calculating ticket = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
fn calculate_ticket(layer: u64, round_id: u32, sys_rand: i32, s: Fr)-> G1Uncompressed{
    let ticket_vals = TicketValues{
        layer,
        round_id,
        sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let mut b =  h_0(ticket_vals_bytes);
    b.mul_assign(s);

    let t_affine = b.into_affine();
    return t_affine.into_uncompressed();
}


// Calculating next server x from ticket
fn calculate_next_server(t_affine_uncompressed: G1Uncompressed, size: u64)->u64{
    let mut t = t_affine_uncompressed.as_ref();

    // server x = H(t), H: {0,1}^* -> Zp
    let mut hasher = VarBlake2b::new(8).unwrap();
    hasher.input(&mut t); // TODO: add constant string to beginning of hash
    let buf = hasher.vec_result();
    let x = u64::from_be_bytes(buf.as_slice().try_into().unwrap()) % size;
    return x;
}


// H0: {0,1}^* -> G1
fn h_0<I: AsRef<[u8]>>(data: I) -> G1 {
    const DST: &[u8] = b"BLS12381G1_XMD:BLAKE2B_SSWU_RO_BBS+_SIGNATURES:ANONYMOUS_MIXNETS:1_0_0";
    <G1 as HashToCurve<ExpandMsgXmd<Blake2b>>>::hash_to_curve(data.as_ref(), DST)
}


// Generating RSA keys
#[cfg(test)]
fn generate_rsa_keys() -> (RsaPublicKey, RsaPrivateKey) {
    let mut rng = rand_4net::thread_rng();
    let bits = 2048;
    let rsa_private_key = RsaPrivateKey::new(&mut rng, bits).unwrap();
    let rsa_public_key = RsaPublicKey::from(&rsa_private_key);
    return (rsa_public_key, rsa_private_key);
}


// Encrypting packet using both asymmetric and symmetric encryption, 128bit secure
fn encrypt_packet(encoded_data: Vec<u8>, pub_key: &RsaPublicKey) -> Vec<u8>{
    // TODO: USE CRATE: log (or tracing) for BENCHMARKS
    // generate random symmetric key and encrypt data with it
    let key = secretbox::gen_key();
    let nonce = secretbox::gen_nonce();
    let mut ciphertext = secretbox::seal(encoded_data.as_ref(), &nonce, &key);

    // encrypt sym key using pub key
    let mut rng = rand_4net::thread_rng();
    let padding = PaddingScheme::new_pkcs1v15_encrypt();
    let mut enc_key = pub_key.encrypt(&mut rng, padding, &key.as_ref()[..]).expect("failed to encrypt");

    // TODO: fix this after decryption implementations...
    // add sym key to encrypted data
    ciphertext.append(&mut enc_key);
    return ciphertext;
    // TODO: https://doc.libsodium.org/public-key_cryptography/sealed_boxes, should be able to do 1000's per sec (Yossi says)
}


mod tests{
    use super::*;
 
    #[test]
    pub fn test_simple_network(){
        println!("about to create the skeleton for the network");

        let mut network = Network{
            id_provider: IDProvider{
                keys: Issuer::new_keys(1).unwrap(),
            },
            sys_rand: 0,
            round_id: 0,
            size: 10,
            servers: vec![Server::new(); 10],
        };

        println!("about to create the network entirely!");

        create_network(&mut network, 10);

        println!("done creating the network!");

        let messages = vec![
            SignatureMessage::hash(b"Testing"),
        ];
        let client = Client{ 
            signature: Signature::new(messages.as_slice(), &network.id_provider.keys.1, &network.id_provider.keys.0).unwrap(),
        };

        let bytes = vec![b'a', b'b', b'c'];

        println!("about to generate the packet, wish me luck!");

        let (data, first_server) = generate_packet(bytes, &client, &network);

        println!("{}, is the first server", first_server);
        println!("{}, is the length of the data", data.len());
        println!("data: {:?}", data);
    }
}
