extern crate pnet;
extern crate redis;

use crate::redis::Commands;
use redis::RedisResult;
use regex::Regex;

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::arp::ArpPacket;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::icmp::{echo_reply, echo_request, IcmpPacket, IcmpTypes};
use pnet::packet::icmpv6::Icmpv6Packet;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use pnet::packet::Packet;
use pnet::util::MacAddr;

use std::env;
use std::io::{self, Write};
use std::net::IpAddr;
use std::process;

#[derive(Debug)]
pub struct Ipv4TcpConnection{
    src_ip: IpAddr,
    src_port: u16,
    dst_ip: IpAddr,
    dst_port: u16,
    total_bytes: u64,
}

impl Ipv4TcpConnection {
    pub fn new(src_ip: IpAddr, src_port: u16, dst_ip: IpAddr, dst_port: u16) -> Self {
        let total_bytes: u64 = 0;
        Self {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            total_bytes,
        }
    }

    pub fn increment_total_bytes(&mut self, bytes: u64) -> Result<(), ()> {
        self.total_bytes += bytes;
        Ok(())
    }
}


fn handle_udp_packet(interface_name: &str, source: IpAddr, destination: IpAddr, packet: &[u8]) {
    let udp = UdpPacket::new(packet);

    if let Some(udp) = udp {
        println!(
            "[{}]: UDP Packet: {}:{} > {}:{}; length: {}",
            interface_name,
            source,
            udp.get_source(),
            destination,
            udp.get_destination(),
            udp.get_length()
        );
    } else {
        println!("[{}]: Malformed UDP Packet", interface_name);
    }
}

fn handle_icmp_packet(interface_name: &str, source: IpAddr, destination: IpAddr, packet: &[u8]) {
    let icmp_packet = IcmpPacket::new(packet);
    if let Some(icmp_packet) = icmp_packet {
        match icmp_packet.get_icmp_type() {
            IcmpTypes::EchoReply => {
                let echo_reply_packet = echo_reply::EchoReplyPacket::new(packet).unwrap();
                println!(
                    "[{}]: ICMP echo reply {} -> {} (seq={:?}, id={:?})",
                    interface_name,
                    source,
                    destination,
                    echo_reply_packet.get_sequence_number(),
                    echo_reply_packet.get_identifier()
                );
            }
            IcmpTypes::EchoRequest => {
                let echo_request_packet = echo_request::EchoRequestPacket::new(packet).unwrap();
                println!(
                    "[{}]: ICMP echo request {} -> {} (seq={:?}, id={:?})",
                    interface_name,
                    source,
                    destination,
                    echo_request_packet.get_sequence_number(),
                    echo_request_packet.get_identifier()
                );
            }
            _ => println!(
                "[{}]: ICMP packet {} -> {} (type={:?})",
                interface_name,
                source,
                destination,
                icmp_packet.get_icmp_type()
            ),
        }
    } else {
        println!("[{}]: Malformed ICMP Packet", interface_name);
    }
}

fn handle_icmpv6_packet(interface_name: &str, source: IpAddr, destination: IpAddr, packet: &[u8]) {
    let icmpv6_packet = Icmpv6Packet::new(packet);
    if let Some(icmpv6_packet) = icmpv6_packet {
        println!(
            "[{}]: ICMPv6 packet {} -> {} (type={:?})",
            interface_name,
            source,
            destination,
            icmpv6_packet.get_icmpv6_type()
        )
    } else {
        println!("[{}]: Malformed ICMPv6 Packet", interface_name);
    }
}

fn handle_tcp_packet(interface_name: &str, source: IpAddr, destination: IpAddr, packet: &[u8]) {
    let tcp = TcpPacket::new(packet);
    if let Some(tcp) = tcp {
        println!(
            "[{}]: TCP Packet: {}:{} > {}:{}; length: {}",
            interface_name,
            source,
            tcp.get_source(),
            destination,
            tcp.get_destination(),
            packet.len()
        );
    } else {
        println!("[{}]: Malformed TCP Packet", interface_name);
    }
}

