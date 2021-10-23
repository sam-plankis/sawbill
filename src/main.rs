extern crate pnet;
extern crate redis;
mod datagram;
mod connection;
mod tcpdb;

use datagram::TcpDatagram;
use connection::TcpConnection;
use tcpdb::TcpDatabase;

use regex::Regex;

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::Packet;
use pnet::util::MacAddr;

use std::env;
use std::fmt::Result;
use std::io::{self, Write};
use std::net::IpAddr;
use std::process;

use clap::{App, load_yaml};
use env_logger;
use log::{debug, error, log_enabled, info, Level, warn};


fn parse_tcp_ipv4_datagram(ethernet: &EthernetPacket) -> Option<TcpDatagram> {
    if let Some(packet) = Ipv4Packet::new(ethernet.payload()) {
        match packet.get_next_level_protocol() {
            IpNextHeaderProtocols::Tcp => {
                let tcp_datagram = TcpDatagram::new(packet);
                return Some(tcp_datagram)
            }
            IpNextHeaderProtocols::Udp => { }
            _ => return None
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
        },
        None => None
    }
}

fn identify_flow_direction(local_ipv4: &String, tcp_datagram: &TcpDatagram) -> Option<String> {
    let src_ip = tcp_datagram.get_src_ip();
    let dst_ip = tcp_datagram.get_dst_ip();
    if local_ipv4 == &dst_ip {
        return Some("a_to_z".to_string())
    }
    if local_ipv4 == &src_ip {
        return Some("z_to_a".to_string())
    }
    None
}

fn process_a_z_datagram(tcp_db: &mut TcpDatabase, tcp_connection: &TcpConnection, tcp_datagram: TcpDatagram) -> () {
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

fn process_z_a_datagram(tcp_db: &mut TcpDatabase, tcp_connection: &TcpConnection, tcp_datagram: TcpDatagram) -> () {
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

fn process_tcp_datagram(local_ipv4: &String, tcp_db: &mut TcpDatabase, tcp_datagram: TcpDatagram) -> () {
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
            }
            "z_to_a" => {
                let tcp_conn = TcpConnection::new(dst_ip, dst_port, src_ip, src_port);
                process_z_a_datagram(tcp_db, &tcp_conn, tcp_datagram);
            }
            _ => { } 
        } 
    } else {
        error!("Unable to identify flow direction!")
    }
}

fn main() {
    env_logger::init();
    let yaml = load_yaml!("cli.yml");
    let args = App::from(yaml).get_matches();


    let ipv4: &str = args
        .value_of("ipv4")
        .unwrap_or("*");

    let iface_name: String = args
        .value_of("interface")
        .unwrap_or("lo0")
        .to_string();

    // Find the network interface with the provided name
    use pnet::datalink::Channel::Ethernet;
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
        Ok(_) => panic!("packetdump: unhandled channel type: {}"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let local_ipv4 = get_local_ipv4(&interface).expect("Could not identify local Ipv4 address for interface");
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
                    match ipv4 {
                        "*" => {{}}
                        _ => {
                            if !flow.contains(ipv4) {
                                continue
                            }
                        }
                    }
                    // Exclude the redis connection itself.
                    if flow.contains("6379") {
                        debug!("Skipped redis packet");
                        continue
                    }
                    process_tcp_datagram(&local_ipv4, &mut tcp_db, tcp_datagram);
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}