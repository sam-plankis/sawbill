
use std::net::IpAddr;

#[derive(Debug)]
pub struct TcpConnection{
    a_ip: IpAddr,
    a_port: u16,
    z_ip: IpAddr,
    z_port: u16,
    //a_z_bytes: usize,
    //z_a_bytes: usize,
    //handshake: bool,
    //a_z_syn_counter: u32,
    z_a_syn_counter: u32,
    //a_z_dup_ack_nums: Vec<u32>,
    //z_a_dup_ack_nums: Vec<u32>,
    //a_z_dup_seq_nums: Vec<u32>,
    //z_a_dup_seq_nums: Vec<u32>,
}

impl TcpConnection {
    pub fn new(a_ip: IpAddr, a_port: u16, z_ip: IpAddr, z_port: u16) -> Self {
        let z_a_syn_counter = 0;
        Self {
            a_ip,
            a_port,
            z_ip,
            z_port,
            z_a_syn_counter,
        }
    }

    pub fn get_flows(&self) -> Vec<String> {
        let mut flows: Vec<String> = Vec::new();
        let a_z_flow = format!("{}:{}->{}:{}", self.a_ip, self.a_port, self.z_ip, self.z_port);
        let z_a_flow = format!("{}:{}->{}:{}", self.z_ip, self.z_port, self.a_ip, self.a_port);
        flows.push(a_z_flow);
        flows.push(z_a_flow);
        flows
    }

    pub fn get_a_ip(&self) -> String {
        format!("{}", self.a_ip)
    }

    pub fn get_z_ip(&self) -> String {
        format!("{}", self.z_ip)
    }

    pub fn get_a_port(&self) -> u16 {
        self.a_port
    }

    pub fn get_z_port(&self) -> u16 {
        self.z_port
    }

}