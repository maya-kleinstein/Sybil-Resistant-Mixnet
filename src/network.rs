use crate::config::*;
use crate::pok_ticket::{PoKOfTicket, PoKOfTicketProof};
use crate::prelude::{PublicKey, *};
use blake2::digest::{Input, VariableOutput};
use blake2::Blake2b;
use blake2::VarBlake2b;
use dryoc::dryocbox;
use dryoc::dryocbox::DryocBox;
use dryoc::types::{Bytes, MutBytes, NewByteArray, StackByteArray};
use dryoc::dryocsecretbox::{DryocSecretBox, Key, Nonce};
use pairing_plus::bls12_381::{Fr, G1};
use pairing_plus::hash_to_curve::HashToCurve;
use pairing_plus::hash_to_field::ExpandMsgXmd;
use pairing_plus::serdes::SerDes;
use pairing_plus::{CurveAffine, CurveProjective};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryInto;
use std::io::Cursor;

/// Network module
/// Contains network related functionality
/// Entities Included: ID provider, Server, Client, Tickets, Conneciton, SetupPacket, Packet, etc.

/// IDprovider configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct IDProvider {
    pub bbs_keys: (PublicKey, SecretKey),
}

type ConnID = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub conn_id: ConnID,
    key: Vec<u8>,
    layer: u64,
    cur_server: u64,
    dest_server: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Connections {
    conns: Vec<HashMap<ConnID, Connection>>,
    pub size: u64,
}

impl Connections {
    pub fn new(size: u64) -> Connections {
        let mut conns = Vec::with_capacity(size as usize);
        for _ in 0..size {
            conns.push(HashMap::new());
        }
        Connections { conns, size }
    }

    pub fn get(&self, server: u64, conn_id: ConnID) -> Option<&Connection> {
        self.conns[server as usize].get(&conn_id)
    }

    pub fn insert(&mut self, server: u64, conn_id: ConnID, conn: Connection) {
        self.conns[server as usize].insert(conn_id, conn);
    }
}

/// Server configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Server {
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
    // Current circuit connections
    circuit: Vec<(ConnID, Connection)>
}

impl Client {
    /// Generate new client for given network
    pub fn new(network: &Network) -> Client {
        let messages = vec![SignatureMessage::hash(b"Testing")];
        Client {
            signature: Signature::new(
                messages.as_slice(),
                &network.id_provider.bbs_keys.1,
                &network.id_provider.bbs_keys.0,
            )
            .unwrap(),
            circuit: Vec::with_capacity(network.layers as usize),
        } 
    }
}

// Ticket configuration
#[derive(Serialize, Deserialize)]
struct TicketValues {
    layer: u64,
    round_id: u32,
    sys_rand: i32,
}

/// Setup Packet Header
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupPacketHeader {
    // Ticket and proof for connections circuit
    ticket: Vec<u8>,
    pub proof: Vec<u8>,
    // Connection details for the circuit
    conn: Connection,
}

/// Setup Packet configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupPacket {
    pub setup_header: SetupPacketHeader,
    pub data: Vec<u8>,
}

/// Packet configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PacketHeader {
    conn_id: ConnID,
    nonce: Nonce,
}

/// Packet configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub data: Vec<u8>,
}

/// Network configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct Network {
    pub id_provider: IDProvider,
    pub sys_rand: i32,
    pub round_id: u32,
    /// Amount of servers in the network
    pub size: u64,
    /// Amount of layers in the network
    pub layers: u64,
    /// Verification type
    pub mix_verification: MixnetVerification,
    /// Is the proof compressed
    pub is_proof_compressed: bool,
    pub servers: Vec<Server>,
}

impl Network {
    /// Generate a network of size size
    pub fn new(size: u64, layers: u64, mix_verification: MixnetVerification, is_proof_compressed: bool) -> Network {
        let id_provider = IDProvider {
            bbs_keys: Issuer::new_keys(1).unwrap(),
        };
        let servers = vec![Server::new(); size.try_into().unwrap()];
        Network {
            id_provider,
            sys_rand: 0,
            round_id: 0,
            size,
            layers,
            mix_verification,
            is_proof_compressed,
            servers,
        }
    }

    fn get_last_layer_idx(&self) -> u64 {
        self.layers - 1
    }
}

//TODO: eventually take care of private-public values (e.g. Network can see all private keys)

