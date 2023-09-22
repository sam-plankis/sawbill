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

pub async fn lookup_ip(ip: &str) -> Option<IpInfo> {
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
