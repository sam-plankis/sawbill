use anyhow;
use tokio;
#[macro_use]
extern crate rocket;
extern crate pnet;
extern crate redis;
mod connection;
mod datagram;
mod tcpdb;

use std::os::unix::thread;
// use std::sync::{Arc, Mutex};
use std::sync::Arc;
// use rocket::tokio::sync::{Arc};
use rocket::futures::lock::Mutex;

use connection::TcpConnection;
use datagram::TcpDatagram;
use pnet::packet::icmpv6::ndp::NdpOptionPacket;
use tcpdb::TcpDatabase;

use regex::Regex;

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::{Tcp, TcpPacket};
use pnet::packet::Packet;
use pnet::util::MacAddr;
use tokio::sync::futures;
use warp::reply::{json, Reply};

use std::cell::Cell;
use std::env;
use std::io::{self, Write};
use std::net::IpAddr;
use std::ops::Deref;
use std::process;

use env_logger;
use log::{debug, error, info, log_enabled, warn, Level};

use clap::Parser;
use serde_json;

use rocket::serde::{Serialize, json::Json};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    interface: String,
}

fn parse_tcp_ipv4_datagram(ethernet: &EthernetPacket) -> Option<TcpDatagram> {
    if let Some(packet) = Ipv4Packet::new(ethernet.payload()) {
        match packet.get_next_level_protocol() {
            IpNextHeaderProtocols::Tcp => {
                let tcp_datagram = TcpDatagram::new(packet);
                return Some(tcp_datagram);
            }
            IpNextHeaderProtocols::Udp => {}
            _ => return None,
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
        }
        None => None,
    }
}

fn identify_flow_direction(local_ipv4: &String, tcp_datagram: &TcpDatagram) -> Option<String> {
    let src_ip = tcp_datagram.get_src_ip();
    let dst_ip = tcp_datagram.get_dst_ip();
    if local_ipv4 == &dst_ip {
        return Some("a_to_z".to_string());
    }
    if local_ipv4 == &src_ip {
        return Some("z_to_a".to_string());
    }
    None
}

fn process_a_z_datagram(
    tcp_db: &mut TcpDatabase,
    tcp_connection: &TcpConnection,
    tcp_datagram: TcpDatagram,
) -> () {
    let flow = tcp_connection.get_flow();
    let a_ip = tcp_connection.get_a_ip();
    let z_ip = tcp_connection.get_z_ip();
    tcp_db.add_tcp_connection(&flow, &a_ip, &z_ip);
    tcp_db.add_a_z_seq_num(&flow, tcp_datagram.get_seq_num());
    tcp_db.add_a_z_ack_num(&flow, tcp_datagram.get_ack_num());
    if let Some(counter) = tcp_db.increment_a_to_z_syn_counter(&flow) {
        if counter == 3 {
            warn!("{} | 3 unanswered SYN packets", flow);
        }
    }
}

fn process_z_a_datagram(
    tcp_db: &mut TcpDatabase,
    tcp_connection: &TcpConnection,
    tcp_datagram: TcpDatagram,
) -> () {
    let flow = tcp_connection.get_flow();
    let a_ip = tcp_connection.get_a_ip();
    let z_ip = tcp_connection.get_z_ip();
    tcp_db.add_tcp_connection(&flow, &a_ip, &z_ip);
    tcp_db.add_z_a_seq_num(&flow, tcp_datagram.get_seq_num());
    tcp_db.add_z_a_ack_num(&flow, tcp_datagram.get_ack_num());
    if let Some(counter) = tcp_db.increment_z_to_a_syn_counter(&flow) {
        if counter == 3 {
            warn!("{} | 3 unanswered SYN packets", flow);
        }
    }
}

fn process_tcp_datagram(
    local_ipv4: &String,
    tcp_db: &mut TcpDatabase,
    tcp_datagram: TcpDatagram,
) -> Option<TcpConnection> {
    debug!("{:#?}", tcp_datagram.get_offset());
    debug!("{:#?}", tcp_datagram.get_options());
    let src_ip = tcp_datagram.get_src_ip();
    let src_port = tcp_datagram.get_src_port();
    let dst_ip = tcp_datagram.get_dst_ip();
    let dst_port = tcp_datagram.get_dst_port();
    if let Some(flow_direction) = identify_flow_direction(local_ipv4, &tcp_datagram) {
        match flow_direction.as_str() {
            "a_to_z" => {
                let tcp_conn = TcpConnection::new(src_ip, src_port, dst_ip, dst_port);
                process_a_z_datagram(tcp_db, &tcp_conn, tcp_datagram);
                Some(tcp_conn.clone())
            }
            "z_to_a" => {
                let tcp_conn = TcpConnection::new(dst_ip, dst_port, src_ip, src_port);
                process_z_a_datagram(tcp_db, &tcp_conn, tcp_datagram);
                Some(tcp_conn.clone())
            }
            _ => None,
        }
    } else {
        error!("Unable to identify flow direction!");
        None
    }
}

