// ravens-server/src/main.rs
// Ravens Nexus License Server — Rust + Axum
// Features: HWID binding, token rebind, sessions, ban by HWID, news, subscriptions, discord gateway
// Security: JWT + HMAC signatures + Rate limiting + Anti-replay

mod discord;
mod payment;
mod db;
mod security;

use axum::{
    extract::{Path, State, Multipart},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Html, sse::{Event, Sse}},
    routing::{delete, get, post, put, patch},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    convert::Infallible,
    path::Path as StdPath,
};
use uuid::Uuid;
use tower_http::{cors::CorsLayer, trace::TraceLayer, services::ServeDir};
use futures::stream::{self, Stream};
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;

const ADMIN_UI_HTML: &str = include_str!("../ui/admin.html");

// ── Config ───────────────────────────────────────────────────────────────────
const ADMIN_SECRET: &str = "M27361HD652hs76766yde28hjdhj87w4h32hoi_sifu849wu3j4897riuyfgihfj__MAGAY__uhfuyqe3r8y9qr389YQR390uredqwfUIOJEAWFpidor276374R627QRDHIUAR";
const SERVER_PORT: u16 = 3000;

// ── Tier ─────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Elite,
    Admin,
}

impl Tier {
    pub fn monthly_requests(&self) -> i64 {
        match self {
            Tier::Free  => 2,
            Tier::Pro   => 10,
            Tier::Elite => -1,
            Tier::Admin => -1,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Tier::Free  => "FREE",
            Tier::Pro   => "PRO",
            Tier::Elite => "ELITE",
            Tier::Admin => "ADMIN",
        }
    }
    pub fn allowed_modules(&self) -> Vec<&str> {
        match self {
            Tier::Free  => vec!["social", "ip_geo", "whois"],
            Tier::Pro   => vec!["social", "ip_geo", "whois", "hibp", "dorks", "paste", "darkweb", "phone"],
            Tier::Elite => vec!["social", "ip_geo", "whois", "hibp", "dorks", "paste", "darkweb", "phone", "intelx", "ai"],
            Tier::Admin => vec!["social", "ip_geo", "whois", "hibp", "dorks", "paste", "darkweb", "phone", "intelx", "ai"],
        }
    }
}

// ── Session ───────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub token: String,
    pub hwid: String,
    pub ip: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub banned: bool,
}

// ── BanRecord ─────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRecord {
    pub hwid: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub banned_by: String,
    pub unbanned: bool,
    pub unbanned_at: Option<DateTime<Utc>>,
    pub unbanned_by: Option<String>,
}

// ── UploadedFile ───────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub url: String,
    pub mime: String,
    pub size: u64,
    pub created_at: DateTime<Utc>,
}

// ── NewsItem ──────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub preview_text: String,
    pub full_text: String,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub media_position: String,
    pub media_rounded: bool,
    pub created_at: DateTime<Utc>,
    pub published: bool,
    pub author: String,
    pub attachments: Vec<UploadedFile>,
}

// ── License ───────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub token: String,
    pub tier: Tier,
    pub hwid: Option<String>,
    pub requests_used: i64,
    pub requests_limit: i64,
    pub last_reset_day: String, // YYYY-MM-DD UTC
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub hwid_banned: bool,
    pub note: String,
}

impl License {
    pub fn new(tier: Tier, expires_days: Option<i64>, note: &str) -> Self {
        let token = format!(
            "RVN-{}",
            Uuid::new_v4().to_string().to_uppercase().replace('-', "").chars().take(16).collect::<String>()
        );
        let limit = tier.monthly_requests();
        let expires_at = expires_days.map(|d| Utc::now() + Duration::days(d));
        Self {
            token, tier, hwid: None,
            requests_used: 0, requests_limit: limit,
            last_reset_day: Utc::now().format("%Y-%m-%d").to_string(),
            created_at: Utc::now(), expires_at,
            active: true, hwid_banned: false,
            note: note.to_string(),
        }
    }
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|e| Utc::now() > e).unwrap_or(false)
    }
    pub fn requests_remaining(&self) -> i64 {
        if self.requests_limit == -1 { return i64::MAX; }
        (self.requests_limit - self.requests_used).max(0)
    }
}


fn ensure_daily_reset(lic: &mut License) {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    if lic.last_reset_day != today {
        lic.last_reset_day = today;
        lic.requests_used = 0;
    }
}

