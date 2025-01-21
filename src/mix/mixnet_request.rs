use serde::{Deserialize, Serialize};
use crate::mix::*;
use crate::network::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MixnetPacketType {
    Packet(Vec<u8>),
    SetupPacket(SetupPacket),
}

pub trait MixnetPacket {
    fn is_setup_packet() -> bool;

    async fn decrypt_incoming_packets(
        enc_packets: Vec<Vec<u8>>,
        cur_server: u64,
        network: &Network,
        conns: &Arc<RwLock<Connections>>,
        layer: u64,
    ) -> Vec<(MixnetPacketType, u64)>;

    fn handle_verify_on_output(
        packets: &mut Vec<MixnetPacketType>,
        network_info: &Network,
        layer: u32,
        src: u16,
        dst: u16,
        total_outgoing: usize,
    );
}

impl MixnetPacket for SetupPacket {
    fn is_setup_packet() -> bool {
        return true;
    }

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
        for (_, _, conn) in &dec_packets {
            (*conns_guard).insert(cur_server.clone(), conn.conn_id.clone(), conn.clone());
        }
        drop(conns_guard);
        // remove the connections from dec_packets without copying the rest of the data
        let ret_packets = dec_packets.into_iter().map(|(packet, next_server, _)| (packet, next_server)).collect();
        return ret_packets;
    }

    /// Verify outgoing packets when MIX_VERIFICATION is set to OnlyVerifyEdgeCases
    fn handle_verify_on_output(
            packets: &mut Vec<MixnetPacketType>,
            network_info: &Network,
            layer: u32,
            src: u16,
            dst: u16,
            total_outgoing: usize,
        ) {
            match *MIX_VERIFICATION {
                MixnetVerification::OnlyVerifyEdgeCases => {
                    // In the case of a middle layer outputing to mix
                    if is_out_of_bounds(packets.len(), total_outgoing.clone())
                    {
                        info!("mix {} is verifying edge case to mix {}", src, dst);
                        // Verify all packets, "throw away" all non valid packets
                        verify_outgoing_setup_packets(packets, network_info, layer);
                    }
                },
                _ => (),
            }
    }
}

impl MixnetPacket for Vec<u8> {
    fn is_setup_packet() -> bool {
        return false;
    }

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

    fn handle_verify_on_output(
        _: &mut Vec<MixnetPacketType>,
        _: &Network,
        _: u32,
        _: u16,
        _: u16,
        _: usize,
        ) {
        // Do nothing
    }
}

/// Verifies outgoing packets, drops invalid ones.
fn verify_outgoing_setup_packets(packets: &mut Vec<MixnetPacketType>, network_info: &Network, layer: u32) {
    *packets = packets
        .par_iter()
        .filter_map(|packet| {
            if let MixnetPacketType::SetupPacket(setup_packet) = packet {
                // Verify the setup packet
                if verify_setup_packet(setup_packet, network_info, (layer - 1) as u64).1 {
                    return Some(packet.clone());
                }
            }
            None
        })
        .collect();
}