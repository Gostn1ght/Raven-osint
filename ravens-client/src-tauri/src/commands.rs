// ═══════════════════════════════════════════════════════════════════════════════
// TAURI COMMANDS — Secure API Client Integration
// ═══════════════════════════════════════════════════════════════════════════════

use crate::hwid;
use crate::config;
use crate::osint;
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

// ═══════════════════════════════════════════════════════════════════════════════
// APP STATE (shared across commands)
// Используем tokio::sync::Mutex — его Guard является Send
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// HWID COMMAND
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_hwid() -> String {
    hwid::get_hwid()
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUTH COMMAND — Authenticate with server (JWT + HMAC salt)
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn authenticate(
    token: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut client = state.api_client.lock().await;
    let result = client.authenticate(&token, &state.hwid).await;
    // MutexGuard автоматически отпускается здесь (выходит из scope)
    result
}

// ═══════════════════════════════════════════════════════════════════════════════
// EXECUTE COMMAND — Send signed request to server
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn execute_action(
    token: String,
    module: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = state.api_client.lock().await;
    let result = client.execute(&module, &token, &state.hwid, session_id.as_deref(), &module).await;
    // MutexGuard автоматически отпускается здесь
    result
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG COMMANDS (encrypted with HWID-derived key)
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// OSINT SCAN (local execution — for modules that work offline)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub target: String,
    pub target_type: String,
    pub modules: Vec<String>,
    pub nvidia_api_key: Option<String>,
}

#[tauri::command]
pub async fn osint_scan(req: ScanRequest) -> Result<Vec<osint::OsintEvent>, String> {
    let mut all_events: Vec<osint::OsintEvent> = vec![];
    let target = req.target.trim();

    for module in &req.modules {
        let events = match module.as_str() {
            "social"  => osint::run_social_check(target).await,
            "hibp"    => osint::run_hibp(target).await,
            "ip"      => osint::run_ip(target).await,
            "whois"   => osint::run_whois(target).await,
            "dorks"   => osint::run_dorks(target),
            "paste"   => osint::run_paste(target).await,
            "darkweb" => osint::run_darkweb(target).await,
            "phone"   => osint::run_phone(target).await,
            "intelx"  => osint::run_intelx(target).await,
            _         => vec![],
        };
        all_events.extend(events);
    }

    // AI analysis if key provided
    if let Some(api_key) = &req.nvidia_api_key {
        if !api_key.is_empty() && req.modules.contains(&"ai".to_string()) {
            let findings: Vec<String> = all_events.iter()
                .filter(|e| e.kind == "found")
                .map(|e| e.text.clone())
                .collect();
            let ai_events = osint::run_ai(target, &findings, api_key).await;
            all_events.extend(ai_events);
        }
    }

    Ok(all_events)
}

// ═══════════════════════════════════════════════════════════════════════════════
// SINGLE MODULE EXECUTION
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ModuleRequest {
    pub module: String,
    pub target: String,
    pub nvidia_api_key: Option<String>,
    pub findings: Option<Vec<String>>,
}

#[tauri::command]
pub async fn osint_module(req: ModuleRequest) -> Result<Vec<osint::OsintEvent>, String> {
    let target = req.target.trim();
    let events = match req.module.as_str() {
        "social"  => osint::run_social_check(target).await,
        "hibp"    => osint::run_hibp(target).await,
        "ip"      => osint::run_ip(target).await,
        "whois"   => osint::run_whois(target).await,
        "dorks"   => osint::run_dorks(target),
        "paste"   => osint::run_paste(target).await,
        "darkweb" => osint::run_darkweb(target).await,
        "phone"   => osint::run_phone(target).await,
        "intelx"  => osint::run_intelx(target).await,
        "ai" => {
            let key = req.nvidia_api_key.as_deref().unwrap_or("");
            let empty: Vec<String> = vec![];
            let findings = req.findings.as_deref().unwrap_or(&empty);
            osint::run_ai(target, findings, key).await
        }
        _ => vec![osint::OsintEvent::new("error", "error", format!("Неизвестный модуль: {}", req.module))],
    };
    Ok(events)
}