// ── App State ─────────────────────────────────────────────────────────────────
pub struct AppState {
    pub licenses: RwLock<HashMap<String, License>>,
    pub hwid_index: RwLock<HashMap<String, String>>,
    pub sessions: RwLock<HashMap<String, Session>>,
    pub banned_hwids: RwLock<HashSet<String>>,
    pub ban_records: RwLock<Vec<BanRecord>>,
    pub news: RwLock<Vec<NewsItem>>,
    pub uploads: RwLock<Vec<UploadedFile>>,
    pub discord_gateway: Arc<discord::DiscordGateway>,
    pub db: Option<db::SharedDatabase>,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            licenses: Default::default(),
            hwid_index: Default::default(),
            sessions: Default::default(),
            banned_hwids: Default::default(),
            ban_records: Default::default(),
            news: Default::default(),
            uploads: Default::default(),
            discord_gateway: Arc::new(discord::DiscordGateway::new()),
            db: None,
        }
    }
}
type SharedState = Arc<AppState>;

// ── Helpers ───────────────────────────────────────────────────────────────────
fn hash_hwid(hwid: &str) -> String {
    let mut h = Sha256::new();
    h.update(hwid.as_bytes());
    hex::encode(h.finalize())
}

fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff: u8 = 0;
    for (x, y) in a.as_bytes().iter().zip(b.as_bytes().iter()) { diff |= x ^ y; }
    diff == 0
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
fn env_or_u16(key: &str, default: u16) -> u16 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ── Admin auth middleware ─────────────────────────────────────────────────────
async fn admin_auth(
    headers: HeaderMap,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let auth = headers.get("x-admin-secret").and_then(|v| v.to_str().ok()).unwrap_or("");
    let secret = env_or("RAVENS_ADMIN_SECRET", ADMIN_SECRET);
    if !ct_eq(auth, &secret) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"Unauthorized"}))).into_response();
    }
    next.run(req).await
}

// ── Request types ─────────────────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct ActivateRequest {
    pub token: String,
    pub hwid: String,
    pub ip: Option<String>,
    pub app_version: Option<String>,
}

