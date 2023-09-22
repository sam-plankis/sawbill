use std::fmt::{self, Display, Formatter};

use rocket::data;
use serde::{Deserialize, Serialize};

use crate::datagram::TcpDatagram;

use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    flow: String,
    a_ip: IpAddr,
    a_port: u16,
    z_ip: IpAddr,
    z_port: u16,
    a_z_bytes: usize,
    z_a_bytes: usize,
    //handshake: bool,
    a_z_syn_counter: u32,
    z_a_syn_counter: u32,
    //a_z_dup_ack_nums: Vec<u32>,
    //z_a_dup_ack_nums: Vec<u32>,
    //a_z_dup_seq_nums: Vec<u32>,
    //z_a_dup_seq_nums: Vec<u32>,
    //datagrams: Vec<TcpDatagram>,
}

impl Display for TcpConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}<->{}:{}",
            self.a_ip, self.a_port, self.z_ip, self.z_port
        )
    }
}

impl TcpConnection {
    pub fn new(flow: String, a_ip: IpAddr, a_port: u16, z_ip: IpAddr, z_port: u16) -> Self {
        let z_a_syn_counter = 0;
        let a_z_syn_counter = 0;
        Self {
            a_ip,
            a_port,
            z_ip,
            z_port,
            a_z_bytes: 0,
            z_a_bytes: 0,
            a_z_syn_counter,
            z_a_syn_counter,
            flow,
            //datagrams: Vec::new(),
        }
    }

    pub fn add(&mut self, datagram: TcpDatagram) {
        //Increment byte count depending on flow direction
        if datagram.src_ip == self.a_ip && datagram.src_port == self.a_port {
            self.a_z_bytes += datagram.bytes as usize;
        } else {
            self.z_a_bytes += datagram.bytes as usize;
        }
    }
}
