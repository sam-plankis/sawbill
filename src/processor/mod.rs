use crate::datagram::TcpDatagram;
use crate::tcp_db::TcpDb;
use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::Packet;
use rocket::futures::lock::Mutex;
use std::ops::Deref;
use std::sync::Arc;

fn parse_tcp_ipv4_datagram(ethernet: &EthernetPacket) -> Option<TcpDatagram> {
    if let Some(packet) = Ipv4Packet::new(ethernet.payload()) {
        match packet.get_next_level_protocol() {
            IpNextHeaderProtocols::Tcp => {
                let tcp_datagram = TcpDatagram::new(packet);
                return Some(tcp_datagram);
            }
            IpNextHeaderProtocols::Udp => {}
            _ => return None,
        }
    }
    None
}

pub async fn process(
    ipv4_filter: String,
    interface: NetworkInterface,
    tcp_db: Arc<Mutex<TcpDb>>,
    count: Arc<Mutex<u32>>,
) -> () {
    // Create a channel to receive on
    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("packetdump: unhandled channel type"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };
    loop {
        match rx.next() {
            Ok(packet) => {
                let ethernet = &EthernetPacket::new(packet).unwrap();
                if let Some(datagram) = parse_tcp_ipv4_datagram(&ethernet) {
                    match ipv4_filter.as_str() {
                        "*" => {}
                        _ => {
                            if datagram.src_ip.to_string() != ipv4_filter
                                && datagram.dst_ip.to_string() != ipv4_filter
                            {
                                continue;
                            }
                        }
                    }
                    // Add the TCP datagram to the TCP database
                    let tcp_db_clone = tcp_db.clone();
                    let mut db_lock = tcp_db_clone.deref().lock().await;
                    let ref mut db = *db_lock;
                    db.add(datagram);
                    // Update total TCP datagram count
                    let count_clone = count.clone();
                    let mut count_lock = count_clone.deref().lock().await;
                    *count_lock += 1;
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}
