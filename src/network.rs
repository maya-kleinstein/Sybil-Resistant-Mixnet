use crate::prelude::{*, PublicKey};
use std::collections::hash_map::DefaultHasher;
use std::convert::TryInto;
use std::hash::{Hash, Hasher};
use ff_zeroize::PrimeField;
use rand_08::Rng;
use rsa::{RsaPrivateKey, RsaPublicKey, PaddingScheme};
use rsa::PublicKey as PublicKeyForRSAEnc;
use serde::{Serialize, Deserialize};


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
    rsa_keys: (RsaPublicKey, RsaPrivateKey),
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
#[derive(Serialize, Deserialize)]
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
    let mut ticket_vals: TicketValues;
    let mut b: u64;
    let mut t: u64;
    let mut data: Vec<u8> = data;
    let mut padding: PaddingScheme;
    let mut encoded_packet: Vec<u8>;

    let s = (client.signature.s.into_repr().0[0] & 0x00000000FFFFFFFF).try_into().unwrap();
    
    let proof_messages = vec![
        pm_revealed!(b"I'm a valid user! Some ID number...?"),
    ];
    let mut rng = rand_08::thread_rng();

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

        // Generating PoK for t=b^s given Signature (A,e,s)       
        // TODO: Generate proof of knowledge for t=b^s as well!
        packet = Packet{
            ticket: t,
            proof: proof.to_bytes(false),
            data,
        };

        encoded_packet = bincode::serialize(&packet).unwrap();

        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        padding = PaddingScheme::new_pkcs1v15_encrypt();
        data = network.servers[t as usize].rsa_keys.0.encrypt(&mut rng, padding, &encoded_packet[..]).expect("failed to encrypt");
    }
    return bincode::deserialize(&data[..]).unwrap();
}

/// Creating a new network of size size
pub fn create_network(network: &mut Network, size: u64){
    let mut keys: (PublicKey, SecretKey) = Issuer::new_keys(1).unwrap();

    let mut rng = rand_08::thread_rng();
    let bits = 2048;
    let mut rsa_private_key:RsaPrivateKey;
    let mut rsa_public_key:RsaPublicKey;

    let id_provider = IDProvider{
        keys: keys,
        sys_rand: rand::random::<i32>(),
    };

    network.id_provider = id_provider;
    network.sys_rand = rand_08::thread_rng().gen();
    network.round_id = 0;
    
    for i in 0..size{
        keys = Issuer::new_keys(1).unwrap();

        rsa_private_key = RsaPrivateKey::new(&mut rng, bits).unwrap();
        rsa_public_key = RsaPublicKey::from(&rsa_private_key);
    
        let server = Server{
            keys,
            rsa_keys: (rsa_public_key, rsa_private_key),
            id: i.try_into().unwrap(),
        };
        network.servers[i as usize] = server;
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}


mod tests{
    use super::*;
    use pairing_plus::bls12_381::{Fr, FrRepr};
    
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
        b =  5;
        t = b.pow((S.into_repr().0[0] & 0x00000000FFFFFFFF).try_into().unwrap());
        println!("t: {}", t);
    }
}
