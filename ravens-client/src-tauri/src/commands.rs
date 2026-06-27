//! Tauri commands — called from the Svelte frontend via invoke()

use reqwest::Client;
use serde_json::Value;
use tauri::State;
use std::sync::Mutex;

const API_BASE: &str = "http://127.0.0.1:8080/api";

pub struct TokenStore(pub Mutex<Option<String>>);

// ─── HWID ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_hwid() -> String {
    crate::hwid::generate()
}

// ─── Generic HTTP wrappers ─────────────────────────────────────────────────

fn client(token: Option<String>) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(t) = token {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", t).parse().unwrap(),
        );
    }
    Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap()
}

#[tauri::command]
pub async fn api_get(path: String, token: Option<String>) -> Result<Value, String> {
    let url = format!("{}{}", API_BASE, path);
    client(token)
        .get(&url)
        .send().await
        .map_err(|e| e.to_string())?
        .json::<Value>().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn api_post(path: String, body: Value, token: Option<String>) -> Result<Value, String> {
    let url = format!("{}{}", API_BASE, path);
    client(token)
        .post(&url)
        .json(&body)
        .send().await
        .map_err(|e| e.to_string())?
        .json::<Value>().await
        .map_err(|e| e.to_string())
}

// ─── Token management ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_token(token: String, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("credentials.bin").map_err(|e| e.to_string())?;
    store.set("jwt", serde_json::json!(token));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn load_token(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("credentials.bin").ok()?;
    store.get("jwt")?.as_str().map(|s| s.to_string())
}

#[tauri::command]
pub async fn clear_token(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("credentials.bin").map_err(|e| e.to_string())?;
    store.delete("jwt");
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Server health ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn server_health() -> bool {
    crate::server::is_healthy_pub().await
}