async fn lookup_ip(ip: &str) -> String {
    let url = format!(
        "http://demo.ip-api.com/json/{}?fields=66846719",
        ip.to_string()
    );
    if let Ok(resp) = reqwest::get(url).await {
        if let Ok(text) = resp.text().await {
            let pretty = serde_json::to_string_pretty(&text).unwrap();
            return pretty;
        }
    }
    return "None".to_string();
}

#[get("/")]
async fn index() -> &'static str {
    "Hello, world!"
}

use serde_derive::Deserialize;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpInfo {
    pub status: String,
    pub continent: String,
    pub continent_code: String,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub region_name: String,
    pub city: String,
    pub district: String,
    pub zip: String,
    pub lat: f64,
    pub lon: f64,
    pub timezone: String,
    pub offset: i64,
    pub currency: String,
    pub isp: String,
    pub org: String,
    #[serde(rename = "as")]
    pub as_field: String,
    pub asname: String,
    pub reverse: String,
    pub mobile: bool,
    pub proxy: bool,
    pub hosting: bool,
    pub query: String,
}


#[get("/conn")]
async fn conn(latest_tcp: &rocket::State<Arc<Mutex<Option<TcpConnection>>>>) -> Option<String> {
    // Just return a JSON array of todos, applying the limit and offset.
    let conn_clone = latest_tcp.clone();
    let mut guard = conn_clone.lock().await;
    match &mut *guard {
        Some(tcp_conn) => {
            let ip = tcp_conn.get_a_ip().to_owned();
            // let ip_info = lookup_ip(&ip).await;
            // let pretty = serde_json::to_string_pretty(&ip_info).unwrap();
            let url = format!(
                "http://demo.ip-api.com/json/{}?fields=66846719",
                ip.to_string()
            );
            println!("{}", url);
            // println!("{} | {:#?}", flow, tcp_conn);
            if let Ok(resp) = reqwest::get(url).await {
                if let Ok(text) = resp.text().await {
                    // let ip_info: Result<IpInfo, serde_json::Error> = serde_json::from_str(&text.as_str()).unwrap();
                    // error!("{} | {:#?}", ip, text);
                    // let text_json = serde_json::(text);
                    // error!("{:#?}", text_json);
                    // let pretty = serde_json::to_string_pretty(&text_json).unwrap();
                    return Some(text);
                }
            }
            return Some("None".to_string());
        }
        // None => Err(warp::reject::reject()),
        None => {
            let empty = "No latest conn".to_string();
            let pretty = serde_json::to_string_pretty(&empty).unwrap();
            Some(pretty)
        }
    }
}

async fn rx_loop(latest_tcp: Arc<Mutex<Option<TcpConnection>>>) {
    println!("Starting rx loop");
    let mut count = 0;
    // Make this an arg again
    let ipv4: &str = "*";

    // env_logger::init();
    let args = Args::parse();
    let iface_name: String = args.interface;

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
        Ok(_) => panic!("packetdump: unhandled channel type"),
        Err(e) => panic!("packetdump: unable to create channel: {}", e),
    };

    let local_ipv4 =
        get_local_ipv4(&interface).expect("Could not identify local Ipv4 address for interface");
    let mut tcp_db: TcpDatabase = TcpDatabase::new();

    if let Some(keys) = tcp_db.get_redis_keys() {
        debug!("{:#?}", keys)
    }

    loop {
        match rx.next() {
            Ok(packet) => {
                let ethernet = &EthernetPacket::new(packet).unwrap();
                if let Some(tcp_datagram) = parse_tcp_ipv4_datagram(&ethernet) {
                    let flow = tcp_datagram.get_flow();
                    match ipv4 {
                        "*" => {}
                        _ => {
                            if !flow.contains(ipv4) {
                                continue;
                            }
                        }
                    }
                    // Exclude the redis connection itself.
                    if flow.contains("6379") {
                        debug!("Skipped redis packet");
                        continue;
                    }
                    if let Some(tcp_conn) =
                        process_tcp_datagram(&local_ipv4, &mut tcp_db, tcp_datagram)
                    {
                        count += 1;
                        debug!("Count: {}", count);
                        // println!("{} | {:#?}", flow, tcp_conn);
                        // *latest_tcp.lock().unwrap() = Some(tcp_conn);
                        let latest_clone = latest_tcp.clone();
                        let mut conn_lock = latest_clone.deref().lock().await;
                        *conn_lock = Some(tcp_conn);
                    }
                }
            }
            Err(e) => panic!("packetdump: unable to receive packet: {}", e),
        }
    }
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    // Shared state between threads
    let latest_tcp: Arc<Mutex<Option<TcpConnection>>>;
    latest_tcp = Arc::new(Mutex::new(None));

    // Build and launch the Rocket web server
    let clone1 = latest_tcp.clone();
    let rocket = rocket::build()
        .manage(clone1)
        .mount("/", routes![index, conn]);
    let thread1 = rocket::tokio::spawn(rocket.launch());

    // Launch the packet capture thread
    let clone2 = latest_tcp.clone();
    let _ = rocket::tokio::task::spawn(async move { rx_loop(clone2).await });

    // Await the Rocket thread ONLY so Ctrl-C works
    _ = thread1.await.unwrap();

    Ok(())
}
