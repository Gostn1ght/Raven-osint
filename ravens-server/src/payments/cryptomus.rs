//! Cryptomus crypto payment invoicing.
//! Docs: https://doc.cryptomus.com/payments/creating-invoice

use reqwest::Client;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

pub async fn create_invoice(
    merchant: &str,
    api_key: &str,
    order_id: &str,
    amount: f64,
) -> anyhow::Result<String> {
    let payload = serde_json::json!({
        "amount":    format!("{:.2}", amount),
        "currency":  "USD",
        "order_id":  order_id,
        "url_callback": "https://your-server/api/pay/cryptomus/callback",
        "url_return": "https://your-server/dashboard",
    });

    let sign = sign_request(api_key, &payload.to_string());
    let client = Client::new();
    let resp: serde_json::Value = client
        .post("https://api.cryptomus.com/v1/payment")
        .header("merchant",  merchant)
        .header("sign",      sign)
        .json(&payload)
        .send().await?
        .json().await?;

    resp["result"]["url"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Cryptomus: no url in response: {}", resp))
}

fn sign_request(api_key: &str, body_json: &str) -> String {
    let encoded = B64.encode(body_json.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(format!("{}{}", encoded, api_key).as_bytes());
    hex::encode(hasher.finalize())
}
