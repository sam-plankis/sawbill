use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpOption;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::Packet;
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpDatagram {
    pub src_ip: IpAddr,
    pub src_port: u16,
    pub dst_ip: IpAddr,
    pub dst_port: u16,
    pub bytes: u32,
    pub ack_num: u32,
    pub seq_num: u32,
    pub flags: u8,
    pub offset: u8,
}

impl TcpDatagram {
    pub fn new(packet: Ipv4Packet) -> Self {
        let src_ip = IpAddr::V4(packet.get_source());
        let dst_ip = IpAddr::V4(packet.get_destination());
        let payload = packet.payload();
        let bytes = payload.len() as u32;
        let tcp = TcpPacket::new(payload).expect("Could not parse TCP datagram!");
        let flags: u8 = tcp.get_flags();
        let dst_port: u16 = tcp.get_destination();
        let src_port: u16 = tcp.get_source();
        let ack_num: u32 = tcp.get_acknowledgement();
        let seq_num: u32 = tcp.get_sequence();
        let offset: u8 = tcp.get_data_offset();
        Self {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            bytes,
            ack_num,
            seq_num,
            flags,
            offset,
        }
    }

    pub fn get_offset(&self) -> u8 {
        self.offset
    }

    pub fn is_syn(&self) -> bool {
        match self.flags {
            2 => true,
            _ => false,
        }
    }
}
