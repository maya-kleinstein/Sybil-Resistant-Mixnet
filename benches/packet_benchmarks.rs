use bbs::config::MixnetVerification::{self};
use bbs::network::*;
use bbs::prelude::*;
use bbs::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Things I want to test: Time to generate a ticket, time to verify a *ticket* and signature, Time to decrypt a packet

pub fn decrypt_setup_packet_layer_benchmark(c: &mut Criterion) {
    let network = black_box(Network::new(2, 3, MixnetVerification::NoVerification, true));
    let mut client = black_box(Client::new(&network));
    let (enc_data, first_server) = black_box(generate_setup_packet(&mut client, &network));  
    c.bench_function("decrypt_setup_packet_layer", |b| {
        b.iter(||  {
            decrypt_setup_layer(&enc_data, first_server, &network, 0);
        })
    });
}

pub fn decrypt_data_packet_layer_benchmark(c: &mut Criterion) {
    let data = black_box(vec![1; 128]);
    let network = black_box(Network::new(2, 3, MixnetVerification::NoVerification, true));
    let mut client = black_box(Client::new(&network));
    let mut conns = Connections::new(network.size);
    let (enc_data, first_server) = black_box(generate_setup_packet(&mut client, &network));  
    let _ = decrypt_setup_packet(enc_data, first_server, &network, &mut conns);
    let enc_data_packet = black_box(generate_data_packet(data, &client, &network));
    c.bench_function("decrypt_data_packet_layer", |b| {
        b.iter(||  {
            decrypt_data_packet_layer(&enc_data_packet, first_server, &conns, 0);
        })
    });
}

pub fn verify_proof_benchmark(c: &mut Criterion) {
    let network = black_box(Network::new(2, 3, MixnetVerification::Verify, true));
    let mut client = black_box(Client::new(&network));
    let (enc_data, first_server) = black_box(generate_setup_packet(&mut client, &network));
    // Decrypt layer of packet
    let dec_setup_packet = black_box(decrypt_setup_layer(
        &enc_data, 
        first_server, 
        &network, 
        0
    )).unwrap().0;

    c.bench_function("verify_proof_benchmark", |b| {
        b.iter(||  {
            verify_setup_packet(&dec_setup_packet, &network, 0);
        })
    });
}

pub fn register_client_benchmark(c: &mut Criterion) {
    let (pk, sk) = black_box(Issuer::new_keys(1).unwrap());
    let message = black_box(SignatureMessage::hash(b"message_0"));

    let signature_blinding = black_box(Signature::generate_blinding());

    let mut builder = black_box(CommitmentBuilder::new());
    builder.add(&pk.h0, &signature_blinding);
    builder.add(&pk.h[0], &message);

    let commitment = black_box(builder.finalize());

    // Completed by the signer
    // `commitment` is received from the recipient
    let messages = black_box(sm_map![]);
    
    c.bench_function("register_client", |b| {
        b.iter(||  {
            BlindSignature::new(&commitment, &messages, &sk, &pk).unwrap();
        })
    });
}

criterion_group!(packet_benches, decrypt_data_packet_layer_benchmark, decrypt_setup_packet_layer_benchmark, verify_proof_benchmark, register_client_benchmark);
criterion_main!(packet_benches);
