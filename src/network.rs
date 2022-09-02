use crate::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::convert::TryInto;
use std::hash::{Hash, Hasher};

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

/// Server configuration
pub struct Server{
    id: u32,
}

/// Client configuration
pub struct IDProvider{
    keys: (PublicKey, SecretKey),
    sys_rand: u32,
}

/// Ticket configuration
#[derive(Hash)]
pub struct TicketValues{
    layer: u32,
    round_id: u32,
}


/// A simple network example
pub fn simple_network() {
    // POC SIMPLE TEST RUN
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

    let ticket_values1 = TicketValues{
        layer: 1,
        round_id: 1,
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

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn as_u32_be(array: &[u8; 4]) -> u32 {
    ((array[0] as u32) << 24) +
    ((array[1] as u32) << 16) +
    ((array[2] as u32) <<  8) +
    ((array[3] as u32) <<  0)
}