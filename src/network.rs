use crate::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use ff_zeroize::{Field, PrimeField};
use pairing_plus::bls12_381::{FrRepr, Fr};
use std::convert::TryFrom;

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
    layer: u32,
    round_id: u32,
    sys_rand: i32,
}

/// Ticket configuration
pub struct Ticket{
    ticket: u64,
    proof: SignatureProof, // TODO: ???
}

/// Packet configuration
pub struct Packet{
    ticket: Ticket,
    data: String
}

pub struct Network{
    id_provider: IDProvider,
    sys_rand: i32,
    round_id: u32,
    size: u32,
    servers: [Server],
}

pub fn generate_packet(data: String, client: &Client, network: &Network) -> Packet{
    let mut packet: Packet;
    let mut cur_sk: &SecretKey;
    let mut ticket_vals: TicketValues;
    let mut b: Fr;
    let mut t: u64;
    for i in network.size..1{
        // t = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
        ticket_vals = TicketValues{
            layer: i,
            round_id: network.round_id,
            sys_rand: network.sys_rand,
        };
        b = Fr::from_repr(FrRepr::from(calculate_hash(&ticket_vals))).unwrap();
        t = 1; // TODO: should be t = b^s

        // TODO: Fix proof generation... rn it only proves knowledge of signature
        let nonce = Verifier::generate_proof_nonce();
        let proof_request = Verifier::new_proof_request(&[0], 
                                &network.servers[usize::try_from(t).unwrap()].keys.0).unwrap();

        let proof_messages = vec![
            pm_revealed!(b"I belong to this network"),
        ];

        let pok = Prover::commit_signature_pok(&proof_request, proof_messages.as_slice(), &client.signature)
            .unwrap();
        let challenge = Prover::create_challenge_hash(&[pok.clone()], None, &nonce).unwrap();

        let proof = Prover::generate_signature_pok(pok, &challenge).unwrap();

        // TODO: Fix so the following is actually onion encryption...
        // TODO: eventually convert t to t modulo amount of server
        cur_sk = &network.servers[usize::try_from(t).unwrap()].keys.1;
        
        // old data needs to be old packet (to string? or smthn...) using cur_sk
        packet = Packet{
            ticket: Ticket{
                ticket: t,
                proof: proof,
            },
            data: "packet".to_string(),
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
