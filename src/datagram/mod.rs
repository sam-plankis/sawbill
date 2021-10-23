use log::{debug, error, log_enabled, info, Level, warn};
use std::net::IpAddr;
use pnet::packet::Packet;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;

#[derive(Debug)]
pub struct TcpDatagram{
    src_ip: IpAddr,
    src_port: u16,
    dst_ip: IpAddr,
    dst_port: u16,
    bytes: u32,
    ack_num: u32,
    seq_num: u32,
    flags: u16,
}

impl TcpDatagram {
    pub fn new(packet: Ipv4Packet) -> Self {
        let src_ip = IpAddr::V4(packet.get_source());
        let dst_ip = IpAddr::V4(packet.get_destination());
        let payload= packet.payload();
        let bytes = payload.len() as u32;
        let tcp = TcpPacket::new(payload).expect("Could not parse TCP datagram!");
        let flags: u16 = tcp.get_flags();
        let dst_port: u16 = tcp.get_destination();
        let src_port: u16 = tcp.get_source();
        let ack_num: u32 = tcp.get_acknowledgement();
        let seq_num: u32 = tcp.get_sequence();
        Self {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            bytes, 
            ack_num, 
            seq_num,
            flags, 
        }
    }

    pub fn get_flow(&self) -> String {
        format!("{}:{}->{}:{}", self.src_ip, self.src_port, self.dst_ip, self.dst_port)
    }

    pub fn get_seq_num(&self) -> u32 {
        self.seq_num
    }

    pub fn get_ack_num(&self) -> u32 {
        self.ack_num
    }

    pub fn get_bytes(&self) -> u32 {
        self.bytes
    }

    pub fn get_src_ip(&self) -> String {
        format!("{}", self.src_ip)
    }

    pub fn get_dst_ip(&self) -> String {
        format!("{}", self.dst_ip)
    }

    pub fn get_src_port(&self) -> u16 {
        self.src_port
    }

    pub fn get_dst_port(&self) -> u16 {
        self.dst_port
    }

    pub fn is_syn(&self) -> bool {
        match self.flags {
            2 => { true }
            _ => { false }
        }
    }
}