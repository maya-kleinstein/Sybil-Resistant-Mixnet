use crate::prelude::{*, PublicKey};
use std::convert::TryInto;
use blake2::Blake2b;
use dryoc::types::StackByteArray;
use pairing_plus::serdes::SerDes;
use pairing_plus::{CurveProjective, CurveAffine};
use pairing_plus::bls12_381::{G1, Fr};
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


impl Client {
    /// Generate new client for given network
    pub fn new(network: &Network) -> Client {
        let messages = vec![
            SignatureMessage::hash(b"Testing"),
        ];
        Client {
            signature: Signature::new(messages.as_slice(), &network.id_provider.bbs_keys.1, &network.id_provider.bbs_keys.0).unwrap(),
        }
    }
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


impl Network {
    /// Generate a network of size size
    pub fn new(size: u64) -> Network {
        let id_provider = IDProvider {
            bbs_keys: Issuer::new_keys(1).unwrap(),
        };
        let servers = vec![Server::new(); size.try_into().unwrap()];
        Network {
            id_provider,
            sys_rand: 0,
            round_id: 0,
            size,
            servers,
        }
    }
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
        let t = calculate_ticket(i, network.round_id, network.sys_rand, client.signature.s);
        
        // server x = H(t) % network size, H: {0,1}^* -> Zp
        x = calculate_next_server(t, network.size);
        // Generating PoK for t=b^s given Signature (A,e,s)       
        // TODO: Generate proof of knowledge for t=b^s as well!

        // Wrapping packet
        let mut t_buf: Vec<u8> = vec![];
        // serialize a t into buffer
        t.serialize(&mut t_buf, false).unwrap();

        let packet = Packet{
            ticket: t_buf,
            proof: proof.to_bytes_uncompressed_form(),
            data,
        };
        let encoded_packet = bincode::serialize(&packet).unwrap();
        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        let wrapped_data = DryocBox::seal_to_vecbox(&encoded_packet, &network.servers[x as usize].key_pair.public_key.clone()).expect("Unable to seal"); // DELETE
        data = bincode::serialize(&wrapped_data).unwrap();
    }
    return (data, x);
}


/// Decrypt a packet traversing through the network
pub fn decrypt_packet(enc_packet: Vec<u8>, x_0 :u64, network: &Network) -> Vec<u8>{
    let mut data = enc_packet;
    let mut x = x_0;
    for _ in 0..(network.size-1){
        // Decrypt Packet 
        let dryocbox : DryocBox<StackByteArray<32>, StackByteArray<16>, Vec<u8>> = bincode::deserialize(&data).unwrap();
        let decrypted = dryocbox.unseal_to_vec(&network.servers[x as usize].key_pair).expect("unable to decrypt");
        let packet: Packet = bincode::deserialize(&decrypted).unwrap();
        // Verify ticket and proof (done by x)
        verify_packet(packet.proof);
        // Retrieving data and next server 
        data = packet.data;
        // Calculating next server using the ticket
        let t_recovered = G1::deserialize(&mut packet.ticket[..].as_ref(), false).unwrap();
        x = calculate_next_server(t_recovered, network.size);
    }
    return data;
}


// Verify the proof of knowledge of the signature and the ticket
fn verify_packet(proof: Vec<u8>){
    // Verifying PoK of Signature
    let proof_cp = PoKOfSignatureProof::from_bytes_uncompressed_form(&proof);
    assert!(proof_cp.is_ok());
    // TODO: verify ticket generation
}


/// Creating a new network of size size
pub fn create_network(network: &mut Network, size: u64){
    let bbs_keys: (PublicKey, SecretKey) = Issuer::new_keys(0).unwrap();

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
fn calculate_ticket(layer: u64, round_id: u32, sys_rand: i32, s: Fr)-> G1{
    let ticket_vals = TicketValues{
        layer,
        round_id,
        sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let mut b =  h_0(ticket_vals_bytes);
    b.mul_assign(s);

    return b;
}


// Calculating next server x from ticket
fn calculate_next_server(t: G1, size: u64)->u64{
    let binding = t.into_affine().into_uncompressed();
    let mut t_affine = binding.as_ref();

    // server x = H(t), H: {0,1}^* -> Zp
    let mut hasher = VarBlake2b::new(8).unwrap();
    hasher.input(&mut t_affine); // TODO: add constant string to beginning of hash
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
 
    const TEST_NETWORK_SIZE: u64 = 5;

    #[test]
    pub fn test_simple_network(){
        let network = Network::new(TEST_NETWORK_SIZE);
        let client = Client::new(&network);
        let data = vec![b'a', b'b', b'c'];
        let (enc_data, first_server) = generate_packet(data, &client, &network);

        println!("{}, is the first server", first_server);
        println!("{}, is the length of the data", enc_data.len());
        println!("enc_data: {:?}", enc_data);

        let dec_data = decrypt_packet(enc_data, first_server, &network);

        println!("dec_data: {:?}", dec_data);
    }
}
