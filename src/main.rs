#[macro_use]
extern crate rocket;
extern crate pnet;
use anyhow;
use clap::Parser;
use pnet::datalink::NetworkInterface;
use regex::Regex;
use rocket::futures::lock::Mutex;
use rocket::serde::json::Json;
use std::net::Ipv4Addr;
use std::sync::Arc;
mod connection;
mod datagram;
mod processor;
use processor::process;
mod tcp_db;
use tcp_db::TcpDb;

use rocket::http::ContentType;
#[get("/")]
fn index() -> (ContentType, &'static str) {
    let page = "<html>
        <head>
            <title>IP Info</title>
        </head>
        <body>
            <h1>IP Info</h1>
            <p>Visit <a href=\"/conn\">/conn</a> to see the latest connection.</p>
            </body>
        </html>";
    (ContentType::HTML, page)
}

#[get("/count")]
async fn count(count: &rocket::State<Arc<Mutex<u32>>>) -> String {
    let count_clone = count.clone();
    let mut guard = count_clone.lock().await;
    let count_val = &mut *guard;
    count_val.to_string()
}

#[get("/reset_count")]
async fn reset_count(count: &rocket::State<Arc<Mutex<u32>>>) -> String {
    let count_clone = count.clone();
    let mut guard = count_clone.lock().await;
    *guard = 0;
    0.to_string()
}

#[get("/tcpdb")]
async fn conn(tcp_db: &rocket::State<Arc<Mutex<TcpDb>>>) -> Json<TcpDb> {
    // Just return a JSON array of todos, applying the limit and offset.
    // let conns_clone = tcp_conns.clone();
    let guard = tcp_db.lock().await;
    let ref db = *guard;
    Json(db.to_owned())
}

fn get_local_ipv4(interface: &NetworkInterface) -> Option<Ipv4Addr> {
    debug!("{}", &interface.to_string());
    let regexp = Regex::new(r"(\d+)\.(\d+)\.(\d+)\.(\d+)").unwrap();
    match regexp.captures(interface.to_string().as_str()) {
        Some(captures) => {
            info!("captures: {:#?}", captures);
            let byte_1 = captures.get(1).unwrap().as_str().parse::<u8>().unwrap();
            let byte_2 = captures.get(2).unwrap().as_str().parse::<u8>().unwrap();
            let byte_3 = captures.get(3).unwrap().as_str().parse::<u8>().unwrap();
            let byte_4 = captures.get(4).unwrap().as_str().parse::<u8>().unwrap();
            Some(Ipv4Addr::new(byte_1, byte_2, byte_3, byte_4))
        }
        None => None,
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    interface: String,
    #[arg(long)]
    ipv4: String,
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    // env_logger::init();
    let args = Args::parse();
    let iface_name: String = args.interface;
    let ipv4: String = args.ipv4;

    // Find the network interface with the provided name
    let interface_names_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = pnet::datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap_or_else(|| panic!("No such network interface: {}", iface_name));

    let local_ipv4 =
        get_local_ipv4(&interface).expect("Could not identify local Ipv4 address for interface");

    // Shared state between threads
    let tcp_db: Arc<Mutex<TcpDb>>;
    tcp_db = Arc::new(Mutex::new(TcpDb::new(local_ipv4)));

    let count: Arc<Mutex<u32>>;
    count = Arc::new(Mutex::new(0));
    let count1 = count.clone();
    let count2 = count.clone();

    // Build and launch the Rocket web server
    let tcp_db_1 = tcp_db.clone();
    let rocket = rocket::build()
        .manage(tcp_db_1)
        .manage(count1)
        .mount("/", routes![index, conn, count, reset_count]);
    let thread1 = rocket::tokio::spawn(rocket.launch());

    // Launch the packet capture thread
    let tcp_db_2 = tcp_db.clone();
    let _ =
        rocket::tokio::task::spawn(async move { process(ipv4, interface, tcp_db_2, count2).await });

    // Await the Rocket thread ONLY so Ctrl-C works
    _ = thread1.await.unwrap();

    Ok(())
}
