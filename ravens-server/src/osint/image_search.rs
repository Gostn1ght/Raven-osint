//! Reverse image search / AI image identification via Lenso.ai

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct LensoResult {
    pub query_url: String,
    pub matches:   Vec<ImageMatch>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMatch {
    pub url:        String,
    pub source:     Option<String>,
    pub title:      Option<String>,
    pub similarity: Option<f64>,
    pub category:   Option<String>,   // "face", "place", "exact", etc.
}

pub async fn lenso_search(image_url: &str, api_key: &str) -> anyhow::Result<LensoResult> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("RavensNexus/1.0")
        .build()?;

    let payload = serde_json::json!({
        "imageUrl": image_url,
        "options": { "includeCategories": ["exact", "similar", "face", "place"] }
    });

    let resp: serde_json::Value = client
        .post("https://lenso.ai/api/search")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send().await?.json().await?;

    let mut matches = Vec::new();
    if let Some(results) = resp["results"].as_array() {
        for r in results {
            matches.push(ImageMatch {
                url:        r["url"].as_str().unwrap_or("").to_string(),
                source:     r["source"].as_str().map(|s| s.into()),
                title:      r["title"].as_str().map(|s| s.into()),
                similarity: r["similarity"].as_f64(),
                category:   r["category"].as_str().map(|s| s.into()),
            });
        }
    }

    Ok(LensoResult { query_url: image_url.to_string(), matches })
}