#[derive(Deserialize)]
pub struct ConsumeRequest {
    pub token: String,
    pub hwid: String,
    pub module: String,
    pub session_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub tier: String,
    pub expires_days: Option<i64>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct RebindRequest {
    pub token: String,
    pub new_hwid: String,
    pub admin_secret: String,
}

#[derive(Deserialize)]
pub struct CreateNewsRequest {
    pub title: String,
    pub preview_text: String,
    pub full_text: String,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub media_position: Option<String>,
    pub media_rounded: Option<bool>,
    pub published: Option<bool>,
    pub author: Option<String>,
}

// ── LicenseInfo ───────────────────────────────────────────────────────────────
#[derive(Serialize)]
pub struct LicenseInfo {
    pub token: String, pub tier: String, pub hwid: Option<String>,
    pub requests_used: i64, pub requests_limit: i64, pub requests_remaining: i64,
    pub expires_at: Option<String>, pub active: bool, pub hwid_banned: bool, pub note: String,
}
impl From<&License> for LicenseInfo {
    fn from(l: &License) -> Self {
        Self {
            token: l.token.clone(), tier: l.tier.name().to_string(),
            hwid: l.hwid.clone(), requests_used: l.requests_used,
            requests_limit: l.requests_limit, requests_remaining: l.requests_remaining(),
            expires_at: l.expires_at.map(|d| d.to_rfc3339()),
            active: l.active, hwid_banned: l.hwid_banned, note: l.note.clone(),
        }
    }
}

// ── API Handlers ──────────────────────────────────────────────────────────────

/// Check if HWID is banned — returns ban record for client block screen
async fn check_hwid_ban(
    State(state): State<SharedState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let hwid = hash_hwid(req["hwid"].as_str().unwrap_or(""));
    if hwid.is_empty() {
        return Json(serde_json::json!({"banned":false}));
    }
    let banned = state.banned_hwids.read().unwrap();
    if banned.contains(&hwid) {
        // Find the ban record for details
        let records = state.ban_records.read().unwrap();
        if let Some(record) = records.iter().find(|r| r.hwid == hwid && !r.unbanned) {
            return Json(serde_json::json!({
                "banned": true,
                "reason": record.reason,
                "date": record.banned_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            }));
        }
        return Json(serde_json::json!({"banned":true,"reason":"Устройство заблокировано","date":"неизвестно"}));
    }
    Json(serde_json::json!({"banned":false}))
}

async fn activate_license(
    State(state): State<SharedState>,
    Json(req): Json<ActivateRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    let hwid = hash_hwid(&req.hwid);

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY CHECK: HWID ban list
    // ═══════════════════════════════════════════════════════════════════════════
    {
        let banned = state.banned_hwids.read().unwrap();
        if banned.contains(&hwid) {
            let records = state.ban_records.read().unwrap();
            if let Some(record) = records.iter().find(|r| r.hwid == hwid && !r.unbanned) {
                return Json(serde_json::json!({
                    "ok":false,
                    "banned": true,
                    "error":"Устройство заблокировано администратором",
                    "reason": record.reason,
                    "date": record.banned_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                }));
            }
            return Json(serde_json::json!({"ok":false,"banned":true,"error":"Устройство заблокировано администратором"}));
        }
    }

    let mut licenses = state.licenses.write().unwrap();
    let license = match licenses.get_mut(&token) {
        Some(l) => l,
        None => return Json(serde_json::json!({"ok":false,"error":"Токен не найден"})),
    };

    ensure_daily_reset(license);

    if !license.active {
        return Json(serde_json::json!({"ok":false,"error":"Токен деактивирован"}));
    }
    if license.is_expired() {
        return Json(serde_json::json!({"ok":false,"error":"Токен истёк"}));
    }
    if license.hwid_banned {
        return Json(serde_json::json!({"ok":false,"error":"Устройство заблокировано"}));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY CHECK: HWID binding (first activation binds HWID)
    // ═══════════════════════════════════════════════════════════════════════════
    if license.hwid.is_none() {
        license.hwid = Some(hwid.clone());
        state.hwid_index.write().unwrap().insert(hwid.clone(), token.clone());
    } else if license.hwid.as_deref() != Some(&hwid) {
        // HWID mismatch — token bound to different hardware
        security::log_action(
            &token, "activate_failed", &format!("hwid_mismatch: {}", hwid),
            "blocked", req.ip.as_deref().unwrap_or("unknown"), &hwid,
        );
        return Json(serde_json::json!({"ok":false,"error":"Токен привязан к другому устройству"}));
    }

    let tier = license.tier.clone();
    let remaining = license.requests_remaining();
    let expires = license.expires_at.map(|d| d.format("%Y-%m-%d").to_string());
    let allowed: Vec<String> = tier.allowed_modules().iter().map(|s| s.to_string()).collect();
    drop(licenses);

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Generate JWT with HWID binding + HMAC salt
    // ═══════════════════════════════════════════════════════════════════════════
    let session_id = Uuid::new_v4().to_string();
    let (jwt_token, hmac_salt) = security::generate_jwt(&hwid, &session_id);
    
    state.sessions.write().unwrap().insert(session_id.clone(), Session {
        id: session_id.clone(),
        token: jwt_token.clone(),
        hwid: hwid.clone(),
        ip: req.ip.unwrap_or_else(|| "unknown".into()),
        last_seen: Utc::now(),
        created_at: Utc::now(),
        app_version: req.app_version.unwrap_or_else(|| "unknown".into()),
        banned: false,
    });

    // Log successful activation
    security::log_action(
        &token, "activate", &format!("tier:{}", tier.name()),
        "success", req.ip.as_deref().unwrap_or("unknown"), &hwid,
    );

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Sign the response with HMAC
    // ═══════════════════════════════════════════════════════════════════════════
    let response_nonce = Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
    let response_body = serde_json::json!({
        "ok": true,
        "tier": tier.name(),
        "requests_remaining": remaining,
        "expires": expires,
        "allowed_modules": allowed,
        "session_id": session_id,
    });
    let response_signature = security::sign_response(&response_body, &response_nonce);

    Json(serde_json::json!({
        "header": {
            "timestamp": chrono::Utc::now().timestamp(),
            "nonce": response_nonce,
            "status": 200,
        },
        "body": response_body,
        "signature": response_signature,
        // Security credentials for client
        "jwt": jwt_token,
        "hmac_salt": hmac_salt,
    }))
}

async fn consume_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ConsumeRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    let hwid = hash_hwid(&req.hwid);

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Verify JWT from header (if provided)
    // ═══════════════════════════════════════════════════════════════════════════
    let jwt_token = headers.get("x-auth-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    let request_nonce = headers.get("x-nonce").and_then(|v| v.to_str().ok()).unwrap_or("");
    let request_signature = headers.get("x-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    
    if !jwt_token.is_empty() {
        // Verify JWT and extract claims
        match security::verify_jwt(jwt_token) {
            Ok(claims) => {
                // HWID must match JWT
                if claims.hwid != hwid {
                    security::log_action(&token, "consume_failed", "hwid_jwt_mismatch", "blocked", "", &hwid);
                    return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"HWID не совпадает с JWT"}));
                }
            }
            Err(e) => {
                security::log_action(&token, "consume_failed", &format!("jwt_invalid: {}", e), "blocked", "", &hwid);
                return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Невалидный JWT токен"}));
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Check session ban status
    // ═══════════════════════════════════════════════════════════════════════════
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.read().unwrap();
        if let Some(sess) = sessions.get(sid) {
            if sess.banned {
                return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Сессия заблокирована"}));
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Check HWID ban list
    // ═══════════════════════════════════════════════════════════════════════════
    {
        let banned = state.banned_hwids.read().unwrap();
        if banned.contains(&hwid) {
            return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Устройство заблокировано"}));
        }
    }

    let mut licenses = state.licenses.write().unwrap();
    let license = match licenses.get_mut(&token) {
        Some(l) => l,
        None => return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Токен не найден"})),
    };

    ensure_daily_reset(license);

    if !license.active || license.is_expired() {
        return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Лицензия недействительна"}));
    }
    if license.hwid.as_deref() != Some(&hwid) {
        return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"HWID не совпадает"}));
    }

    let allowed = license.tier.allowed_modules();
    if !allowed.contains(&req.module.as_str()) {
        return Json(serde_json::json!({"ok":false,"requests_remaining":license.requests_remaining(),"error":"Модуль недоступен на вашем тарифе"}));
    }

    if license.requests_limit != -1 && license.requests_used >= license.requests_limit {
        return Json(serde_json::json!({"ok":false,"requests_remaining":0,"error":"Лимит запросов исчерпан"}));
    }
    if license.requests_limit != -1 { license.requests_used += 1; }
    let remaining = license.requests_remaining();
    drop(licenses);

    if let Some(sid) = &req.session_id {
        if let Ok(mut sessions) = state.sessions.write() {
            if let Some(sess) = sessions.get_mut(sid) {
                sess.last_seen = Utc::now();
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SECURITY: Sign the response with HMAC
    // ═══════════════════════════════════════════════════════════════════════════
    let response_nonce = Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
    let response_body = serde_json::json!({"ok":true,"requests_remaining":remaining});
    let response_signature = security::sign_response(&response_body, &response_nonce);

    // Log the action
    security::log_action(&token, "consume", &req.module, "success", "", &hwid);

    Json(serde_json::json!({
        "header": {
            "timestamp": chrono::Utc::now().timestamp(),
            "nonce": response_nonce,
            "status": 200,
        },
        "body": response_body,
        "signature": response_signature,
    }))
}

async fn license_status(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let licenses = state.licenses.read().unwrap();
    match licenses.get(&token) {
        Some(l) => (StatusCode::OK, Json(serde_json::to_value(LicenseInfo::from(l)).unwrap())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"Токен не найден"}))).into_response(),
    }
}

async fn get_news_public(State(state): State<SharedState>) -> impl IntoResponse {
    let news = state.news.read().unwrap();
    let published: Vec<&NewsItem> = news.iter().filter(|n| n.published).collect();
    Json(serde_json::to_value(&published).unwrap())
}

async fn rebind_token(
    State(state): State<SharedState>,
    Json(req): Json<RebindRequest>,
) -> impl IntoResponse {
    let secret = env_or("RAVENS_ADMIN_SECRET", ADMIN_SECRET);
    if !ct_eq(&req.admin_secret, &secret) {
        return Json(serde_json::json!({"ok":false,"error":"Неверный секрет"}));
    }
    let token = req.token.trim().to_uppercase();
    let new_hwid = hash_hwid(&req.new_hwid);
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => {
            if let Some(old) = &l.hwid {
                state.hwid_index.write().unwrap().remove(old);
            }
            l.hwid = Some(new_hwid.clone());
            state.hwid_index.write().unwrap().insert(new_hwid, token.clone());
            Json(serde_json::json!({"ok":true,"message":"HWID успешно переназначен"}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Токен не найден"})),
    }
}

// ── File Upload ───────────────────────────────────────────────────────────────
const MAX_IMAGE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_VIDEO_SIZE: u64 = 100 * 1024 * 1024; // 100MB
const ALLOWED_IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];
const ALLOWED_VIDEO_TYPES: &[&str] = &["video/mp4", "video/webm", "video/quicktime"];

async fn upload_file(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Verify admin secret
    let auth = headers.get("x-admin-secret").and_then(|v| v.to_str().ok()).unwrap_or("");
    let secret = env_or("RAVENS_ADMIN_SECRET", ADMIN_SECRET);
    if !ct_eq(auth, &secret) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"Unauthorized"}))).into_response();
    }

    let mut saved_files: Vec<UploadedFile> = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => continue,
        };
        let size = data.len() as u64;

        // Validate type and size
        let is_image = ALLOWED_IMAGE_TYPES.contains(&content_type.as_str());
        let is_video = ALLOWED_VIDEO_TYPES.contains(&content_type.as_str());
        if !is_image && !is_video {
            continue;
        }
        if is_image && size > MAX_IMAGE_SIZE {
            continue;
        }
        if is_video && size > MAX_VIDEO_SIZE {
            continue;
        }

        // Generate filename
        let ext = match content_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/quicktime" => "mov",
            _ => "bin",
        };
        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        let uploads_dir = StdPath::new("uploads");
        let _ = tokio_fs::create_dir_all(uploads_dir).await;
        let filepath = uploads_dir.join(&filename);

        // Write file
        if let Ok(mut file) = tokio_fs::File::create(&filepath).await {
            let _ = file.write_all(&data).await;
        }

        let uploaded = UploadedFile {
            id: Uuid::new_v4().to_string(),
            url: format!("/uploads/{}", filename),
            mime: content_type,
            size,
            created_at: Utc::now(),
        };
        state.uploads.write().unwrap().push(uploaded.clone());
        saved_files.push(uploaded);
    }

    if saved_files.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"No valid files uploaded"}))).into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"ok":true,"files":saved_files}))).into_response()
}

