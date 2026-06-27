//! WHOIS + DNS resolver + Reverse IP lookup

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WhoisResult {
    pub domain:       String,
    pub registrar:    Option<String>,
    pub created:      Option<String>,
    pub updated:      Option<String>,
    pub expires:      Option<String>,
    pub status:       Vec<String>,
    pub name_servers: Vec<String>,
    pub registrant:   Option<String>,
    pub country:      Option<String>,
    pub raw:          String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DnsResult {
    pub domain: String,
    pub a:      Vec<String>,
    pub aaaa:   Vec<String>,
    pub mx:     Vec<String>,
    pub txt:    Vec<String>,
    pub ns:     Vec<String>,
    pub cname:  Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IpInfoResult {
    pub ip:       String,
    pub hostname: Option<String>,
    pub city:     Option<String>,
    pub region:   Option<String>,
    pub country:  Option<String>,
    pub org:      Option<String>,
    pub loc:      Option<String>,   // "lat,lon"
    pub timezone: Option<String>,
}

pub async fn lookup_whois(domain: &str) -> anyhow::Result<WhoisResult> {
    // Use whois-rust crate for raw WHOIS data, then parse key fields
    let raw = whois_rust::WhoIs::from_string(domain)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .lookup(whois_rust::WhoIsLookupOptions::from_string(domain)
            .map_err(|e| anyhow::anyhow!("{}", e))?)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut result = WhoisResult { domain: domain.to_string(), raw: raw.clone(), ..Default::default() };
    for line in raw.lines() {
        let lower = line.to_lowercase();
        if let Some(val) = extract_field(line, &["registrar:"]) { result.registrar = Some(val); }
        if let Some(val) = extract_field(line, &["creation date:", "created:"]) { result.created = Some(val); }
        if let Some(val) = extract_field(line, &["updated date:", "last updated:"]) { result.updated = Some(val); }
        if let Some(val) = extract_field(line, &["registry expiry date:", "expiry date:", "expires:"]) { result.expires = Some(val); }
        if lower.starts_with("name server:") || lower.starts_with("nserver:") {
            if let Some(val) = extract_field(line, &["name server:", "nserver:"]) {
                result.name_servers.push(val);
            }
        }
        if lower.starts_with("domain status:") {
            if let Some(val) = extract_field(line, &["domain status:"]) {
                result.status.push(val.split_whitespace().next().unwrap_or(&val).to_string());
            }
        }
        if let Some(val) = extract_field(line, &["registrant organization:", "registrant name:"]) {
            result.registrant = Some(val);
        }
        if let Some(val) = extract_field(line, &["registrant country:"]) { result.country = Some(val); }
    }
    Ok(result)
}

fn extract_field(line: &str, prefixes: &[&str]) -> Option<String> {
    for &pfx in prefixes {
        let l = line.to_lowercase();
        if l.starts_with(pfx) {
            return Some(line[pfx.len()..].trim().to_string());
        }
    }
    None
}

pub async fn lookup_dns(domain: &str) -> anyhow::Result<DnsResult> {
    use trust_dns_resolver::TokioAsyncResolver;
    use trust_dns_resolver::config::*;
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default());
    let mut result = DnsResult { domain: domain.to_string(), ..Default::default() };

    if let Ok(r) = resolver.lookup_ip(domain).await {
        for ip in r.iter() {
            if ip.is_ipv4() { result.a.push(ip.to_string()); }
            else { result.aaaa.push(ip.to_string()); }
        }
    }
    if let Ok(r) = resolver.mx_lookup(domain).await {
        for mx in r.iter() { result.mx.push(format!("{} {}", mx.preference(), mx.exchange())); }
    }
    if let Ok(r) = resolver.txt_lookup(domain).await {
        for txt in r.iter() {
            result.txt.push(txt.to_string());
        }
    }
    if let Ok(r) = resolver.ns_lookup(domain).await {
        for ns in r.iter() { result.ns.push(ns.to_string()); }
    }
    Ok(result)
}

pub async fn lookup_ip(ip: &str) -> anyhow::Result<IpInfoResult> {
    let client = Client::builder().timeout(Duration::from_secs(8)).build()?;
    let url = format!("https://ipinfo.io/{}/json", ip);
    let resp: serde_json::Value = client.get(&url)
        .header("Accept", "application/json")
        .send().await?.json().await?;
    Ok(IpInfoResult {
        ip:       ip.to_string(),
        hostname: resp["hostname"].as_str().map(|s| s.to_string()),
        city:     resp["city"].as_str().map(|s| s.to_string()),
        region:   resp["region"].as_str().map(|s| s.to_string()),
        country:  resp["country"].as_str().map(|s| s.to_string()),
        org:      resp["org"].as_str().map(|s| s.to_string()),
        loc:      resp["loc"].as_str().map(|s| s.to_string()),
        timezone: resp["timezone"].as_str().map(|s| s.to_string()),
    })
}
