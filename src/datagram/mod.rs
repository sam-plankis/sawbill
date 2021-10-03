use std::net::IpAddr;

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
    pub fn new(src_ip: IpAddr, src_port: u16, dst_ip: IpAddr, dst_port: u16, bytes: u32, flags: u16) -> Self {
        let ack_num: u32= 0;
        let seq_num: u32= 0;
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