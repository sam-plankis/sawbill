use std::{collections::HashMap, net::Ipv4Addr};

use crate::connection::TcpConnection;
use crate::datagram::TcpDatagram;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpDb {
    local_ipv4: Ipv4Addr,
    flows: HashMap<String, TcpConnection>,
}

impl TcpDb {
    pub fn new(local_ipv4: Ipv4Addr) -> Self {
        let flows: HashMap<String, TcpConnection> = HashMap::new();
        Self { local_ipv4, flows }
    }

    fn parse_flow(&self, datagram: &TcpDatagram) -> String {
        if self.local_ipv4 == datagram.dst_ip {
            format!(
                "{}:{}<->{}:{}",
                datagram.src_ip, datagram.src_port, datagram.dst_ip, datagram.dst_port
            );
        }
        if self.local_ipv4 == datagram.src_ip {
            format!(
                "{}:{}<->{}:{}",
                datagram.dst_ip, datagram.dst_port, datagram.src_ip, datagram.src_port
            );
        }
        "unknown".to_string()
    }

    pub fn add(&mut self, datagram: TcpDatagram) -> () {
        let flow = self.parse_flow(&datagram);
        if !self.flows.contains_key(&flow) {
            let new_connection = TcpConnection::new(
                datagram.src_ip,
                datagram.src_port,
                datagram.dst_ip,
                datagram.dst_port,
            );
            self.flows.insert(flow.clone(), new_connection);
        }
        if let Some(connection) = self.flows.get_mut(&flow) {
            connection.add(datagram);
        }
    }
}
