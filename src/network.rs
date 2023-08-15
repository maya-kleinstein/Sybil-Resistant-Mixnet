use crate::pok_ticket::{PoKOfTicket, PoKOfTicketProof};
use crate::prelude::{*, PublicKey};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryInto;
use std::io::Cursor;
use blake2::Blake2b;
use dryoc::types::StackByteArray;
use pairing_plus::serdes::SerDes;
use pairing_plus::{CurveProjective, CurveAffine};
use pairing_plus::bls12_381::{G1, Fr};
use pairing_plus::hash_to_curve::HashToCurve;
use pairing_plus::hash_to_field::ExpandMsgXmd;
use rand::rngs::OsRng;
use rand_4net::Rng;
use serde::{Serialize, Deserialize};
use blake2::VarBlake2b;
use blake2::digest::{Input, VariableOutput};
use dryoc::dryocbox;
use dryoc::dryocbox::DryocBox;
use crate::config::*;

/// Network module
/// Contains network related functionality
/// Entities Included: ID provider, Server, Client, Tickets, etc.

/// IDprovider configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct IDProvider{
    pub bbs_keys: (PublicKey, SecretKey),
}

/// Server configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Server{
    key_pair: dryocbox::KeyPair,
}


impl Server {
    pub fn new() -> Server {
        Server {
            key_pair: dryocbox::KeyPair::gen(),
        }
    }
}

/// Client configuration
#[derive(Debug)]
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet{
    ticket: Vec<u8>,
    pub proof: Vec<u8>, 
    pub data: Vec<u8>,
}

/// Network configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct Network{
    pub id_provider: IDProvider,
    pub sys_rand: i32,
    pub round_id: u32,
    /// Amount of servers in the network
    pub size: u64,
    pub num_layers: u64,
    pub mix_verification: MixnetVerification,
    pub servers: Vec<Server>,
}


impl Network {
    /// Generate a network of size size
    pub fn new(size: u64, num_layers: u64, mix_verification: MixnetVerification) -> Network {
        let id_provider = IDProvider {
            bbs_keys: Issuer::new_keys(1).unwrap(),
        };
        let servers = vec![Server::new(); size.try_into().unwrap()];
        Network {
            id_provider,
            sys_rand: 0,
            round_id: 0,
            size,
            num_layers,
            mix_verification,
            servers,
        }
    }
}


//TODO: eventually take care of private-public values (e.g. Network can see all private keys)

/// Generate a packet from the client to the network with the given data
pub fn generate_packet(data: Vec<u8>, client: &Client, network: &Network) -> (Vec<u8>, u64){
    let mut data: Vec<u8> = data;
    let mut x: u64 = 0;
    let mut packet: Packet;

    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in (0..network.num_layers).rev(){
        // Creates packet layer (proof + ticket)
        (packet, x) = generate_layer(data, client, network, i);

        // Serialize packet
        let encoded_packet = bincode::serialize(&packet).unwrap();
        
        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        let wrapped_data = DryocBox::seal_to_vecbox(&encoded_packet, &network.servers[x as usize].key_pair.public_key.clone()).expect("Unable to seal");
        data = bincode::serialize(&wrapped_data).unwrap();
    }
    return (data, x);
}

/// Create packet with data proof and ticket
pub fn generate_layer(data: Vec<u8>, client: &Client, network: &Network, layer: u64) -> (Packet, u64){
    // t = b^e, where b=H0(layer, RoundID, SysRand) and e is part of signature
    let (b, t) = calculate_ticket(layer, network.round_id, network.sys_rand, client.signature.e);
    // server x = H(t) % network size, H: {0,1}^* -> Zp
    let x = calculate_next_server(t, network.size);

    // serialize t into buffer
    let mut t_buf = Vec::new();
    t.serialize(&mut t_buf, false).unwrap();

    let mut proof: Vec<u8> = Vec::new();
    match network.mix_verification {
        MixnetVerification::NoVerification => (),
        _ => proof = get_ticket_proof(client, network, t, b).to_bytes_uncompressed_form(),
    };

    let packet = Packet{
        ticket: t_buf,
        proof,
        data,
    };
    return (packet, x);
}

/// Decrypt a packet traversing through the network
pub fn decrypt_packet(enc_packet: Vec<u8>, x_0 :u64, network: &Network) -> Vec<u8>{
    let mut data = enc_packet;
    let mut x = x_0;

    for i in 0..network.num_layers{
        // Decrypt Packet 
        let dryocbox : DryocBox<StackByteArray<32>, StackByteArray<16>, Vec<u8>> = bincode::deserialize(&data).unwrap();
        let decrypted = dryocbox.unseal_to_vec(&network.servers[x as usize].key_pair).expect("unable to decrypt");
        let mut packet: Packet = bincode::deserialize(&decrypted).unwrap();

        // Verify ticket and proof (done by x)
        x = verify_packet(&mut packet, &network, i).0;
        // Retrieving data and next server 
        data = packet.data;
    }
    return data;
}

