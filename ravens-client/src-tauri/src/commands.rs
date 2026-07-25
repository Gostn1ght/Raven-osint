// ═══════════════════════════════════════════════════════════════════════════════
// TAURI COMMANDS — thin secure client.
// ═══════════════════════════════════════════════════════════════════════════════
// The client performs NO reconnaissance itself. It authenticates a token, then asks
// the server to run each OSINT module and simply renders what comes back.

use crate::config;
use crate::hwid;
use std::sync::Arc;
use tauri::State;

// ── Shared state ────────────────────────────────────────────────────────────────
pub struct AppState {
    pub api_client: Arc<tokio::sync::Mutex<hwid::SecureApiClient>>,
    pub hwid: String,
}

impl AppState {
    pub fn new(server_url: String) -> Self {
        Self {
            api_client: Arc::new(tokio::sync::Mutex::new(hwid::SecureApiClient::new(server_url))),
            hwid: hwid::get_hwid(),
        }
    }
}

// ── HWID ────────────────────────────────────────────────────────────────────────
#[tauri::command]
pub fn get_hwid() -> String {
    hwid::get_hwid()
}

// ── Server URL ──────────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn set_server_url(server_url: String, state: State<'_, AppState>) -> Result<(), String> {
    state.api_client.lock().await.set_server_url(&server_url);
    Ok(())
}

// ── Auth ────────────────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn authenticate(
    token: String,
    server_url: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut client = state.api_client.lock().await;
    client.authenticate(&token, &state.hwid, &server_url).await
}

// ── OSINT (executed on the server) ───────────────────────────────────────────────
#[tauri::command]
pub async fn osint_run(
    token: String,
    module: String,
    target: String,
    session_id: Option<String>,
    findings: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = state.api_client.lock().await;
    let findings = findings.unwrap_or_default();
    client
        .run_osint(&token, &state.hwid, &module, &target, session_id.as_deref(), &findings)
        .await
}

// ── Discord OSINT (server-proxied) ───────────────────────────────────────────────
#[tauri::command]
pub async fn discord_osint_run(
    token: String,
    session_id: Option<String>,
    endpoint: String,      // "guild-info" | "user-info" | "invite-info"
    bot_token: String,
    target: String,        // guild_id | user_id | invite_code
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = state.api_client.lock().await;
    client.run_discord_osint(&token, &state.hwid, &endpoint, &bot_token, &target, session_id.as_deref()).await
}

// ── Telegram OSINT (server-proxied) ──────────────────────────────────────────────
#[tauri::command]
pub async fn telegram_osint_run(
    token: String,
    session_id: Option<String>,
    bot_token: String,
    target: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = state.api_client.lock().await;
    client.run_telegram_osint(&token, &state.hwid, &bot_token, &target, session_id.as_deref()).await
}

// ── Config (encrypted with an HWID-derived key) ─────────────────────────────────
#[tauri::command]
pub fn save_config(key: &str, value: &str) -> Result<(), String> {
    let h = hwid::get_hwid();
    let encrypted = config::encrypt(value, &h).map_err(|e| e.to_string())?;
    let path = config_path(key);
    std::fs::write(&path, encrypted).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_config(key: &str) -> Result<String, String> {
    let h = hwid::get_hwid();
    let path = config_path(key);
    let encrypted = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    config::decrypt(&encrypted, &h).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_config(key: &str) -> Result<(), String> {
    let path = config_path(key);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn config_path(key: &str) -> std::path::PathBuf {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.push("ravens_config");
    std::fs::create_dir_all(&dir).ok();
    dir.push(format!("{}.enc", key));
    dir
}

// ── Window controls (custom title bar) ──────────────────────────────────────────
#[tauri::command]
pub fn minimize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn maximize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn close_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}
