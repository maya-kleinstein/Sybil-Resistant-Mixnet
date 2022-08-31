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

struct Server{
    id: u32,
}

struct IDProvider{
    keys: (PublicKey, SecretKey),
    sys_rand: u32,
}

#[derive(Hash)]
struct TicketValues{
    layer: u32,
    roundID: u32,
    sys_rand: u32,
}


fn main() {
    // POC SIMPLE TEST RUN
    // Create an ID provider
    let id_provider = IDProvider{
        keys: Issuer::new_keys(1).unwrap(),
        sys_rand: 1,
    };

    let (pk, sk) = id_provider.keys;

    // Create Server*n (n=1)
    // Creating Server1
    let server1 = Server{
        id: 1,
    };

    // Create client
    let msg = vec![
        SignatureMessage::hash(b"I belong to this network"),
    ];

    // Create signature for client
    let signature = Signature::new(msg.as_slice(), &sk, &pk).unwrap();

    let ticket_values1 = TicketValues{
        layer: 1,
        roundID: 1,
        sys_rand: id_provider.sys_rand,
    };

    let b1 = calculate_hash(&ticket_values1);

    let tmp = signature.to_bytes_compressed_form();
    let t1 = u64::pow(b1, as_u32_be(&tmp[..=4].try_into().unwrap()).into());

    let claims = vec![
        t1.to_be_bytes(),
    ];

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
    let challenge = Prover::create_challenge_hash(&[pok.clone()], Some(claims.as_slice()), &nonce).unwrap();

    let proof = Prover::generate_signature_pok(pok, &challenge).unwrap();

    // Send `proof` and `challenge` to Verifier
    match Verifier::verify_signature_pok(&proof_request, &proof, &nonce) {
        Ok(_) => assert!(true),   // check revealed messages
        Err(_) => assert!(false), // Why did the proof failed
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