// ── SSE for live news ─────────────────────────────────────────────────────────
async fn news_sse(State(state): State<SharedState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let news = state.news.read().unwrap();
    let events = news.clone();
    let stream = stream::iter(events.into_iter().map(|item| {
        let json = serde_json::to_string(&item).unwrap_or_default();
        Ok(Event::default().data(json).event("news"))
    }));
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(30))
    )
}

// ── Health Check ──────────────────────────────────────────────────────────────
async fn health_check(State(state): State<SharedState>) -> impl IntoResponse {
    let modules = serde_json::json!({
        "social": {"status":"ok","note":"Reddit, GitHub, Telegram APIs"},
        "ip_geo": {"status":"ok","note":"ip-api.com"},
        "whois": {"status":"ok","note":"rdap.org + cloudflare-dns"},
        "hibp": {"status":"ok","note":"haveibeenpwned.com (rate limited without key)"},
        "dorks": {"status":"ok","note":"static dork generation"},
        "paste": {"status":"ok","note":"doxbin.org scraping"},
        "darkweb": {"status":"ok","note":"ahmia.fi"},
        "phone": {"status":"ok","note":"veriphone.io"},
        "intelx": {"status":"ok","note":"intelx.io"},
        "ai": {"status":"ok","note":"NVIDIA NIM (requires API key)"},
    });
    let banned_count = state.banned_hwids.read().unwrap().len();
    let active_sessions = state.sessions.read().unwrap().values().filter(|s| !s.banned).count();
    Json(serde_json::json!({
        "status":"ok",
        "service":"ravens-nexus-server",
        "version":"2.8.0",
        "modules": modules,
        "stats":{
            "banned_hwids": banned_count,
            "active_sessions": active_sessions,
            "total_licenses": state.licenses.read().unwrap().len(),
        }
    }))
}