/// Generate a packet from the client to the network with the given data
pub fn generate_setup_packet(client: &mut Client, network: &Network) -> (Vec<u8>, u64) {
    let mut data: Vec<u8> = Vec::new();
    let mut cur_server: u64 = 0;
    let mut setup_packet: SetupPacket;

    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in (0..network.layers).rev() { 
        // Creates packet layer (proof + ticket)
        (setup_packet, cur_server) = generate_setup_packet_layer(data, client, network, i);

        // Add connection to client path
        client.circuit.push((setup_packet.setup_header.conn.conn_id, setup_packet.setup_header.conn.clone()));

        // Serialize packet
        let encoded_setup_packet = bincode::serialize(&setup_packet).unwrap();

        /*
            Note: 
                Since the first server doesn't have a "previous" server to verify its ticket,
                we set it to some random value.
                This shouldn't effect the verification process, 
                as the first server should just verify the proof for the NEXT server.
         */
        if i == 0 { 
            // Set cur_server to some random value
            cur_server = rand::random::<u64>() % network.size;
            println!("The layer 0 server is {}", cur_server);
        }

        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        let wrapped_data = DryocBox::seal_to_vecbox(
            &encoded_setup_packet,
            &network.servers[cur_server as usize].key_pair.public_key.clone(),
        )
        .expect("Unable to seal");
        data = bincode::serialize(&wrapped_data).unwrap();
    }
    client.circuit.reverse();
    client.circuit[0].1.cur_server = cur_server;
    for i in 1..client.circuit.len() {
        client.circuit[i].1.cur_server = client.circuit[i-1].1.dest_server;
    }

    return (data, cur_server);
}

/// Create packet with data proof and ticket
pub fn generate_setup_packet_layer(
    data: Vec<u8>,
    client: &Client,
    network: &Network,
    layer: u64,
) -> (SetupPacket, u64) {
    // t = b^e, where b=H0(layer, RoundID, SysRand) and e is part of signature
    let (b, t) = calculate_ticket(
        layer,
        network.round_id,
        network.sys_rand,
        client.signature.e,
    );
    // server x = H(t) % network size, H: {0,1}^* -> Zp
    let server_id = calculate_next_server(t, network.size);

    // serialize t into buffer
    let mut t_buf = Vec::new();
    t.serialize(&mut t_buf, network.is_proof_compressed).unwrap();

    let mut proof: Vec<u8> = Vec::new();
    match network.mix_verification {
        MixnetVerification::NoVerification | _ if layer == network.get_last_layer_idx() => (),
        _ => match network.is_proof_compressed {
            true => proof = get_ticket_proof(client, network, t, b).to_bytes_compressed_form(),
            false => proof = get_ticket_proof(client, network, t, b).to_bytes_uncompressed_form(),
        },
    };

    // Create connection
    let conn = Connection {
        conn_id: rand::random::<u64>(),
        key: Key::gen().as_slice().to_vec(),
        layer,
        cur_server: 0, // NOTE: cur_server is set to 0, as it is updated at the end of packet generation
        dest_server : server_id,
    };

    
    let setup_packet = SetupPacket {
        setup_header: SetupPacketHeader {
            ticket: t_buf,
            proof,
            conn,
        },
        data: data,
    };

    return (setup_packet, server_id);
}

/// Decrypt a setup packet traversing through the network, while updating network connections
pub fn decrypt_setup_packet(
    enc_packet: Vec<u8>, 
    first_server: u64, 
    network: &Network, 
    conns: &mut Connections,
) -> Vec<u8> {
    let mut data = enc_packet;
    let mut cur_server = first_server;

    for i in 0..network.layers {
        // Decrypt Packet
        let decrypted_packet = decrypt_setup_layer(&data, cur_server, network, i).unwrap();
        conns.insert(cur_server, decrypted_packet.2.conn_id, decrypted_packet.2);
        data = decrypted_packet.0.data;
        cur_server = decrypted_packet.1;
    }
    return data;
}

/// unwraps single layer of packet, given the current server and layer
/// Verifies in case of mixnet type verify
pub fn decrypt_setup_layer(
    enc_packet: &[u8],
    cur_server: u64,
    network: &Network,
    layer: u64,
) -> Option<(SetupPacket, u64, Connection)> {
    // Decrypt Packet
    let dryocbox: DryocBox<StackByteArray<32>, StackByteArray<16>, Vec<u8>> =
        bincode::deserialize(enc_packet).unwrap();
    let decrypted = dryocbox
        .unseal_to_vec(&network.servers[cur_server as usize].key_pair)
        .expect("unable to decrypt");
    let packet: SetupPacket = bincode::deserialize(&decrypted).unwrap();

    // Verify ticket and proof (done by cur_server)
    let next_server: u64;
    let valid: bool;
    match network.mix_verification {
        MixnetVerification::Verify if layer != network.get_last_layer_idx() => {
            (next_server, valid) = verify_setup_packet(&packet, &network, layer);
            if !valid {
                return None;
            }
        }
        _ => next_server = get_next_server_from_packet(&packet, &network),
    };

    let conn = packet.setup_header.conn.clone();
    // TODO: add verification that dest server in conn is next_server

    // Retrieving data and next server
    return Some((packet, next_server, conn));
}

