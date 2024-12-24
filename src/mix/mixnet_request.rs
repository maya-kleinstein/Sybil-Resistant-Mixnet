use serde::{Deserialize, Serialize};
use crate::mix::*;
use crate::network::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MixnetPacketType {
    Packet(Vec<u8>),
    SetupPacket(SetupPacket),
}

pub trait MixnetPacket {
    async fn decrypt_incoming_packets(
        enc_packets: Vec<Vec<u8>>,
        cur_server: u64,
        network: &Network,
        conns: &Arc<RwLock<Connections>>,
        layer: u64,
    ) -> Vec<(MixnetPacketType, u64)>;
}

impl MixnetPacket for SetupPacket {
    async fn decrypt_incoming_packets(
        enc_packets: Vec<Vec<u8>>,
        cur_server: u64,
        network: &Network,
        conns : &Arc<RwLock<Connections>>,
        layer: u64,
    ) -> Vec<(MixnetPacketType, u64)> {
        let dec_packets: Vec<(MixnetPacketType, u64, Connection)> = enc_packets
            .par_iter()
            .filter_map(|enc_packet| {
                // I'll decrypt the packet and return the packet, layer, and connection
                let dec_packet = decrypt_setup_layer(enc_packet, cur_server, network, layer);
                return dec_packet.map(|(packet, next_server, conn)| 
                    (MixnetPacketType::SetupPacket(packet), 
                    next_server,
                    conn
                ));
            })
            .collect();
        let mut conns_guard = conns.write().await;
        for (_, next_server, conn) in &dec_packets {
            (*conns_guard).insert(next_server.clone(), conn.conn_id.clone(), conn.clone());
        }
        drop(conns_guard);
        // remove the connections from dec_packets without copying the rest of the data
        let ret_packets = dec_packets.into_iter().map(|(packet, next_server, _)| (packet, next_server)).collect();
        return ret_packets;
    }
}

impl MixnetPacket for Vec<u8> {
    async fn decrypt_incoming_packets(
        enc_packets: Vec<Vec<u8>>,
        cur_server: u64,
        _ : &Network,
        conns: &Arc<RwLock<Connections>>,
        layer: u64,
    ) -> Vec<(MixnetPacketType, u64)> {
        let conns_guard = conns.read().await;
        let dec_packets = enc_packets
            .par_iter()
            .filter_map(|enc_packet| {
                // I'll decrypt the packet and return the packet, layer, and connection
                let dec_packet = decrypt_packet_layer(enc_packet, cur_server, &(*conns_guard), layer);
                return dec_packet.map(|(packet, next_server)| 
                    (MixnetPacketType::Packet(packet), 
                    next_server
                ));
            })
            .collect();
        return dec_packets;
    }
}