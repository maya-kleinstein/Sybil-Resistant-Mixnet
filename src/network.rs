use crate::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use ff_zeroize::{Field, PrimeField};
use pairing_plus::bls12_381::{FrRepr, Fr};
use std::convert::{TryFrom, TryInto};

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

/// Ticket configuration
pub struct Ticket{
    ticket: u64,
    proof: Vec<u8>, // TODO: ???
}

/// Packet configuration
pub struct Packet{
    ticket: Ticket,
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
    let mut s: u32;
    let mut data: Vec<u8>;
    let mut proof: SignatureProof;
    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in network.size..1{
        // t = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
        ticket_vals = TicketValues{
            layer: i,
            round_id: network.round_id,
            sys_rand: network.sys_rand,
        };
        b =  calculate_hash(&ticket_vals);
        s = (client.signature.s.into_repr().0[0] & 0x00000000FFFFFFFF).try_into().unwrap();
        t = b.pow(s) % network.size;

        // TODO: Genereate Proof of Knowledge for (A,e,s) and t=b^s

        // Onion Encryption
        cur_pk = &network.servers[usize::try_from(t).unwrap()].keys.0;
        
        // old data needs to be old packet (to string? or smthn...) using cur_sk
        packet = Packet{
            ticket: Ticket{
                ticket: t,
                proof: proof.to_bytes(false),
            },
            data,
        };
    }
    return packet;
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

mod tests{
    use rand::Rng;
    use super::*;

    #[test]
    pub fn tests(){
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

    #[test]
    /// A simple network example
    pub fn simple_network() {
        // POC SIMPLE TEST RUN
        let mut rng = rand::thread_rng();
        // Create an ID provider
        let id_provider = IDProvider{
            keys: Issuer::new_keys(1).unwrap(),
            sys_rand: 1,
        };

        let (pk, sk) = id_provider.keys;

        // Create client
        let msg = vec![
            SignatureMessage::hash(b"I belong to this network"),
        ];

        // Create signature for client
        let signature = Signature::new(msg.as_slice(), &sk, &pk).unwrap();

        // Point A
        let ticket_values1 = TicketValues{
            layer: 1,
            round_id: 1,
            sys_rand: 1,
        };

        let b1 = calculate_hash(&ticket_values1);

        let sig_bytes = signature.to_bytes_compressed_form();
        let s_bytes: [u8; 4] = sig_bytes.iter().cloned().rev().take(4).collect::<Vec<u8>>()
        .try_into()
        .unwrap();
        let t1 = u64::overflowing_pow(b1, as_u32_be(&s_bytes).into()).0;

        let claims = t1.to_be_bytes();

        // Verify signature (needs to be done once per server)
        let nonce = Verifier::generate_proof_nonce();
        let proof_request = Verifier::new_proof_request(&[0], &pk).unwrap();

        // Sends `proof_request` and `nonce` to the prover
        let proof_messages = vec![
            pm_revealed!(b"I belong to this network"),
        ];

        // prover creates pok for proof request
        let pok = Prover::commit_signature_pok(&proof_request, proof_messages.as_slice(), &signature)
            .unwrap();
        
        // complete the ticket zkps as desired and compute `challenge_hash`
        let challenge = Prover::create_challenge_hash(&[pok.clone()], Some(&[claims.as_slice()]), &nonce).unwrap();

        let proof = Prover::generate_signature_pok(pok, &challenge).unwrap();

        // // Send `proof` and `challenge` to Verifier (AKA current server)
        // match Verifier::verify_signature_pok(&proof_request, &proof, &nonce) {
        //     Ok(_) => assert!(true),   // check revealed messages
        //     Err(_) => assert!(false), // Why did the proof failed
        // };
        
        // Verifier creates their own challenge bytes using proof, proof_request, claims, and nonce
        let ver_challenge = Verifier::create_challenge_hash(
            &[proof.clone()],
            &[proof_request.clone()],
            &nonce,
            Some(&[claims.as_slice()]),
        )
        .unwrap();

        assert_eq!(challenge, ver_challenge);

        // Verifier checks proof1
        let res = proof.proof.verify(
            &proof_request.verification_key,
            &proof.revealed_messages,
            &ver_challenge,
        );
        match res {
            Ok(_) => assert!(true),   // check revealed messages
            Err(_) => assert!(false), // Why did the proof fail?
        };
    }


    fn as_u32_be(array: &[u8; 4]) -> u32 {
        ((array[0] as u32) << 24) +
        ((array[1] as u32) << 16) +
        ((array[2] as u32) <<  8) +
        ((array[3] as u32) <<  0)
    }
}