fn handle_transport_protocol(
    interface_name: &str,
    source: IpAddr,
    destination: IpAddr,
    protocol: IpNextHeaderProtocol,
    packet: &[u8],
) {
    match protocol {
        IpNextHeaderProtocols::Udp => {
            handle_udp_packet(interface_name, source, destination, packet)
        }
        IpNextHeaderProtocols::Tcp => {
            handle_tcp_packet(interface_name, source, destination, packet)
        }
        IpNextHeaderProtocols::Icmp => {
            handle_icmp_packet(interface_name, source, destination, packet)
        }
        IpNextHeaderProtocols::Icmpv6 => {
            handle_icmpv6_packet(interface_name, source, destination, packet)
        }
        _ => println!(
            "[{}]: Unknown {} packet: {} > {}; protocol: {:?} length: {}",
            interface_name,
            match source {
                IpAddr::V4(..) => "IPv4",
                _ => "IPv6",
            },
            source,
            destination,
            protocol,
            packet.len()
        ),
    }
}

fn handle_ipv4_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = Ipv4Packet::new(ethernet.payload());
    if let Some(header) = header {
        handle_transport_protocol(
            interface_name,
            IpAddr::V4(header.get_source()),
            IpAddr::V4(header.get_destination()),
            header.get_next_level_protocol(),
            header.payload(),
        );
    } else {
        println!("[{}]: Malformed IPv4 Packet", interface_name);
    }
}

fn handle_ipv6_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = Ipv6Packet::new(ethernet.payload());
    if let Some(header) = header {
        handle_transport_protocol(
            interface_name,
            IpAddr::V6(header.get_source()),
            IpAddr::V6(header.get_destination()),
            header.get_next_header(),
            header.payload(),
        );
    } else {
        println!("[{}]: Malformed IPv6 Packet", interface_name);
    }
}

fn handle_arp_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = ArpPacket::new(ethernet.payload());
    if let Some(header) = header {
        println!(
            "[{}]: ARP packet: {}({}) > {}({}); operation: {:?}",
            interface_name,
            ethernet.get_source(),
            header.get_sender_proto_addr(),
            ethernet.get_destination(),
            header.get_target_proto_addr(),
            header.get_operation()
        );
    } else {
        println!("[{}]: Malformed ARP Packet", interface_name);
    }
}

fn handle_ethernet_frame(interface: &NetworkInterface, ethernet: &EthernetPacket) {
    let interface_name = &interface.name[..];
    match ethernet.get_ethertype() {
        EtherTypes::Ipv4 => handle_ipv4_packet(interface_name, ethernet),
        EtherTypes::Ipv6 => handle_ipv6_packet(interface_name, ethernet),
        EtherTypes::Arp => handle_arp_packet(interface_name, ethernet),
        _ => println!(
            "[{}]: Unknown packet: {} > {}; ethertype: {:?} length: {}",
            interface_name,
            ethernet.get_source(),
            ethernet.get_destination(),
            ethernet.get_ethertype(),
            ethernet.packet().len()
        ),
    }
}

