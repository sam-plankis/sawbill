use std::fmt::{Display, self, Formatter};

use serde::{Deserialize, Serialize};

use crate::datagram::TcpDatagram;

use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    a_ip: IpAddr,
    a_port: u16,
    z_ip: IpAddr,
    z_port: u16,
    //a_z_bytes: usize,
    //z_a_bytes: usize,
    //handshake: bool,
    a_z_syn_counter: u32,
    z_a_syn_counter: u32,
    //a_z_dup_ack_nums: Vec<u32>,
    //z_a_dup_ack_nums: Vec<u32>,
    //a_z_dup_seq_nums: Vec<u32>,
    //z_a_dup_seq_nums: Vec<u32>,
    flow: String,
    datagrams: Vec<TcpDatagram>,
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
    pub fn new(a_ip: IpAddr, a_port: u16, z_ip: IpAddr, z_port: u16) -> Self {
        let z_a_syn_counter = 0;
        let a_z_syn_counter = 0;
        let flow: String = format!("{}:{}<->{}:{}", a_ip, a_port, z_ip, z_port);
        Self {
            a_ip,
            a_port,
            z_ip,
            z_port,
            a_z_syn_counter,
            z_a_syn_counter,
            flow,
            datagrams: Vec::new(),
        }
    }

    pub fn add(&mut self, tcp_datagram: TcpDatagram) {
        self.datagrams.push(tcp_datagram);
    }

    pub fn get_flow(&self) -> String {
        self.flow.clone()
    }

    pub fn get_flows(&self) -> Vec<String> {
        let mut flows: Vec<String> = Vec::new();
        let a_z_flow = format!(
            "{}:{}->{}:{}",
            self.a_ip, self.a_port, self.z_ip, self.z_port
        );
        let z_a_flow = format!(
            "{}:{}->{}:{}",
            self.z_ip, self.z_port, self.a_ip, self.a_port
        );
        flows.push(a_z_flow);
        flows.push(z_a_flow);
        flows
    }

    pub fn get_a_z_flow(&self) -> String {
        let a_z_flow = format!(
            "{}:{}->{}:{}",
            self.a_ip, self.a_port, self.z_ip, self.z_port
        );
        a_z_flow
    }

    pub fn get_z_a_flow(&self) -> String {
        let z_a_flow = format!(
            "{}:{}->{}:{}",
            self.z_ip, self.z_port, self.a_ip, self.a_port
        );
        z_a_flow
    }

}
