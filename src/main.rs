#[macro_use]
extern crate rocket;

extern crate pnet;
extern crate redis;
use anyhow;



use rocket::futures::lock::Mutex;
use std::sync::Arc;

use connection::TcpConnection;



use env_logger;
use log::{debug, error, info, log_enabled, warn, Level};

use clap::Parser;
use serde_json;

use rocket::serde::{json::Json, Serialize};

mod connection;
mod datagram;
mod processor;
use processor::process;
mod tcpdb;

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

#[macro_use]
extern crate serde_derive;

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

#[get("/conn")]
async fn conn(
    latest_tcp: &rocket::State<Arc<Mutex<Option<TcpConnection>>>>,
) -> Option<Json<IpInfo>> {
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
                    let ip_info: IpInfo = serde_json::from_str(text.as_str()).unwrap();
                    return Some(Json(ip_info));
                }
            }
            return None;
        }
        None => {
            let empty = "No latest conn".to_string();
            let pretty = serde_json::to_string_pretty(&empty).unwrap();
            return None;
        }
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


    // Shared state between threads
    let latest_tcp: Arc<Mutex<Option<TcpConnection>>>;
    latest_tcp = Arc::new(Mutex::new(None));

    let count: Arc<Mutex<u32>>;
    count = Arc::new(Mutex::new(0));
    let count1 = count.clone();
    let count2 = count.clone();

    // Build and launch the Rocket web server
    let clone1 = latest_tcp.clone();
    let rocket = rocket::build()
        .manage(clone1)
        .manage(count1)
        .mount("/", routes![index, conn, count, reset_count]);
    let thread1 = rocket::tokio::spawn(rocket.launch());

    // Launch the packet capture thread
    let clone2 = latest_tcp.clone();
    let _ =
        rocket::tokio::task::spawn(async move { process(ipv4, iface_name, clone2, count2).await });

    // Await the Rocket thread ONLY so Ctrl-C works
    _ = thread1.await.unwrap();

    Ok(())
}