fn connect_redis() -> redis::Connection {
    let client = redis::Client::open("redis://127.0.0.1:6379").expect("Could not connect to redis!");
    let mut con = client.get_connection().expect("Could not connect to redis!");
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

pub struct ConnBytes {
    conn: String,
    bytes: usize,
}

impl ConnBytes {
    pub fn new(conn: String, bytes: usize) -> Self {
        Self {
            conn,
            bytes
        }
    }
}

fn parse_connection_bytes(ethernet: &EthernetPacket) -> Option<ConnBytes> {
    if let Some(header) = Ipv4Packet::new(ethernet.payload()) {
        let src_ip = IpAddr::V4(header.get_source());
        let dst_ip = IpAddr::V4(header.get_destination());
        let packet = header.payload();
        let bytes = packet.len();
        match header.get_next_level_protocol() {
            IpNextHeaderProtocols::Tcp => {
                if let Some(tcp) = TcpPacket::new(packet) {
                    let dst_port = tcp.get_destination();
                    let src_port = tcp.get_source();
                    let conn_key = format!("{}:{}->{}:{}", src_ip, src_port, dst_ip, dst_port);
                    let conn_bytes = ConnBytes::new(conn_key, bytes);
                    return Some(conn_bytes)
                }
            }
            IpNextHeaderProtocols::Udp => { }
            _ => return None
        }
    }
    None
}

fn main() {
    use pnet::datalink::Channel::Ethernet;
    let src_ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 10, 1));
    let dst_ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 10, 2));
    println!("{:?}", src_ip);
    println!("{:?}", dst_ip);
    let src_port: u16 = 56789;
    let dst_port: u16 = 80;
    let ipv4_tcp_conn = Ipv4TcpConnection::new(
        src_ip,
        src_port,
        dst_ip,
        dst_port
    );

    let src_ip = ipv4_tcp_conn.src_ip;
    let src_port = ipv4_tcp_conn.src_port;
    let dst_ip = ipv4_tcp_conn.dst_ip;
    let dst_port = ipv4_tcp_conn.dst_port;
    let conn_key = format!("{}:{}::{}:{}", src_ip, src_port, dst_ip, dst_port);

    let mut redis_conn = connect_redis();
    if let Some(keys) = get_redis_keys(&mut redis_conn){
        println!("{:?}", keys);
    } 
    let iface_name = match env::args().nth(1) {
        Some(n) => n,
        None => {
            writeln!(io::stderr(), "USAGE: packetdump <NETWORK INTERFACE>").unwrap();
            process::exit(1);
        }
    };
    let interface_names_match = |iface: &NetworkInterface| iface.name == iface_name;

    // Find the network interface with the provided name
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

    loop {
        match rx.next() {
            Ok(packet) => { 
                let ethernet = &EthernetPacket::new(packet).unwrap();
                if let Some(conn_bytes) = parse_connection_bytes(&ethernet) {
                    let conn_key = conn_bytes.conn;
                    if conn_key.contains("6379") {
                        continue
                    } else {
                        {}
                    }
                    let bytes = conn_bytes.bytes;
                    if let Ok(current_bytes) = get_conn_key_byte_counter(&mut redis_conn, &conn_key) {
                        increment_conn_bytes(&mut redis_conn, &conn_key, bytes).expect("Could not update redis key!");
                        let total_bytes = current_bytes as usize + bytes;
                        println!("Updated existing key | {} | {} total bytes", conn_key, total_bytes)
                    } else {
                        add_conn_to_redis(&mut redis_conn, &conn_key).expect("Could not create redis key!");
                        increment_conn_bytes(&mut redis_conn, &conn_key, bytes).expect("Could not update redis key!");
                        println!("Created new key | {} | {} starting bytes", conn_key, bytes)
                    }
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }

    loop {
        let mut buf: [u8; 1600] = [0u8; 1600];
        let mut fake_ethernet_frame = MutableEthernetPacket::new(&mut buf[..]).unwrap();
        match rx.next() {
            Ok(packet) => {
                let payload_offset;
                if cfg!(any(target_os = "macos", target_os = "ios"))
                    && interface.is_up()
                    && !interface.is_broadcast()
                    && ((!interface.is_loopback() && interface.is_point_to_point())
                        || interface.is_loopback())
                {
                    if interface.is_loopback() {
                        // The pnet code for BPF loopback adds a zero'd out Ethernet header
                        payload_offset = 14;
                    } else {
                        // Maybe is TUN interface
                        payload_offset = 0;
                    }
                    if packet.len() > payload_offset {
                        let version = Ipv4Packet::new(&packet[payload_offset..])
                            .unwrap()
                            .get_version();
                        if version == 4 {
                            fake_ethernet_frame.set_destination(MacAddr(0, 0, 0, 0, 0, 0));
                            fake_ethernet_frame.set_source(MacAddr(0, 0, 0, 0, 0, 0));
                            fake_ethernet_frame.set_ethertype(EtherTypes::Ipv4);
                            fake_ethernet_frame.set_payload(&packet[payload_offset..]);
                            handle_ethernet_frame(&interface, &fake_ethernet_frame.to_immutable());
                            continue;
                        } else if version == 6 {
                            fake_ethernet_frame.set_destination(MacAddr(0, 0, 0, 0, 0, 0));
                            fake_ethernet_frame.set_source(MacAddr(0, 0, 0, 0, 0, 0));
                            fake_ethernet_frame.set_ethertype(EtherTypes::Ipv6);
                            fake_ethernet_frame.set_payload(&packet[payload_offset..]);
                            handle_ethernet_frame(&interface, &fake_ethernet_frame.to_immutable());
                            continue;
                        }
                    }
                }
                handle_ethernet_frame(&interface, &EthernetPacket::new(packet).unwrap());
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}