/// unwraps single layer of packet, given the current server and layer
/// Verifies in case of mixnet type verify
pub fn decrypt_layer(
    enc_packet: &[u8], 
    x: u64, network: &Network, 
    layer: u64
) -> Option<(Packet, u64)> {
    // Decrypt Packet 
    let dryocbox : DryocBox<StackByteArray<32>, StackByteArray<16>, Vec<u8>> = bincode::deserialize(enc_packet).unwrap();
    let decrypted = dryocbox.unseal_to_vec(&network.servers[x as usize].key_pair).expect("unable to decrypt");
    let packet: Packet = bincode::deserialize(&decrypted).unwrap();

    let next_server: u64;
    let valid: bool;
    // Verify ticket and proof (done by x)
    match network.mix_verification {
        MixnetVerification::Verify => 
            {
                (next_server, valid) = verify_packet(&packet, &network, layer);
                if !valid {
                    return None;
                }
            },
        _ => 
            next_server = get_next_server_from_packet(&packet, &network),
    };
    // Retrieving data and next server 
    return Some((packet, next_server));
}

/// Verify the proof of knowledge of the signature and the ticket
/// Return the next server and is_valid
pub fn verify_packet(packet: &Packet, network: &Network, layer: u64) -> (u64, bool) {
    let revealed_msgs = setup_default_msgs();

    // Calculating next server using the ticket
    let mut cursor = Cursor::new(&packet.ticket);
    let t_recovered = slice_to_elem!(&mut cursor, G1, false).unwrap();
    let x = calculate_next_server(t_recovered, network.size);

    // Recovering the value of b
    let ticket_vals = TicketValues{
        layer,
        round_id: network.round_id,
        sys_rand: network.sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let b_recovered =  h_0(ticket_vals_bytes);
    // getting proof from bytes
    let proof = PoKOfTicketProof::from_bytes_uncompressed_form(&packet.proof).unwrap();
    // Setting up revealed indices
    let mut revealed_indices = BTreeSet::new();
    revealed_indices.insert(0);
    // The verifier generates the challenge on its own.
    let challenge_bytes = proof.get_bytes_for_challenge(revealed_indices.clone(), &network.id_provider.bbs_keys.0, b_recovered, t_recovered);
    let challenge_verifier = ProofChallenge::hash(&challenge_bytes);
    let valid = proof
        .verify(&network.id_provider.bbs_keys.0, &revealed_msgs, &challenge_verifier, b_recovered, t_recovered)
        .unwrap()
        .is_valid();
    return (x, valid);
}

pub fn get_next_server_from_packet(packet: &Packet, network: &Network) -> u64 {
    let mut cursor = Cursor::new(&packet.ticket);
    let t_recovered = slice_to_elem!(&mut cursor, G1, false).unwrap();
    let x = calculate_next_server(t_recovered, network.size);
    return x;
}

/// Verify batch
pub fn verify_batch(packets: &Vec<Packet>, network: &Network, layer: u64){
   // Set up msg.'s info before decrypting
    let mut revealed_indices = BTreeSet::new();
    revealed_indices.insert(0);

    let revealed_msgs = setup_default_msgs();

    let mut batch: Vec<(PoKOfTicketProof, ProofChallenge, G1, G1)> = Vec::with_capacity(packets.len());

    for i in 0..packets.len(){
        // Calculating next server using the ticket
        let mut cursor = Cursor::new(&packets[i].ticket);
        let t_recovered = slice_to_elem!(&mut cursor, G1, false).unwrap();

        // Recovering the value of b
        // TODO: value for b is the same for everybody - compute once!
        let ticket_vals = TicketValues{
            layer,
            round_id: network.round_id,
            sys_rand: network.sys_rand,
        };
        let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
        let b_recovered =  h_0(ticket_vals_bytes);
        // getting proof from bytes
        let proof = PoKOfTicketProof::from_bytes_uncompressed_form(&packets[i].proof).unwrap();

        // The verifier generates the challenge on its own.
        let challenge_bytes = proof.get_bytes_for_challenge(revealed_indices.clone(), &network.id_provider.bbs_keys.0, b_recovered, t_recovered);
        let challenge_verifier = ProofChallenge::hash(&challenge_bytes);
        batch.push((proof, challenge_verifier, b_recovered, t_recovered))
    }
    // Verify ticket and proof (done by x)
    assert!(PoKOfTicketProof::batch_verify(batch, &network.id_provider.bbs_keys.0, &revealed_msgs)
        .unwrap()
        .is_valid());
}

/// Generating packets with false proofs
/// TODO: NOT DONE YET
pub fn generate_bad_packet(
    data: Vec<u8>,
    client: &Client, 
    network: &Network,
    bad_tickets: &Vec<G1>,
) -> (Vec<u8>, u64){
    let mut data: Vec<u8> = data;
    let mut x: u64 = 0;
    let mut packet: Packet;

    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in (0..network.num_layers).rev(){
        // Creates packet layer (proof + ticket)
        (packet, x) = generate_layer(data, client, network, i);

        // Mess with packet by setting ticket to default
        if i > 0 {
            let false_ticket = bad_tickets[i as usize - 1];

            packet.ticket = vec![];
            
            false_ticket.serialize(&mut packet.ticket, false).unwrap();
            x = calculate_next_server(false_ticket, network.size);
        }

        // Serialize packet
        let encoded_packet = bincode::serialize(&packet).unwrap();
        
        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        let wrapped_data = DryocBox::seal_to_vecbox(&encoded_packet, &network.servers[x as usize].key_pair.public_key.clone()).expect("Unable to seal");
        data = bincode::serialize(&wrapped_data).unwrap();
    }
    return (data, x);
}

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

/// Get a random ticket that maps to i for all i in range(NUM_SERVERS)
pub fn ticket_server_map_generator(num_mixes: u16) -> HashMap<u64, G1> {
    // create a hashmap for (ticket, server) mapping
    let mut ticket_server_map: HashMap<u64, G1> = HashMap::new();
    // generate values until hashmap is full
    let r = &mut OsRng;
    while ticket_server_map.len() < num_mixes.into(){
        let rand_ticket = G1::random(r);
        let rand_server = calculate_next_server(rand_ticket, num_mixes.into());
        // if rand_server is not in hashmap, add it
        if !ticket_server_map.contains_key(&rand_server){
            ticket_server_map.insert(rand_server, rand_ticket);
        }
    }
    return ticket_server_map;
}


// Calculating ticket = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
fn calculate_ticket(layer: u64, round_id: u32, sys_rand: i32, e: Fr)-> (G1, G1){
    let ticket_vals = TicketValues{
        layer,
        round_id,
        sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let b =  h_0(ticket_vals_bytes);
    let mut t = b;
    t.mul_assign(e);

    return (b, t);
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


// Calculate ticket proof
fn get_ticket_proof(client: &Client, network: &Network, t: G1, b:G1) -> PoKOfTicketProof {
    let proof_messages = vec![
        pm_revealed!(b"Testing"),
    ];

    // Building proof for ticket + signature
    let ticket_pok = PoKOfTicket::init(
        &client.signature, 
        &network.id_provider.bbs_keys.0, 
        proof_messages.as_slice(),
        t,
        b
    ).unwrap();

    // TODO: beware weak fiat shamir
    let challenge_prover = ProofChallenge::hash(&ticket_pok.to_bytes());
    let proof = ticket_pok.gen_proof(&challenge_prover).unwrap();
    return proof;
}


fn setup_default_msgs() -> BTreeMap<usize, SignatureMessage> {
    let messages = vec![
        SignatureMessage::hash(b"Testing"),
    ];
    
    let mut revealed_indices = BTreeSet::new();
    revealed_indices.insert(0);

    let mut revealed_msgs = BTreeMap::new();
    for i in &revealed_indices {
        revealed_msgs.insert(i.clone(), messages[*i]);
    }
    return revealed_msgs
}

// H0: {0,1}^* -> G1
fn h_0<I: AsRef<[u8]>>(data: I) -> G1 {
    const DST: &[u8] = b"BLS12381G1_XMD:BLAKE2B_SSWU_RO_BBS+_SIGNATURES:ANONYMOUS_MIXNETS:1_0_0";
    <G1 as HashToCurve<ExpandMsgXmd<Blake2b>>>::hash_to_curve(data.as_ref(), DST)
}


#[cfg(test)]
mod tests{
    use super::*;
 
    const TEST_NETWORK_SIZE: u64 = 2;
    const TEST_NUM_LAYERS: u64 = 2;
    const TEST_MIX_VERIFICATION: MixnetVerification = MixnetVerification::NoVerification;

    #[test]
    pub fn test_simple_network(){
        let network = Network::new(TEST_NETWORK_SIZE, TEST_NUM_LAYERS, TEST_MIX_VERIFICATION);
        let client = Client::new(&network);
        let data = vec![b'a', b'b', b'c'];
        let (enc_data, first_server) = generate_packet(data, &client, &network);

        println!("{}, is the first server", first_server);
        println!("{}, is the length of the data", enc_data.len());

        let dec_data = decrypt_packet(enc_data, first_server, &network);

        assert_eq!(dec_data, vec![b'a', b'b', b'c']);
    }

    #[test]
    pub fn test_batch_verification(){
        let network = Network::new(2, 3, MixnetVerification::NoVerification);
        let clients = vec![
            Client::new(&network),
            Client::new(&network),
            Client::new(&network)];
    
        let packets = vec![
            generate_layer(vec![1, 2, 3], &clients[0], &network, 0).0,
            generate_layer(vec![1, 2, 3], &clients[1], &network, 0).0,
            generate_layer(vec![1, 2, 3], &clients[2], &network, 0).0];
    
        verify_batch(&packets, &network, 0);
    }

    #[test]
    pub fn test_into_and_from_bytes_g1(){
        let t = G1::one();
        let mut t_buf = Vec::new();
        t.serialize(&mut t_buf, false).unwrap();

        let mut cursor = Cursor::new(t_buf);
        let _t_recover = slice_to_elem!(&mut cursor, G1, false).unwrap();
        println!("Well?");
    }

}