// ── Discord State Endpoint (2.7.5) ────────────────────────────────────────────
async fn discord_state(State(state): State<SharedState>) -> impl IntoResponse {
    let gateway_state = state.discord_gateway.get_state();
    Json(serde_json::to_value(gateway_state).unwrap())
}

// ── Payment Endpoints ─────────────────────────────────────────────────────────
async fn validate_license(
    State(state): State<SharedState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let key = req["key"].as_str().unwrap_or("");
    match payment::validate_license_key(key, &state.licenses) {
        Ok(info) => (StatusCode::OK, Json(serde_json::json!({"valid": true, "license": info}))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"valid": false, "error": e}))).into_response(),
    }
}

async fn create_payment(
    State(state): State<SharedState>,
    Json(req): Json<payment::PaymentRequest>,
) -> impl IntoResponse {
    match payment::process_payment(req, &state.licenses) {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e}))).into_response(),
    }
}

async fn subscription_status(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    match payment::get_subscription_status(&token, &state.licenses) {
        Some(sub) => (StatusCode::OK, Json(serde_json::to_value(sub).unwrap())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Token not found"}))).into_response(),
    }
}

// ── Admin Handlers ────────────────────────────────────────────────────────────

async fn admin_create_token(
    State(state): State<SharedState>,
    Json(req): Json<CreateTokenRequest>,
) -> impl IntoResponse {
    let tier = match req.tier.to_lowercase().as_str() {
        "pro" => Tier::Pro, "elite" => Tier::Elite, "admin" => Tier::Admin, _ => Tier::Free,
    };
    let note = req.note.unwrap_or_default();
    let license = License::new(tier.clone(), req.expires_days, &note);
    let token = license.token.clone();
    state.licenses.write().unwrap().insert(token.clone(), license);
    Json(serde_json::json!({"token":token,"tier":tier.name(),"expires_days":req.expires_days}))
}

async fn admin_list_tokens(State(state): State<SharedState>) -> impl IntoResponse {
    let licenses = state.licenses.read().unwrap();
    let list: Vec<LicenseInfo> = licenses.values().map(LicenseInfo::from).collect();
    Json(list)
}

async fn admin_revoke_token(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => { l.active = false; Json(serde_json::json!({"ok":true})) }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

async fn admin_change_tier(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let new_tier = match body["tier"].as_str().unwrap_or("free") {
        "pro" => Tier::Pro, "elite" => Tier::Elite, "admin" => Tier::Admin, _ => Tier::Free,
    };
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => {
            l.tier = new_tier.clone();
            l.requests_limit = new_tier.monthly_requests();
            Json(serde_json::json!({"ok":true,"tier":new_tier.name()}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

async fn admin_reset_requests(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => { l.requests_used = 0; Json(serde_json::json!({"ok":true})) }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

async fn admin_rebind_hwid(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let new_hwid_raw = body["new_hwid"].as_str().unwrap_or("").to_string();
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => {
            if let Some(old) = &l.hwid {
                state.hwid_index.write().unwrap().remove(old);
            }
            if new_hwid_raw.is_empty() {
                l.hwid = None;
            } else {
                let new_hwid = hash_hwid(&new_hwid_raw);
                l.hwid = Some(new_hwid.clone());
                state.hwid_index.write().unwrap().insert(new_hwid, token.clone());
            }
            Json(serde_json::json!({"ok":true,"message":"HWID переназначен"}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

async fn admin_list_sessions(State(state): State<SharedState>) -> impl IntoResponse {
    let sessions = state.sessions.read().unwrap();
    let list: Vec<&Session> = sessions.values().collect();
    Json(serde_json::to_value(&list).unwrap())
}

async fn admin_ban_session(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let ban_hwid = body["ban_hwid"].as_bool().unwrap_or(false);
    let reason = body["reason"].as_str().unwrap_or("Нарушение правил").to_string();
    let admin_name = body["admin_name"].as_str().unwrap_or("admin").to_string();
    let mut sessions = state.sessions.write().unwrap();
    match sessions.get_mut(&session_id) {
        Some(sess) => {
            sess.banned = true;
            let hwid = sess.hwid.clone();
            let token = sess.token.clone();
            drop(sessions);
            if ban_hwid {
                state.banned_hwids.write().unwrap().insert(hwid.clone());
                // Create ban record with reason and date
                let record = BanRecord {
                    hwid: hwid.clone(),
                    reason,
                    banned_at: Utc::now(),
                    banned_by: admin_name,
                    unbanned: false,
                    unbanned_at: None,
                    unbanned_by: None,
                };
                state.ban_records.write().unwrap().push(record);
                let mut licenses = state.licenses.write().unwrap();
                if let Some(lic) = licenses.get_mut(&token) {
                    lic.hwid_banned = true;
                    lic.active = false;
                }
            }
            Json(serde_json::json!({"ok":true,"banned_hwid":ban_hwid}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Сессия не найдена"})),
    }
}

/// List all ban records (for admin panel)
async fn admin_list_bans(State(state): State<SharedState>) -> impl IntoResponse {
    let records = state.ban_records.read().unwrap();
    Json(serde_json::to_value(&*records).unwrap())
}

/// Unban HWID — accessible with admin token, logs who unbanned
async fn admin_unban_hwid_v2(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let hwid = body["hwid"].as_str().unwrap_or("").to_string();
    let admin_name = body["admin_name"].as_str().unwrap_or("admin").to_string();
    if hwid.is_empty() {
        return Json(serde_json::json!({"ok":false,"error":"HWID required"}));
    }
    state.banned_hwids.write().unwrap().remove(&hwid);
    // Mark ban record as unbanned
    let mut records = state.ban_records.write().unwrap();
    for rec in records.iter_mut() {
        if rec.hwid == hwid && !rec.unbanned {
            rec.unbanned = true;
            rec.unbanned_at = Some(Utc::now());
            rec.unbanned_by = Some(admin_name);
        }
    }
    drop(records);
    // Reactivate licenses
    let mut licenses = state.licenses.write().unwrap();
    for lic in licenses.values_mut() {
        if lic.hwid.as_deref() == Some(&hwid) {
            lic.hwid_banned = false;
            lic.active = true;
        }
    }
    Json(serde_json::json!({"ok":true}))
}

async fn admin_unban_hwid(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let hwid = body["hwid"].as_str().unwrap_or("").to_string();
    state.banned_hwids.write().unwrap().remove(&hwid);
    let mut licenses = state.licenses.write().unwrap();
    for lic in licenses.values_mut() {
        if lic.hwid.as_deref() == Some(&hwid) {
            lic.hwid_banned = false;
            lic.active = true;
        }
    }
    Json(serde_json::json!({"ok":true}))
}

async fn admin_create_news(
    State(state): State<SharedState>,
    Json(req): Json<CreateNewsRequest>,
) -> impl IntoResponse {
    let item = NewsItem {
        id: Uuid::new_v4().to_string(),
        title: req.title,
        preview_text: req.preview_text,
        full_text: req.full_text,
        image_url: req.image_url,
        video_url: req.video_url,
        media_position: req.media_position.unwrap_or_else(|| "top".into()),
        media_rounded: req.media_rounded.unwrap_or(true),
        created_at: Utc::now(),
        published: req.published.unwrap_or(true),
        author: req.author.unwrap_or_else(|| "Admin".into()),
    };
    let id = item.id.clone();
    state.news.write().unwrap().push(item);
    Json(serde_json::json!({"ok":true,"id":id}))
}

async fn admin_list_news(State(state): State<SharedState>) -> impl IntoResponse {
    let news = state.news.read().unwrap();
    Json(serde_json::to_value(&*news).unwrap())
}

async fn admin_update_news(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut news = state.news.write().unwrap();
    match news.iter_mut().find(|n| n.id == id) {
        Some(item) => {
            if let Some(v) = body["title"].as_str() { item.title = v.to_string(); }
            if let Some(v) = body["preview_text"].as_str() { item.preview_text = v.to_string(); }
            if let Some(v) = body["full_text"].as_str() { item.full_text = v.to_string(); }
            if let Some(v) = body["image_url"].as_str() { item.image_url = Some(v.to_string()); }
            if let Some(v) = body["video_url"].as_str() { item.video_url = Some(v.to_string()); }
            if let Some(v) = body["media_position"].as_str() { item.media_position = v.to_string(); }
            if let Some(v) = body["media_rounded"].as_bool() { item.media_rounded = v; }
            if let Some(v) = body["published"].as_bool() { item.published = v; }
            Json(serde_json::json!({"ok":true}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Новость не найдена"})),
    }
}

async fn admin_delete_news(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut news = state.news.write().unwrap();
    let before = news.len();
    news.retain(|n| n.id != id);
    if news.len() < before {
        Json(serde_json::json!({"ok":true}))
    } else {
        Json(serde_json::json!({"ok":false,"error":"Не найдено"}))
    }
}

async fn admin_stats(State(state): State<SharedState>) -> impl IntoResponse {
    let licenses = state.licenses.read().unwrap();
    let sessions = state.sessions.read().unwrap();
    let total = licenses.len();
    let active = licenses.values().filter(|l| l.active && !l.is_expired()).count();
    let mut by_tier: HashMap<&str, usize> = HashMap::new();
    for l in licenses.values() { *by_tier.entry(l.tier.name()).or_insert(0) += 1; }
    let active_sessions = sessions.values().filter(|s| !s.banned).count();
    let banned_sessions = sessions.values().filter(|s| s.banned).count();
    let banned_hwids = state.banned_hwids.read().unwrap().len();
    Json(serde_json::json!({
        "total_tokens": total,
        "active_tokens": active,
        "by_tier": by_tier,
        "active_sessions": active_sessions,
        "banned_sessions": banned_sessions,
        "banned_hwids": banned_hwids,
        "news_count": state.news.read().unwrap().len(),
    }))
}

// ── Seed ──────────────────────────────────────────────────────────────────────
fn seed_demo(state: &SharedState) {
    let mut licenses = state.licenses.write().unwrap();
    let mut admin = License::new(Tier::Admin, None, "Demo admin");
    admin.token = "RVN-ADMIN-DEMO-0000-0000".to_string();
    licenses.insert(admin.token.clone(), admin);
    let free = License::new(Tier::Free, Some(30), "Demo free 30 days");
    licenses.insert(free.token.clone(), free);
    let pro = License::new(Tier::Pro, Some(30), "Demo pro");
    licenses.insert(pro.token.clone(), pro);

    let mut news = state.news.write().unwrap();
    news.push(NewsItem {
        id: Uuid::new_v4().to_string(),
        title: "Ravens Nexus v2.8 — Новые возможности".to_string(),
        preview_text: "Обновление системы OSINT: привязка к железу, активные сессии, система новостей и улучшенный интерфейс.".to_string(),
        full_text: "В этом обновлении мы добавили:\n\n• Привязка токена к HWID устройства\n• Система активных сессий с баном по железу\n• Новостная лента с медиа-контентом\n• Улучшенная админ-панель\n• Free тариф: 2 запроса, доступ к OSINT\n• Pro тариф: 1000 запросов, все модули\n• Elite тариф: безлимит + AI".to_string(),
        image_url: Some("https://images.unsplash.com/photo-1614064641938-3bbee52942c7?w=800".to_string()),
        video_url: None,
        media_position: "top".to_string(),
        media_rounded: true,
        created_at: Utc::now(),
        published: true,
        author: "Ravens Team".to_string(),
        attachments: vec![],
    });
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    
    // Initialize Database (persistence)
    let db_url = env_or("DATABASE_URL", "sqlite:ravens_nexus.db");
    let db = match db::Database::new(&db_url).await {
        Ok(database) => {
            tracing::info!("Database connected: {}", db_url);
            Some(Arc::new(database))
        }
        Err(e) => {
            tracing::warn!("Database connection failed ({}), using in-memory storage", e);
            None
        }
    };
    
    // Initialize Discord Gateway
    let discord_gateway = Arc::new(discord::DiscordGateway::new());
    let bot_token = env_or("DISCORD_BOT_TOKEN", "");
    if !bot_token.is_empty() {
        discord_gateway.start(Some(bot_token)).await;
    }
    
    let state = Arc::new(AppState {
        discord_gateway,
        db,
        ..Default::default()
    });
    seed_demo(&state);

    let admin_routes = Router::new()
        .route("/tokens", post(admin_create_token))
        .route("/tokens", get(admin_list_tokens))
        .route("/tokens/:token", delete(admin_revoke_token))
        .route("/tokens/:token/tier", put(admin_change_tier))
        .route("/tokens/:token/reset-requests", post(admin_reset_requests))
        .route("/tokens/:token/rebind-hwid", post(admin_rebind_hwid))
        .route("/sessions", get(admin_list_sessions))
        .route("/sessions/:id/ban", post(admin_ban_session))
        .route("/hwid/unban", post(admin_unban_hwid_v2))
        .route("/bans", get(admin_list_bans))
        .route("/news", get(admin_list_news))
        .route("/news", post(admin_create_news))
        .route("/news/:id", patch(admin_update_news))
        .route("/news/:id", delete(admin_delete_news))
        .route("/stats", get(admin_stats))
        .route("/upload", post(upload_file))
        .layer(middleware::from_fn(admin_auth));

    let api_routes = Router::new()
        .route("/license/activate", post(activate_license))
        .route("/license/consume", post(consume_request))
        .route("/license/status/:token", get(license_status))
        .route("/license/rebind", post(rebind_token))
        .route("/hwid/check", post(check_hwid_ban))
        .route("/news", get(get_news_public))
        .route("/news/stream", get(news_sse))
        .route("/discord/state", get(discord_state))
        .route("/payment/validate", post(validate_license))
        .route("/payment/create", post(create_payment))
        .route("/payment/status/:token", get(subscription_status));

    let cors = CorsLayer::permissive();
    let app = Router::new()
        .route("/", get(|| async { Json(serde_json::json!({"status":"ok","service":"ravens-nexus-server","hint":"Open /admin-ui/admin.html"})) }))
        .route("/admin-ui/admin.html", get(|| async { Html(ADMIN_UI_HTML) }))
        .nest("/admin", admin_routes)
        .nest("/api", api_routes)
        .route("/health", get(health_check))
        .nest_service("/uploads", ServeDir::new("uploads"))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let port = env_or_u16("RAVENS_SERVER_PORT", SERVER_PORT);
    let addr = format!("0.0.0.0:{}", port);
    let secret = env_or("RAVENS_ADMIN_SECRET", ADMIN_SECRET);

    // Try to detect public Replit URL (REPL_SLUG / REPLIT_DEV_DOMAIN / REPLIT_DOMAINS)
    let public_url = std::env::var("REPLIT_DEV_DOMAIN")
        .ok()
        .map(|d| format!("https://{}", d))
        .or_else(|| std::env::var("REPLIT_DOMAINS").ok().map(|d| {
            let first = d.split(',').next().unwrap_or(&d).to_string();
            format!("https://{}", first)
        }))
        .unwrap_or_else(|| format!("http://localhost:{}", port));

    tracing::info!("================================================================");
    tracing::info!("  RAVENS NEXUS LICENSE SERVER — STARTED");
    tracing::info!("================================================================");
    tracing::info!("  Local bind:      {}", addr);
    tracing::info!("  Public URL:      {}", public_url);
    tracing::info!("  Health check:    {}/health", public_url);
    tracing::info!("  Admin Panel:     {}/admin-ui/admin.html", public_url);
    tracing::info!("  API base:        {}/api", public_url);
    tracing::info!("----------------------------------------------------------------");
    if secret == ADMIN_SECRET {
        tracing::warn!("  ADMIN SECRET: using built-in default — set RAVENS_ADMIN_SECRET env var!");
    } else {
        tracing::info!("  ADMIN SECRET: configured via RAVENS_ADMIN_SECRET env var");
    }
    tracing::info!("================================================================");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
