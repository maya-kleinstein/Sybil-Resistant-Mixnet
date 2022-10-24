use crate::prelude::{*, PublicKey};
use std::convert::TryInto;
use blake2::Blake2b;
use pairing_plus::{CurveProjective, CurveAffine};
use pairing_plus::bls12_381::{G1, Fr, G1Uncompressed};
use pairing_plus::hash_to_curve::HashToCurve;
use pairing_plus::hash_to_field::ExpandMsgXmd;
use rand_4net::Rng;
use serde::{Serialize, Deserialize};
use blake2::VarBlake2b;
use blake2::digest::{Input, VariableOutput};
use dryoc::dryocbox;
use dryoc::dryocbox::DryocBox;


/// Network module
/// Contains network related functionality
/// Entities Included: ID provider, Server, Client, Tickets, etc.



/// IDprovider configuration
pub struct IDProvider{
    bbs_keys: (PublicKey, SecretKey),
}

/// Server configuration
#[derive(Clone)]
pub struct Server{
    key_pair: dryocbox::KeyPair,
}

impl Server {
    fn new() -> Server {
        Server {
            key_pair: dryocbox::KeyPair::gen(),
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
    let sig_pok = PoKOfSignature::init(&client.signature, &network.id_provider.bbs_keys.0, proof_messages.as_slice()).unwrap();
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
        data = DryocBox::seal_to_vecbox(&encoded_packet, &network.servers[x as usize].key_pair.public_key.clone()).expect("Unable to seal").to_vec();
    }
    return (data, x);
}

/// Decrypt a packet traversing through the network
// pub fn decrypt_packet(packet: Vec<u8>, server: &Server, network: &Network) -> Vec<u8>{
    
// }


/// Creating a new network of size size
pub fn create_network(network: &mut Network, size: u64){
    let bbs_keys: (PublicKey, SecretKey) = Issuer::new_keys(1).unwrap();

    let id_provider = IDProvider{
        bbs_keys,
    };

    network.id_provider = id_provider;
    network.sys_rand = rand_4net::thread_rng().gen();
    network.round_id = 0;
    
    for i in 0..size{
        network.servers[i as usize] = Server::new();
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


mod tests{
    use super::*;
 
    const TEST_NETWORK_SIZE: u64 = 3;

    #[test]
    pub fn test_simple_network(){
        println!("about to create the skeleton for the network");

        let network = Network{
            id_provider: IDProvider{
                bbs_keys: Issuer::new_keys(1).unwrap(),
            },
            sys_rand: 0,
            round_id: 0,
            size: TEST_NETWORK_SIZE,
            servers: vec![Server::new(); TEST_NETWORK_SIZE.try_into().unwrap()],
        };

        println!("about to create client stuff");

        let messages = vec![
            SignatureMessage::hash(b"Testing"),
        ];
        let client = Client{ 
            signature: Signature::new(messages.as_slice(), &network.id_provider.bbs_keys.1, &network.id_provider.bbs_keys.0).unwrap(),
        };

        let bytes = vec![b'a', b'b', b'c'];

        println!("about to generate the packet, wish me luck!");

        let (data, first_server) = generate_packet(bytes, &client, &network);

        println!("{}, is the first server", first_server);
        println!("{}, is the length of the data", data.len());
        println!("data: {:?}", data);
    }
}
