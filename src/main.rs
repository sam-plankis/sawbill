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
use std::io::{self, Write};
use std::net::IpAddr;
use std::process;

use clap::{App, load_yaml};
use env_logger;
use log::{debug, error, log_enabled, info, Level};


fn connect_redis() -> redis::Connection {
    let client = redis::Client::open("redis://127.0.0.1:6379").expect("Could not connect to redis!");
    let con = client.get_connection().expect("Could not connect to redis!");
    con
}

fn add_conn_to_redis(con: &mut redis::Connection, conn_key: &String) -> redis::RedisResult<()> {
    let _ : () = redis::cmd("SET").arg(conn_key).arg(0).query(con)?;
    Ok(())
}

fn get_conn_key_byte_counter(con: &mut redis::Connection, conn_key: &String) -> redis::RedisResult<i32> {
    let count: i32 = con.get(conn_key)?;
    Ok(count)
}

fn increment_conn_bytes(con: &mut redis::Connection, conn_key: &String, bytes: usize) -> redis::RedisResult<()> {
    let _ : () = redis::cmd("INCRBY").arg(conn_key).arg(bytes).query(con)?;
    Ok(())
}

fn get_redis_keys(con: &mut redis::Connection) -> Option<Vec<String>> {
    if let Some(keys) = redis::cmd("KEYS")
        .arg("*")
        .query(con)
        .expect("Could not get redis keys") { return Some(keys) }
    None
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
    if local_ipv4.contains(&dst_ip) {
        return Some("a_to_z".to_string())
    }
    if local_ipv4.contains(&src_ip) {
        return Some("z_to_a".to_string())
    }
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
                    let bytes = tcp_datagram.get_bytes() as usize;
                    if let Ok(current_bytes) = get_conn_key_byte_counter(&mut redis_conn, &flow) {
                        increment_conn_bytes(&mut redis_conn, &flow, bytes).expect("Could not update redis key!");
                        let total_bytes = current_bytes as usize + bytes;
                        println!("Updated existing key | {} | {} total bytes", flow, total_bytes)
                    } else {
                        add_conn_to_redis(&mut redis_conn, &flow).expect("Could not create redis key!");
                        increment_conn_bytes(&mut redis_conn, &flow, bytes).expect("Could not update redis key!");
                        println!("Created new key | {} | {} starting bytes", flow, bytes);
                    }
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}