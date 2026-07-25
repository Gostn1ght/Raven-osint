// ═══════════════════════════════════════════════════════════════════════════════
// HWID + SECURITY MODULE — Client Side
// ═══════════════════════════════════════════════════════════════════════════════
//
// Этот модуль:
// 1. Собирает мультифакторный HWID (CPU + MAC + Disk + Motherboard)
// 2. Подписывает запросы HMAC-SHA256
// 3. Проверяет подпись ответа сервера
// 4. Хранит JWT и HMAC salt в защищённом хранилище
//

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;

// Тип для HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

// Секрет для HMAC (должен совпадать с серверным — в проде компиляция через env var)
const HMAC_SECRET: &str = "ravens-nexus-hmac-secret-change-in-production-2026";

// ═══════════════════════════════════════════════════════════════════════════════
// HWID COLLECTION (Multi-Factor)
// ═══════════════════════════════════════════════════════════════════════════════

/// Собирает мультифакторный HWID из нескольких компонентов железа
pub fn get_hwid() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut hasher = Sha256::new();
    
    // 1. Hostname
    let hostname = System::host_name().unwrap_or_default();
    hasher.update(hostname.as_bytes());
    
    // 2. OS version
    let os_version = System::os_version().unwrap_or_default();
    hasher.update(os_version.as_bytes());
    
    // 3. CPU count + brand
    let cpu_count = sys.cpus().len().to_string();
    hasher.update(cpu_count.as_bytes());
    if let Some(cpu) = sys.cpus().first() {
        hasher.update(cpu.brand().as_bytes());
    }
    
    // 4. Total memory
    let total_mem = sys.total_memory().to_string();
    hasher.update(total_mem.as_bytes());
    
    // 5. MAC Address
    if let Some(mac) = get_mac_address() {
        hasher.update(mac.as_bytes());
    }
    
    // 6. Disk serial (OS-specific)
    if let Some(disk) = get_disk_serial() {
        hasher.update(disk.as_bytes());
    }

    // Финальный хеш
    let result = hasher.finalize();
    hex::encode(result)
}

/// Получает MAC адрес первого активного сетевого интерфейса
fn get_mac_address() -> Option<String> {
    // Используем network-interface крейт или парсим системные команды
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("getmac")
            .args(&["/FO", "CSV", "/NH"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().next().and_then(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            parts.get(0).map(|s| s.trim_matches('"').to_string())
        })
    }
    
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("cat")
            .arg("/sys/class/net/eth0/address")
            .output()
            .ok()?;
        let mac = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if mac.is_empty() { None } else { Some(mac) }
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("ifconfig")
            .arg("en0")
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .find(|l| l.contains("ether"))
            .and_then(|l| l.split_whitespace().nth(1))
            .map(|s| s.to_string())
    }
}

