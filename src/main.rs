extern crate pnet;
extern crate redis;
mod datagram;
mod connection;

use datagram::TcpDatagram;
use connection::TcpConnection;

use crate::redis::Commands;
use redis::RedisResult;
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


fn connect_redis() -> redis::Connection {
    let client = redis::Client::open("redis://127.0.0.1:6379").expect("Could not connect to redis!");
    let con = client.get_connection().expect("Could not connect to redis!");
    con
}

fn increment_z_to_a_syn_counter(con: &mut redis::Connection, tcp_connection: &TcpConnection) -> Option<i32> {
    let flow = tcp_connection.get_flow();
    if let Some(counter) = redis::cmd("HINCRBY")
        .arg(&flow)
        .arg("z_to_a_syn_counter")
        .arg(1)
        .query(con)
        .unwrap() { return Some(counter) }
    None
}

fn get_z_to_a_syn_counter(con: &mut redis::Connection, tcp_connection: &TcpConnection) -> Option<i32> {
    let flow = tcp_connection.get_flow();
    if let Some(counter) = redis::cmd("HGET")
        .arg(&flow)
        .arg("z_to_a_syn_counter")
        .query(con)
        .unwrap() { return Some(counter) }
    None
}

fn get_redis_keys(con: &mut redis::Connection) -> Option<Vec<String>> {
    if let Some(keys) = redis::cmd("KEYS")
        .arg("*")
        .query(con)
        .expect("Could not get redis keys") { return Some(keys) }
    None
}

fn add_tcp_connection(con: &mut redis::Connection, tcp_connection: &TcpConnection) -> bool {
    let flow = tcp_connection.get_flow();
    let a_ip = tcp_connection.get_a_ip();
    let z_ip = tcp_connection.get_z_ip();
    let result: redis::RedisResult<String> = redis::cmd("HGET").arg(&flow).arg("a_ip").query(con);
    match result {

        // Connection already exists.
        Ok(_) => { return false; }

        // Create the connection.
        Err(_) => {
            let _: () = redis::cmd("HSET").arg(&flow).arg("a_to_z_syn_counter").arg(0).query(con).unwrap();
            let _: () = redis::cmd("HSET").arg(&flow).arg("z_to_a_syn_counter").arg(0).query(con).unwrap();
            let _: () = redis::cmd("HSET").arg(&flow).arg("a_ip").arg(a_ip).query(con).unwrap();
            let _: () = redis::cmd("HSET").arg(&flow).arg("z_ip").arg(z_ip).query(con).unwrap();
            return true
        }
    }
}

fn parse_tcp_ipv4_datagram(ethernet: &EthernetPacket) -> Option<TcpDatagram> {
    if let Some(header) = Ipv4Packet::new(ethernet.payload()) {
        let src_ip = IpAddr::V4(header.get_source());
        let dst_ip = IpAddr::V4(header.get_destination());
        let packet = header.payload();
        let bytes = packet.len() as u32;
        match header.get_next_level_protocol() {
            IpNextHeaderProtocols::Tcp => {
                if let Some(tcp) = TcpPacket::new(packet) {
                    let flags = tcp.get_flags();
                    let dst_port = tcp.get_destination();
                    let src_port = tcp.get_source();
                    let tcp_datagram= TcpDatagram::new(src_ip, src_port, dst_ip, dst_port, bytes, flags);
                    return Some(tcp_datagram)
                }
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
        debug!("Flow determination | local {} | src {} | dst {} | a_to_z", local_ipv4, src_ip, dst_ip);
        return Some("a_to_z".to_string())
    }
    if local_ipv4 == &src_ip {
        debug!("Flow determination | local {} | src {} | dst {} | z_to_a", local_ipv4, src_ip, dst_ip);
        return Some("z_to_a".to_string())
    }
    debug!("Flow determination | local {} | src {} | dst {} | failed", local_ipv4, src_ip, dst_ip);
    None
}

fn main() {
    env_logger::init();
    let yaml = load_yaml!("cli.yml");
    let args = App::from(yaml).get_matches();


    let mut redis_conn = connect_redis();
    if let Some(keys) = get_redis_keys(&mut redis_conn){
        println!("{:?}", keys);
    } 

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
                    if flow.contains("6379") {
                        continue
                    }

                    let src_ip = tcp_datagram.get_src_ip();
                    let src_port = tcp_datagram.get_src_port();
                    let dst_ip = tcp_datagram.get_dst_ip();
                    let dst_port = tcp_datagram.get_dst_port();
                    let flow_direction = identify_flow_direction(&local_ipv4, &tcp_datagram).unwrap();
                    match flow_direction.as_str() {
                        "a_to_z" => {
                            let tcp_connection = TcpConnection::new(src_ip, src_port, dst_ip, dst_port);
                            if add_tcp_connection(&mut redis_conn, &tcp_connection) {

                            } else {

                            }
                        }
                        "z_to_a" => {
                            let tcp_connection = TcpConnection::new(dst_ip, dst_port, src_ip, src_port);
                            if add_tcp_connection(&mut redis_conn, &tcp_connection) {
                                
                            } else {
                                if let Some(counter) = increment_z_to_a_syn_counter(&mut redis_conn, &tcp_connection) {
                                    debug!("{} | z_to_a_syn_counter: {}", flow, counter);
                                    if counter >= 3 {
                                        warn!("{} | 3 or more unanswered SYN packets", flow);
                                    }
                                }
                            }
                        }
                        _ => {  }
                    }
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}