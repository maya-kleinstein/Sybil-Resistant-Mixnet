use crate::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use ff_zeroize::{Field, PrimeField};
use pairing_plus::bls12_381::{FrRepr, Fr};
use rand::rngs::ThreadRng;
use std::convert::{TryFrom, TryInto};
use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey, PaddingScheme};


#[deny(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unconditional_recursion,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_extern_crates,
    unused_parens,
    while_true,
    unused_results,
)]

/// Network module
/// Contains network related functionality
/// Entities Included: ID provider, Server, Client, Tickets, etc.

/// IDprovider configuration
pub struct IDProvider{
    keys: (PublicKey, SecretKey),
    sys_rand: i32,
}

/// Server configuration
pub struct Server{
    keys: (PublicKey, SecretKey),
    rsa_keys: (RsaPublicKey, RsaPrivateKey)
    id: u32,
}

/// Client configuration
pub struct Client {
    signature: Signature,
}

// Ticket configuration
#[derive(Hash)]
struct TicketValues{
    layer: u64,
    round_id: u32,
    sys_rand: i32,
}

/// Packet configuration
pub struct Packet{
    ticket: u64,
    proof: Vec<u8>, 
    data: Vec<u8>,
}

/// Network configuration
pub struct Network{
    id_provider: IDProvider,
    sys_rand: i32,
    round_id: u32,
    size: u64,
    servers: [Server],
}
//TODO: eventually take care of private-public values (e.g. Network can see all private keys)

/// Generate a packet from the client to the network with the given data
pub fn generate_packet(data: Vec<u8>, client: &Client, network: &Network) -> Packet{
    let mut packet: Packet;
    let mut cur_pk: &PublicKey;
    let mut ticket_vals: TicketValues;
    let mut b: u64;
    let mut t: u64;
    let mut data: Vec<u8>;
    let mut padding: PaddingScheme;
    let mut enc_data: &Vec<u8>;

    let s = (client.signature.s.into_repr().0[0] & 0x00000000FFFFFFFF).try_into().unwrap();
    
    let proof_messages = vec![
        pm_revealed!(b"I'm a valid user! Some ID number...?"),
    ];
    let rng = rand::thread_rng();

    let pok = PoKOfSignature::init(&client.signature, &network.id_provider.keys.0, proof_messages.as_slice()).unwrap();
    let challenge_prover = ProofChallenge::hash(&pok.to_bytes());
    let proof = pok.gen_proof(&challenge_prover).unwrap();

    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in network.size..1{
        // t = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
        ticket_vals = TicketValues{
            layer: i,
            round_id: network.round_id,
            sys_rand: network.sys_rand,
        };
        b =  calculate_hash(&ticket_vals);
        t = b.pow(s) % network.size;

        // Get the public key of the server with ID t
        cur_pk = &network.servers[t as usize].keys.0;

        // Generating PoK for Signature (A,e,s) and t=b^s        
        // TODO: Generate proof of knowledge for t=b^s as well!
        // TODO: complete other zkps as desired and compute `challenge_hash`???
        // TODO: add bytes from other proofs???
        packet = Packet{
            ticket: t,
            proof: proof.to_bytes(false),
            data,
        };

        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        padding = PaddingScheme::new_pkcs1v15_encrypt();
        data = &network.servers[t as usize].rsa_keys.0.encrypt(&mut rng, padding, &packet[..]).expect("failed to encrypt");
    }
    return data;
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

mod tests{
    use rand::Rng;
    use super::*;

    // #[test]
    // pub fn test_simple_network(){

    // }

    #[test]
    pub fn test_ticket_creation(){
        let mut b: u64;
        let mut s: u64;
        let mut t: u64;
        s = 0xFF88DDAA00000002;
        let mut S: Fr;
        S = Fr::from_repr(FrRepr::from(s)).unwrap();
        // t = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
        b =  5; // Fr::from_repr(FrRepr::from(calculate_hash(&ticket_vals))).unwrap();
        t = b.pow((S.into_repr().0[0] & 0x00000000FFFFFFFF).try_into().unwrap());
        println!("t: {}", t);
    }
}