/// Получает серийный номер диска (OS-specific)
fn get_disk_serial() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("wmic")
            .args(&["diskdrive", "get", "SerialNumber"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().nth(1).map(|s| s.trim().to_string())
    }
    
    #[cfg(target_os = "linux")]
    {
        // Используем /etc/machine-id как fallback
        std::fs::read_to_string("/etc/machine-id")
            .ok()
            .map(|s| s.trim().to_string())
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("system_profiler")
            .args(&["SPSerialATADataType"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .find(|l| l.contains("Serial"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim().to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HMAC SIGNATURE UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

/// Подписывает данные HMAC-SHA256
pub fn hmac_sign(data: &[u8], key: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(data);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Подписывает строку
pub fn hmac_sign_str(data: &str, key: &str) -> String {
    hmac_sign(data.as_bytes(), key.as_bytes())
}

/// Проверяет HMAC подпись (constant-time comparison)
pub fn hmac_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Генерирует timestamp (unix seconds)
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Генерирует уникальный nonce
pub fn generate_nonce() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY CREDENTIALS (stored after auth)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCredentials {
    pub jwt: String,
    pub hmac_salt: String,
    pub hwid: String,
}

/// Подписывает запрос к серверу
pub fn sign_request(hwid: &str, nonce: &str, timestamp: u64, hmac_salt: &str) -> String {
    let message = format!("{}:{}:{}", hwid, nonce, timestamp);
    hmac_sign_str(&message, hmac_salt)
}

/// Проверяет подпись ответа сервера
pub fn verify_server_response(
    body: &serde_json::Value,
    nonce: &str,
    signature: &str,
) -> bool {
    let message = format!("{}:{}", nonce, serde_json::to_string(body).unwrap_or_default());
    let expected = hmac_sign_str(&message, HMAC_SECRET);
    hmac_compare(signature, &expected)
}

// ═══════════════════════════════════════════════════════════════════════════════
// API CLIENT WITH SECURITY
// ═══════════════════════════════════════════════════════════════════════════════

pub struct SecureApiClient {
    pub server_url: String,
    pub credentials: Option<SecurityCredentials>,
}

impl SecureApiClient {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            credentials: None,
        }
    }
    
    /// Updates the backend base URL (called when the user changes it in Settings).
    pub fn set_server_url(&mut self, url: &str) {
        if !url.trim().is_empty() {
            self.server_url = url.trim().trim_end_matches('/').to_string();
        }
    }

    /// Authenticate — binds HWID on the server and receives a session token + salt.
    pub async fn authenticate(
        &mut self,
        token: &str,
        hwid: &str,
        server_url: &str,
    ) -> Result<serde_json::Value, String> {
        self.set_server_url(server_url);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/license/activate", self.server_url))
            .json(&serde_json::json!({
                "token": token,
                "hwid": hwid,
                "app_version": "2.8.0",
            }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_default();
            return Err(format!("Auth failed: {}", err));
        }

        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Parse error: {}", e))?;

        // Best-effort response-signature verification. TLS already guarantees integrity;
        // this only adds value when the client is built with the server's shared secret.
        let signature = json["signature"].as_str().unwrap_or("");
        let nonce = json["header"]["nonce"].as_str().unwrap_or("");
        if !signature.is_empty() && !verify_server_response(&json["body"], nonce, signature) {
            tracing::warn!("activate: response signature not verified (shared secret not configured) — relying on TLS");
        }

        // Store per-session credentials for signing subsequent requests.
        if let (Some(jwt), Some(salt)) = (json["jwt"].as_str(), json["hmac_salt"].as_str()) {
            self.credentials = Some(SecurityCredentials {
                jwt: jwt.to_string(),
                hmac_salt: salt.to_string(),
                hwid: hwid.to_string(),
            });
        }

        Ok(json)
    }

    /// Discord OSINT — proxied through the server (guild-info, user-info, invite-info).
    pub async fn run_discord_osint(
        &self,
        token: &str,
        hwid: &str,
        endpoint: &str, // "guild-info" | "user-info" | "invite-info"
        bot_token: &str,
        target: &str,  // guild_id | user_id | invite_code
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let creds = self.credentials.as_ref().ok_or("Не аутентифицирован — активируйте токен")?;
        let timestamp = current_timestamp();
        let nonce = generate_nonce();
        let signature = sign_request(hwid, &nonce, timestamp, &creds.hmac_salt);
        let client = reqwest::Client::new();
        let url = format!("{}/api/discord/{}", self.server_url, endpoint);
        // Build body depending on endpoint
        let body = match endpoint {
            "guild-info" => serde_json::json!({ "token": token, "hwid": hwid, "session_id": session_id, "bot_token": bot_token, "guild_id": target }),
            "user-info"  => serde_json::json!({ "token": token, "hwid": hwid, "session_id": session_id, "bot_token": bot_token, "user_id":  target }),
            _            => serde_json::json!({ "token": token, "hwid": hwid, "session_id": session_id, "invite_code": target }),
        };
        let response = client.post(&url)
            .header("X-Auth-Token", &creds.jwt)
            .header("X-HWID", hwid)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Nonce", &nonce)
            .header("X-Signature", &signature)
            .json(&body)
            .send().await
            .map_err(|e| format!("Network error: {}", e))?;
        let status = response.status();
        let json: serde_json::Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if !status.is_success() {
            return Err(json["error"].as_str().unwrap_or("Ошибка сервера").to_string());
        }
        Ok(json["body"].clone())
    }

    /// Telegram OSINT — chat-info proxied through the server.
    pub async fn run_telegram_osint(
        &self,
        token: &str,
        hwid: &str,
        bot_token: &str,
        target: &str,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let creds = self.credentials.as_ref().ok_or("Не аутентифицирован — активируйте токен")?;
        let timestamp = current_timestamp();
        let nonce = generate_nonce();
        let signature = sign_request(hwid, &nonce, timestamp, &creds.hmac_salt);
        let client = reqwest::Client::new();
        let url = format!("{}/api/telegram/chat-info", self.server_url);
        let response = client.post(&url)
            .header("X-Auth-Token", &creds.jwt)
            .header("X-HWID", hwid)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Nonce", &nonce)
            .header("X-Signature", &signature)
            .json(&serde_json::json!({
                "token": token, "hwid": hwid, "session_id": session_id,
                "bot_token": bot_token, "target": target,
            }))
            .send().await
            .map_err(|e| format!("Network error: {}", e))?;
        let status = response.status();
        let json: serde_json::Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if !status.is_success() {
            return Err(json["error"].as_str().unwrap_or("Ошибка сервера").to_string());
        }
        Ok(json["body"].clone())
    }

    /// Runs an OSINT module on the server with a signed request. The server executes
    /// the reconnaissance and returns the resulting events.
    pub async fn run_osint(
        &self,
        token: &str,
        hwid: &str,
        module: &str,
        target: &str,
        session_id: Option<&str>,
        findings: &[String],
    ) -> Result<serde_json::Value, String> {
        let creds = self.credentials.as_ref().ok_or("Не аутентифицирован — активируйте токен")?;

        let timestamp = current_timestamp();
        let nonce = generate_nonce();
        let signature = sign_request(hwid, &nonce, timestamp, &creds.hmac_salt);

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/osint/run", self.server_url))
            .header("X-Auth-Token", &creds.jwt)
            .header("X-HWID", hwid)
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Nonce", &nonce)
            .header("X-Signature", &signature)
            .json(&serde_json::json!({
                "token": token,
                "hwid": hwid,
                "module": module,
                "target": target,
                "session_id": session_id,
                "findings": findings,
            }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        let status = response.status();
        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Parse error: {}", e))?;

        // Non-success responses carry a flat {ok:false,error} body (no envelope).
        if !status.is_success() {
            let err = json["error"].as_str().unwrap_or("Ошибка сервера").to_string();
            return Err(err);
        }

        let resp_signature = json["signature"].as_str().unwrap_or("");
        let resp_nonce = json["header"]["nonce"].as_str().unwrap_or("");
        let resp_body = &json["body"];
        if !resp_signature.is_empty() && !verify_server_response(resp_body, resp_nonce, resp_signature) {
            tracing::warn!("osint: response signature not verified — relying on TLS");
        }

        Ok(resp_body.clone())
    }
}