/// Verify the proof of knowledge of the signature and the ticket
/// Return the next server and is_valid
/// TODO: this function can panic easily - need to handle errors better
pub fn verify_setup_packet(packet: &SetupPacket, network: &Network, layer: u64) -> (u64, bool) {
    let revealed_msgs = setup_default_msgs();

    // Calculating next server using the ticket
    let mut cursor = Cursor::new(&packet.setup_header.ticket);
    let t_recovered = slice_to_elem!(&mut cursor, G1, network.is_proof_compressed).unwrap();
    let x = calculate_next_server(t_recovered, network.size);

    // Recovering the value of b
    let ticket_vals = TicketValues {
        layer,
        round_id: network.round_id,
        sys_rand: network.sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let b_recovered = h_0(ticket_vals_bytes);
    // getting proof from bytes
    let proof: PoKOfTicketProof;
    match network.is_proof_compressed {
        true => proof = PoKOfTicketProof::from_bytes_compressed_form(&packet.setup_header.proof).unwrap(),
        false => proof = PoKOfTicketProof::from_bytes_uncompressed_form(&packet.setup_header.proof).unwrap(),
    }
    // Setting up revealed indices
    let mut revealed_indices = BTreeSet::new();
    revealed_indices.insert(0);
    // The verifier generates the challenge on its own.
    let challenge_bytes = proof.get_bytes_for_challenge(
        revealed_indices.clone(),
        &network.id_provider.bbs_keys.0,
        b_recovered,
        t_recovered,
    );
    let challenge_verifier = ProofChallenge::hash(&challenge_bytes);
    let valid = proof
        .verify(
            &network.id_provider.bbs_keys.0,
            &revealed_msgs,
            &challenge_verifier,
            b_recovered,
            t_recovered,
        )
        .unwrap()
        .is_valid();
    return (x, valid);
}

pub fn get_next_server_from_packet(packet: &SetupPacket, network: &Network) -> u64 {
    let mut cursor = Cursor::new(&packet.setup_header.ticket);
    let t_recovered = slice_to_elem!(&mut cursor, G1, network.is_proof_compressed).unwrap();
    let x = calculate_next_server(t_recovered, network.size);
    return x;
}

/// Generating packets with false proofs
pub fn generate_bad_setup_packet(
    client: &mut Client,
    network: &Network,
    bad_tickets: &Vec<G1>,
) -> (Vec<u8>, u64) {
    let mut data: Vec<u8> = Vec::new();
    let mut cur_server: u64 = 0;
    let mut setup_packet: SetupPacket;

    // Onion Encrypt the data using the keys matching the calculated tickets
    for i in (0..network.layers).rev() {
        // Creates packet layer (proof + ticket)
        (setup_packet, cur_server) = generate_setup_packet_layer(data, client, network, i);

        // Mess with packet by setting ticket to default
        if i > 0 {
            let false_ticket = bad_tickets[i as usize - 1];

            setup_packet.setup_header.ticket = vec![];

            false_ticket.serialize(&mut setup_packet.setup_header.ticket, network.is_proof_compressed).unwrap();
            cur_server = calculate_next_server(false_ticket, network.size);

            // re-write the server id in the connection
            setup_packet.setup_header.conn.dest_server = cur_server;
        }
        
        client.circuit.push((setup_packet.setup_header.conn.conn_id, setup_packet.setup_header.conn.clone()));

        // Serialize packet
        let encoded_packet = bincode::serialize(&setup_packet).unwrap();

        // Onion Encryption, where: packet = enc(cur_pk, old_packet || (proof, challenge, proof_request, t))
        let wrapped_data = DryocBox::seal_to_vecbox(
            &encoded_packet,
            &network.servers[cur_server as usize].key_pair.public_key.clone(),
        )
        .expect("Unable to seal");
        data = bincode::serialize(&wrapped_data).unwrap();
    }
    client.circuit.reverse();
    client.circuit[0].1.cur_server = cur_server;
    for i in 1..client.circuit.len() {
        client.circuit[i].1.cur_server = client.circuit[i-1].1.dest_server;
    }
    return (data, cur_server);
}

/// Get a random ticket that maps to i for all i in range(num_mixes)
pub fn ticket_server_map_generator(num_mixes: u16) -> HashMap<u64, G1> {
    // create a hashmap for (ticket, server) mapping
    let mut ticket_server_map: HashMap<u64, G1> = HashMap::new();
    // generate values until hashmap is full
    let r = &mut OsRng;
    while ticket_server_map.len() < num_mixes.into() {
        let rand_ticket = G1::random(r);
        let rand_server = calculate_next_server(rand_ticket, num_mixes.into());
        // if rand_server is not in hashmap, add it
        if !ticket_server_map.contains_key(&rand_server) {
            ticket_server_map.insert(rand_server, rand_ticket);
        }
    }
    return ticket_server_map;
}

pub fn generate_packet(data: Vec<u8>, client: &Client, network: &Network) -> Vec<u8> {
    let mut data : Vec<u8> = data;
    let mut packet: Packet;

    for i in (0..network.layers).rev() {
        let nonce = Nonce::gen();
        let key_bytes: &[u8] = &client.circuit[i as usize].1.key;
        let mut key: Key = Key::default(); 
        Key::copy_from_slice(&mut key, key_bytes);
        // Creates packet layer (encrypt with circuit keys)
        let dryocsecretbox = DryocSecretBox::encrypt_to_vecbox(
            &data, 
            &nonce, 
            &key,
        );
        packet = Packet {
            header: PacketHeader {
                conn_id: client.circuit[i as usize].0,
                nonce,
            },
            data: dryocsecretbox.to_vec(),
        };

        data = bincode::serialize(&packet).unwrap();
    }

    return data;
}

// TODO: make it so this function returns "Result" instead of "Option"
//  this will be more readable and easier to maintain
pub fn decrypt_packet_layer(
    enc_packet: &[u8], 
    cur_server: u64, 
    conns: &Connections,
    layer: u64
) -> Option<(Vec<u8>, u64)> {
    let packet: Packet = match bincode::deserialize(enc_packet) {
        Ok(p) => p,
        Err(_) => return None,
    };
    let dryocsecretbox = match DryocSecretBox::from_bytes(&packet.data) {
        Ok(boxed) => boxed,
        Err(_) => return None,
    };
    let conn_id = packet.header.conn_id;
    let conn = match conns.get(cur_server, conn_id) {
        Some(c) => c,
        None => return None,
    };
    if conn.layer != layer && conn.cur_server != cur_server {
        return None;
    }
    let key_bytes: &[u8] = &conn.key;
    let mut key: Key = Key::default(); 
    Key::copy_from_slice(&mut key, key_bytes);
    let nonce = &packet.header.nonce;

    let decrypted_packet = match dryocsecretbox.decrypt_to_vec(nonce, &key) {
        Ok(packet) => packet,
        Err(_) => return None,
    };

    return Some((decrypted_packet, conn.dest_server));
}

// Calculating ticket = b^s, where b=H(layer, RoundID, SysRand) and s is part of signature
fn calculate_ticket(layer: u64, round_id: u32, sys_rand: i32, e: Fr) -> (G1, G1) {
    let ticket_vals = TicketValues {
        layer,
        round_id,
        sys_rand,
    };
    let ticket_vals_bytes = bincode::serialize(&ticket_vals).unwrap();
    let b = h_0(ticket_vals_bytes);
    let mut t = b;
    t.mul_assign(e);

    return (b, t);
}

// Calculating next server x from ticket
fn calculate_next_server(t: G1, size: u64) -> u64 {
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
fn get_ticket_proof(client: &Client, network: &Network, t: G1, b: G1) -> PoKOfTicketProof {
    let proof_messages = vec![pm_revealed!(b"Testing")];

    // Building proof for ticket + signature
    let ticket_pok = PoKOfTicket::init(
        &client.signature,
        &network.id_provider.bbs_keys.0,
        proof_messages.as_slice(),
        t,
        b,
    )
    .unwrap();

    // TODO: beware weak fiat shamir
    let challenge_prover = ProofChallenge::hash(&ticket_pok.to_bytes());
    let proof = ticket_pok.gen_proof(&challenge_prover).unwrap();
    return proof;
}

fn setup_default_msgs() -> BTreeMap<usize, SignatureMessage> {
    let messages = vec![SignatureMessage::hash(b"Testing")];

    let mut revealed_indices = BTreeSet::new();
    revealed_indices.insert(0);

    let mut revealed_msgs = BTreeMap::new();
    for i in &revealed_indices {
        revealed_msgs.insert(i.clone(), messages[*i]);
    }
    return revealed_msgs;
}

// H0: {0,1}^* -> G1
fn h_0<I: AsRef<[u8]>>(data: I) -> G1 {
    const DST: &[u8] = b"BLS12381G1_XMD:BLAKE2B_SSWU_RO_BBS+_SIGNATURES:ANONYMOUS_MIXNETS:1_0_0";
    <G1 as HashToCurve<ExpandMsgXmd<Blake2b>>>::hash_to_curve(data.as_ref(), DST)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_NETWORK_SIZE: u64 = 2;
    const TEST_NETWORK_LAYERS: u64 = 5;
    const TEST_NETWORK_MIX_VERIFICATION: MixnetVerification = MixnetVerification::Verify;
    const TEST_IS_COMPRESSED_PROOF: bool = true;

    #[test]
    pub fn test_basic_setup_packet() {
        let network = Network::new(
            TEST_NETWORK_SIZE,
            TEST_NETWORK_LAYERS,
            TEST_NETWORK_MIX_VERIFICATION,
            TEST_IS_COMPRESSED_PROOF,
        );
        let mut client = Client::new(&network);
        let mut conns = Connections::new(network.size);
        let (enc_data, first_server) = generate_setup_packet(&mut client, &network);    
        
        println!("{}, is the first server", first_server);
        println!("{}, is the length of the packet", enc_data.len());

        let dec_data = decrypt_setup_packet(enc_data, first_server, &network, &mut conns);
        assert_eq!(0, dec_data.len());

        let circuit = &client.circuit;
        assert_eq!(TEST_NETWORK_LAYERS as usize, circuit.len());
        assert_eq!(circuit[0].1.cur_server, first_server);

        let mut num_conn_ids = vec![0; network.size as usize];
        for i in 0..circuit.len() {
            num_conn_ids[circuit[i].1.cur_server as usize] += 1;
        }

        for i in 0..network.size as usize {
            println!("Server {} has {} connections", i, num_conn_ids[i]);
            assert_eq!(num_conn_ids[i], conns.conns[i].len());
        }
    }

    #[test]
    pub fn test_basic_generate_packet() {
        // First create connection circuit
        let network = Network::new(
            TEST_NETWORK_SIZE,
            TEST_NETWORK_LAYERS,
            TEST_NETWORK_MIX_VERIFICATION,
            TEST_IS_COMPRESSED_PROOF,
        );
        let mut client = Client::new(&network);
        let mut conns = Connections::new(network.size);
        let (enc_data, first_server) = generate_setup_packet(&mut client, &network);    
        
        let _ = decrypt_setup_packet(enc_data, first_server, &network, &mut conns);

        let data = vec![b'a'; 128];
        let mut enc_packet = generate_packet(data, &client, &network);
        println!("The encrypted data is of len: {}", enc_packet.len());
        let mut cur_server = first_server;
        for layer in 0..network.layers {
            let decrypted = decrypt_packet_layer(&enc_packet, cur_server, &conns, layer).unwrap();
            enc_packet = decrypted.0;
            cur_server = decrypted.1;
        }
        println!("The decrypted data is of len: {}", enc_packet.len());
        
    }

    #[test]
    pub fn test_decrypt_setup_layer() {
        let network = Network::new(
            TEST_NETWORK_SIZE,
            TEST_NETWORK_LAYERS,
            TEST_NETWORK_MIX_VERIFICATION,
            TEST_IS_COMPRESSED_PROOF,
        );
        let mut client = Client::new(&network);
        // let mut conns = Connections::new(network.size);
        let (enc_data, first_server) = generate_setup_packet(&mut client, &network);    
        // Testing if the setup packet is bad
        let dec_setup_packet = decrypt_setup_layer(
            &enc_data, 
            first_server, 
            &network, 
            0
        );
        if dec_setup_packet.is_none() {
            println!("The packet is bad");
        } else {
            println!("The packet is good");
        }
    }

    #[test]
    pub fn test_into_and_from_bytes_g1() {
        let t = G1::one();
        let mut t_buf = Vec::new();
        t.serialize(&mut t_buf, TEST_IS_COMPRESSED_PROOF).unwrap();

        let mut cursor = Cursor::new(t_buf);
        let _t_recover = slice_to_elem!(&mut cursor, G1, TEST_IS_COMPRESSED_PROOF).unwrap();
        println!("Well?");
    }
}
