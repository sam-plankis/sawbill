use crate::connection::TcpConnection;
use crate::datagram::TcpDatagram;
use crate::tcpdb::TcpDatabase;

use pnet::datalink::Channel::Ethernet;

use rocket::futures::lock::Mutex;
use std::sync::Arc;

use regex::Regex;

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::Packet;
use std::ops::Deref;

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

fn get_local_ipv4(interface: &NetworkInterface) -> Option<String> {
    debug!("{}", &interface.to_string());
    let regexp = Regex::new(r"(\d+\.\d+\.\d+\.\d+)").unwrap();
    match regexp.captures(interface.to_string().as_str()) {
        Some(captures) => {
            let local_ipv4 = captures.get(0).unwrap().as_str().to_string();
            debug!("Found local Ipv4 address: {:#?}", local_ipv4);
            Some(local_ipv4)
        }
        None => None,
    }
}

fn identify_flow_direction(local_ipv4: &String, tcp_datagram: &TcpDatagram) -> Option<String> {
    let src_ip = tcp_datagram.get_src_ip();
    let dst_ip = tcp_datagram.get_dst_ip();
    if local_ipv4 == &dst_ip {
        return Some("a_to_z".to_string());
    }
    if local_ipv4 == &src_ip {
        return Some("z_to_a".to_string());
    }
    None
}

fn process_a_z_datagram(
    tcp_db: &mut TcpDatabase,
    tcp_connection: &TcpConnection,
    tcp_datagram: TcpDatagram,
) -> () {
    let flow = tcp_connection.get_flow();
    let a_ip = tcp_connection.get_a_ip();
    let z_ip = tcp_connection.get_z_ip();
    tcp_db.add_tcp_connection(&flow, &a_ip, &z_ip);
    tcp_db.add_a_z_seq_num(&flow, tcp_datagram.get_seq_num());
    tcp_db.add_a_z_ack_num(&flow, tcp_datagram.get_ack_num());
    if let Some(counter) = tcp_db.increment_a_to_z_syn_counter(&flow) {
        if counter == 3 {
            warn!("{} | 3 unanswered SYN packets", flow);
        }
    }
}

fn process_z_a_datagram(
    tcp_db: &mut TcpDatabase,
    tcp_connection: &TcpConnection,
    tcp_datagram: TcpDatagram,
) -> () {
    let flow = tcp_connection.get_flow();
    let a_ip = tcp_connection.get_a_ip();
    let z_ip = tcp_connection.get_z_ip();
    tcp_db.add_tcp_connection(&flow, &a_ip, &z_ip);
    tcp_db.add_z_a_seq_num(&flow, tcp_datagram.get_seq_num());
    tcp_db.add_z_a_ack_num(&flow, tcp_datagram.get_ack_num());
    if let Some(counter) = tcp_db.increment_z_to_a_syn_counter(&flow) {
        if counter == 3 {
            warn!("{} | 3 unanswered SYN packets", flow);
        }
    }
}

fn process_tcp_datagram(
    local_ipv4: &String,
    tcp_db: &mut TcpDatabase,
    tcp_datagram: TcpDatagram,
) -> Option<TcpConnection> {
    debug!("{:#?}", tcp_datagram.get_offset());
    debug!("{:#?}", tcp_datagram.get_options());
    let src_ip = tcp_datagram.get_src_ip();
    let src_port = tcp_datagram.get_src_port();
    let dst_ip = tcp_datagram.get_dst_ip();
    let dst_port = tcp_datagram.get_dst_port();
    if let Some(flow_direction) = identify_flow_direction(local_ipv4, &tcp_datagram) {
        match flow_direction.as_str() {
            "a_to_z" => {
                let tcp_conn = TcpConnection::new(src_ip, src_port, dst_ip, dst_port);
                process_a_z_datagram(tcp_db, &tcp_conn, tcp_datagram);
                Some(tcp_conn.clone())
            }
            "z_to_a" => {
                let tcp_conn = TcpConnection::new(dst_ip, dst_port, src_ip, src_port);
                process_z_a_datagram(tcp_db, &tcp_conn, tcp_datagram);
                Some(tcp_conn.clone())
            }
            _ => None,
        }
    } else {
        error!("Unable to identify flow direction!");
        None
    }
}

pub async fn process(
    ipv4_filter: String,
    iface_name: String,
    latest_tcp: Arc<Mutex<Option<TcpConnection>>>,
    count: Arc<Mutex<u32>>,
) -> () {
    // Find the network interface with the provided name
    let interface_names_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap_or_else(|| panic!("No such network interface: {}", iface_name));
    // Create a channel to receive on
    let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("packetdump: unhandled channel type"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let local_ipv4 =
        get_local_ipv4(&interface).expect("Could not identify local Ipv4 address for interface");
    let mut tcp_db: TcpDatabase = TcpDatabase::new();

    if let Some(keys) = tcp_db.get_redis_keys() {
        debug!("{:#?}", keys)
    }

    loop {
        match rx.next() {
            Ok(packet) => {
                let ethernet = &EthernetPacket::new(packet).unwrap();
                if let Some(tcp_datagram) = parse_tcp_ipv4_datagram(&ethernet) {
                    let flow = tcp_datagram.get_flow();
                    match ipv4_filter.as_str() {
                        "*" => {}
                        _ => {
                            if !flow.contains(ipv4_filter.as_str()) {
                                continue;
                            }
                        }
                    }
                    // Exclude the redis connection itself.
                    if flow.contains("6379") {
                        debug!("Skipped redis packet");
                        continue;
                    }
                    if let Some(tcp_conn) =
                        process_tcp_datagram(&local_ipv4, &mut tcp_db, tcp_datagram)
                    {
                        let count_clone = count.clone();
                        let mut count_lock = count_clone.deref().lock().await;
                        *count_lock += 1;
                        // println!("{} | {:#?}", flow, tcp_conn);
                        // *latest_tcp.lock().unwrap() = Some(tcp_conn);
                        let latest_clone = latest_tcp.clone();
                        let mut conn_lock = latest_clone.deref().lock().await;
                        *conn_lock = Some(tcp_conn);
                    }
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}
