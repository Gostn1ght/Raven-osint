//! Geo-intelligence: IP geolocation, GeoSpy/Picarta image geo-tagging,
//! OpenSky aircraft tracking, USGS earthquake feed.

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

// ─── IP Geolocation ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct GeoIpResult {
    pub ip:        String,
    pub lat:       Option<f64>,
    pub lon:       Option<f64>,
    pub city:      Option<String>,
    pub country:   Option<String>,
    pub isp:       Option<String>,
    pub asn:       Option<String>,
    pub is_vpn:    Option<bool>,
    pub is_tor:    Option<bool>,
    pub is_proxy:  Option<bool>,
}

pub async fn geoip(ip: &str) -> anyhow::Result<GeoIpResult> {
    let client = http_client();
    // ip-api.com has a free tier (no key needed, 45 req/min)
    let url = format!(
        "http://ip-api.com/json/{}?fields=status,lat,lon,city,country,isp,as,proxy,hosting",
        ip
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    Ok(GeoIpResult {
        ip: ip.to_string(),
        lat:      resp["lat"].as_f64(),
        lon:      resp["lon"].as_f64(),
        city:     resp["city"].as_str().map(|s| s.into()),
        country:  resp["country"].as_str().map(|s| s.into()),
        isp:      resp["isp"].as_str().map(|s| s.into()),
        asn:      resp["as"].as_str().map(|s| s.into()),
        is_vpn:   resp["hosting"].as_bool(),
        is_tor:   None,
        is_proxy: resp["proxy"].as_bool(),
    })
}

// ─── Image Geolocation (Picarta) ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageGeoResult {
    pub lat:       f64,
    pub lon:       f64,
    pub country:   Option<String>,
    pub region:    Option<String>,
    pub confidence: Option<f64>,
}

pub async fn picarta_geolocate(image_url: &str, api_key: &str) -> anyhow::Result<ImageGeoResult> {
    let client = http_client();
    let payload = serde_json::json!({ "url": image_url, "top_k": 1 });
    let resp: serde_json::Value = client
        .post("https://picarta.ai/classify")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send().await?.json().await?;
    Ok(ImageGeoResult {
        lat:        resp["predictions"][0]["latitude"].as_f64().unwrap_or(0.0),
        lon:        resp["predictions"][0]["longitude"].as_f64().unwrap_or(0.0),
        country:    resp["predictions"][0]["country"].as_str().map(|s| s.into()),
        region:     resp["predictions"][0]["region"].as_str().map(|s| s.into()),
        confidence: resp["predictions"][0]["score"].as_f64(),
    })
}

// ─── OpenSky — live aircraft ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Aircraft {
    pub icao24:    String,
    pub callsign:  Option<String>,
    pub lat:       Option<f64>,
    pub lon:       Option<f64>,
    pub altitude:  Option<f64>,
    pub velocity:  Option<f64>,
    pub heading:   Option<f64>,
    pub on_ground: bool,
}

pub async fn opensky_states(
    lat_min: f64, lat_max: f64, lon_min: f64, lon_max: f64,
) -> anyhow::Result<Vec<Aircraft>> {
    let client = http_client();
    let url = format!(
        "https://opensky-network.org/api/states/all?lamin={}&lamax={}&lomin={}&lomax={}",
        lat_min, lat_max, lon_min, lon_max
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    let mut aircraft = Vec::new();
    if let Some(states) = resp["states"].as_array() {
        for s in states {
            let arr = match s.as_array() { Some(a) => a, None => continue };
            aircraft.push(Aircraft {
                icao24:    arr.get(0).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                callsign:  arr.get(1).and_then(|v| v.as_str()).map(|s| s.trim().to_string()),
                lon:       arr.get(5).and_then(|v| v.as_f64()),
                lat:       arr.get(6).and_then(|v| v.as_f64()),
                altitude:  arr.get(7).and_then(|v| v.as_f64()),
                velocity:  arr.get(9).and_then(|v| v.as_f64()),
                heading:   arr.get(10).and_then(|v| v.as_f64()),
                on_ground: arr.get(8).and_then(|v| v.as_bool()).unwrap_or(false),
            });
        }
    }
    Ok(aircraft)
}

// ─── USGS Earthquakes ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Earthquake {
    pub id:        String,
    pub place:     String,
    pub magnitude: f64,
    pub time:      i64,
    pub lat:       f64,
    pub lon:       f64,
    pub depth:     f64,
}

pub async fn usgs_earthquakes(min_magnitude: f64) -> anyhow::Result<Vec<Earthquake>> {
    let client = http_client();
    let url = format!(
        "https://earthquake.usgs.gov/fdsnws/event/1/query?format=geojson&limit=100&minmagnitude={}",
        min_magnitude
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    let mut quakes = Vec::new();
    if let Some(features) = resp["features"].as_array() {
        for f in features {
            let props = &f["properties"];
            let coords = &f["geometry"]["coordinates"];
            quakes.push(Earthquake {
                id:        f["id"].as_str().unwrap_or("").to_string(),
                place:     props["place"].as_str().unwrap_or("Unknown").to_string(),
                magnitude: props["mag"].as_f64().unwrap_or(0.0),
                time:      props["time"].as_i64().unwrap_or(0),
                lon:       coords[0].as_f64().unwrap_or(0.0),
                lat:       coords[1].as_f64().unwrap_or(0.0),
                depth:     coords[2].as_f64().unwrap_or(0.0),
            });
        }
    }
    Ok(quakes)
}

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("RavensNexus/1.0")
        .build()
        .unwrap()
}
