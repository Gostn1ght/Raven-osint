// ravens-server/src/main.rs
// Ravens Nexus License + OSINT Server — Rust + Axum
// The server is the single source of truth: it owns licensing (tokens/account keys,
// HWID binding, sessions, bans, tiers, quotas) AND runs the OSINT engine itself, so
// the desktop client is a thin front-end that only renders server-produced results.
//
// Security: HMAC-signed session tokens · signed responses · per-request signatures
//           · anti-replay (timestamp + one-time nonce) · per-HWID rate limiting.

mod discord;
mod payment;
mod osint;
mod security;
mod store;

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
    convert::Infallible,
    path::Path as StdPath,
    sync::{Arc, OnceLock, RwLock},
};
use uuid::Uuid;
use tower_http::{cors::CorsLayer, trace::TraceLayer, services::ServeDir};
use futures::stream::{self, Stream};
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;

const ADMIN_UI_HTML: &str = include_str!("../ui/admin.html");

// ── Config ───────────────────────────────────────────────────────────────────
const SERVER_PORT: u16 = 3000;
const STATE_FILE: &str = "data/ravens_state.json";

/// Admin secret — from `RAVENS_ADMIN_SECRET`, or a random per-process value that is
/// printed to the log at startup. There is intentionally NO hard-coded production key.
fn admin_secret() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| {
        match std::env::var("RAVENS_ADMIN_SECRET") {
            Ok(v) if v.len() >= 8 && v != "RAVENS_ADMIN_SECRET_CHANGE_ME" => v,
            _ => {
                use rand::RngCore;
                let mut b = [0u8; 24];
                rand::thread_rng().fill_bytes(&mut b);
                let gen = hex::encode(b);
                tracing::warn!("RAVENS_ADMIN_SECRET not set — generated a random admin secret for this run:");
                tracing::warn!("    RAVENS_ADMIN_SECRET = {}", gen);
                gen
            }
        }
    })
}

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
    pub fn daily_requests(&self) -> i64 {
        match self {
            Tier::Free => 2,
            Tier::Pro => 1000,
            Tier::Elite => -1,
            Tier::Admin => -1,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Tier::Free => "FREE",
            Tier::Pro => "PRO",
            Tier::Elite => "ELITE",
            Tier::Admin => "ADMIN",
        }
    }
    pub fn allowed_modules(&self) -> Vec<&str> {
        match self {
            Tier::Free => vec!["social", "ip_geo", "whois"],
            Tier::Pro => vec!["social", "ip_geo", "whois", "hibp", "dorks", "paste", "darkweb", "phone"],
            Tier::Elite | Tier::Admin => vec![
                "social", "ip_geo", "whois", "hibp", "dorks", "paste", "darkweb", "phone", "intelx", "ai",
            ],
        }
    }
    pub fn from_str(s: &str) -> Tier {
        match s.to_lowercase().as_str() {
            "pro" => Tier::Pro,
            "elite" => Tier::Elite,
            "admin" => Tier::Admin,
            _ => Tier::Free,
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
    #[serde(default)]
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
        let limit = tier.daily_requests();
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

// ── Persistence snapshot ───────────────────────────────────────────────────────
#[derive(Serialize, Deserialize, Default)]
struct Snapshot {
    #[serde(default)]
    licenses: Vec<License>,
    #[serde(default)]
    banned_hwids: Vec<String>,
    #[serde(default)]
    ban_records: Vec<BanRecord>,
    #[serde(default)]
    news: Vec<NewsItem>,
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
    pub store: store::Store,
    pub rate_limiter: security::RateLimiter,
    pub nonce_cache: security::NonceCache,
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
            store: store::Store::new(STATE_FILE),
            rate_limiter: security::RateLimiter::default(),
            nonce_cache: security::NonceCache::default(),
        }
    }
}

impl AppState {
    /// Builds a persistable snapshot (clones data out; holds no locks afterwards).
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            licenses: self.licenses.read().unwrap().values().cloned().collect(),
            banned_hwids: self.banned_hwids.read().unwrap().iter().cloned().collect(),
            ban_records: self.ban_records.read().unwrap().clone(),
            news: self.news.read().unwrap().clone(),
        }
    }
    /// Persists the current snapshot to disk. Callers must NOT hold any state lock.
    fn persist(&self) {
        self.store.save(&self.snapshot());
    }
    /// Loads a snapshot into state and rebuilds the HWID index.
    fn load_snapshot(&self, snap: Snapshot) {
        let mut idx = self.hwid_index.write().unwrap();
        let mut licenses = self.licenses.write().unwrap();
        for lic in snap.licenses {
            if let Some(h) = &lic.hwid {
                idx.insert(h.clone(), lic.token.clone());
            }
            licenses.insert(lic.token.clone(), lic);
        }
        *self.banned_hwids.write().unwrap() = snap.banned_hwids.into_iter().collect();
        *self.ban_records.write().unwrap() = snap.ban_records;
        *self.news.write().unwrap() = snap.news;
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
fn hdr<'a>(h: &'a HeaderMap, k: &str) -> &'a str {
    h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("")
}

/// Wraps a body in the signed response envelope the client verifies.
fn signed_envelope(body: serde_json::Value) -> Json<serde_json::Value> {
    let nonce = Uuid::new_v4().to_string().split('-').next().unwrap_or("").to_string();
    let signature = security::sign_response(&body, &nonce);
    Json(serde_json::json!({
        "header": { "timestamp": Utc::now().timestamp(), "nonce": nonce, "status": 200 },
        "body": body,
        "signature": signature,
    }))
}

// ── Admin auth middleware ─────────────────────────────────────────────────────
async fn admin_auth(
    headers: HeaderMap,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let auth = hdr(&headers, "x-admin-secret");
    if !ct_eq(auth, admin_secret()) {
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
pub struct OsintRunRequest {
    pub token: String,
    pub hwid: String,
    pub module: String,
    pub target: String,
    pub session_id: Option<String>,
    pub findings: Option<Vec<String>>,
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
    #[serde(default)]
    pub attachments: Option<Vec<UploadedFile>>,
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
    let raw = req["hwid"].as_str().unwrap_or("");
    if raw.is_empty() {
        return Json(serde_json::json!({"banned":false}));
    }
    let hwid = hash_hwid(raw);
    let banned = state.banned_hwids.read().unwrap();
    if banned.contains(&hwid) {
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
    let raw_hwid = req.hwid.trim().to_string();
    if raw_hwid.is_empty() {
        return Json(serde_json::json!({"ok":false,"error":"HWID обязателен"}));
    }
    let hwid = hash_hwid(&raw_hwid);

    // SECURITY: HWID ban list
    {
        let banned = state.banned_hwids.read().unwrap();
        if banned.contains(&hwid) {
            let records = state.ban_records.read().unwrap();
            if let Some(record) = records.iter().find(|r| r.hwid == hwid && !r.unbanned) {
                return Json(serde_json::json!({
                    "ok":false, "banned": true,
                    "error":"Устройство заблокировано администратором",
                    "reason": record.reason,
                    "date": record.banned_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                }));
            }
            return Json(serde_json::json!({"ok":false,"banned":true,"error":"Устройство заблокировано администратором"}));
        }
    }

    let (tier, remaining, expires, allowed) = {
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
        // SECURITY: HWID binding (first activation binds HWID)
        if license.hwid.is_none() {
            license.hwid = Some(hwid.clone());
            state.hwid_index.write().unwrap().insert(hwid.clone(), token.clone());
        } else if license.hwid.as_deref() != Some(&hwid) {
            security::log_action(&token, "activate_failed", "hwid_mismatch", "blocked",
                req.ip.as_deref().unwrap_or("unknown"), &hwid);
            return Json(serde_json::json!({"ok":false,"error":"Токен привязан к другому устройству"}));
        }
        let tier = license.tier.clone();
        let remaining = license.requests_remaining();
        let expires = license.expires_at.map(|d| d.format("%Y-%m-%d").to_string());
        let allowed: Vec<String> = tier.allowed_modules().iter().map(|s| s.to_string()).collect();
        (tier, remaining, expires, allowed)
    };
    state.persist();

    // SECURITY: issue HWID-bound session token + per-session salt
    let session_id = Uuid::new_v4().to_string();
    let (jwt_token, hmac_salt) = security::generate_jwt(&hwid, &session_id);

    state.sessions.write().unwrap().insert(session_id.clone(), Session {
        id: session_id.clone(),
        token: token.clone(),
        hwid: hwid.clone(),
        ip: req.ip.unwrap_or_else(|| "unknown".into()),
        last_seen: Utc::now(),
        created_at: Utc::now(),
        app_version: req.app_version.unwrap_or_else(|| "unknown".into()),
        banned: false,
    });

    security::log_action(&token, "activate", &format!("tier:{}", tier.name()), "success", "", &hwid);

    let body = serde_json::json!({
        "ok": true,
        "tier": tier.name(),
        "requests_remaining": remaining,
        "expires": expires,
        "allowed_modules": allowed,
        "session_id": session_id,
    });
    let nonce = Uuid::new_v4().to_string().split('-').next().unwrap_or("").to_string();
    let signature = security::sign_response(&body, &nonce);
    Json(serde_json::json!({
        "header": { "timestamp": Utc::now().timestamp(), "nonce": nonce, "status": 200 },
        "body": body,
        "signature": signature,
        "jwt": jwt_token,
        "hmac_salt": hmac_salt,
    }))
}

/// Shared license gate: verifies a session's security context and consumes one quota
/// unit for `module`. Returns Ok(remaining) or Err(json error body).
fn authorize_and_consume(
    state: &SharedState,
    headers: &HeaderMap,
    token: &str,
    raw_hwid: &str,
    module: &str,
    session_id: &Option<String>,
) -> Result<i64, (StatusCode, serde_json::Value)> {
    let hwid = hash_hwid(raw_hwid);

    // 1. Session-token security context
    let jwt = hdr(headers, "x-auth-token");
    let ts: u64 = hdr(headers, "x-timestamp").parse().unwrap_or(0);
    let nonce = hdr(headers, "x-nonce");
    let sig = hdr(headers, "x-signature");

    if !security::timestamp_is_fresh(ts) {
        return Err((StatusCode::UNAUTHORIZED, serde_json::json!({"ok":false,"error":"Запрос устарел или недействителен (anti-replay)"})));
    }
    if !state.nonce_cache.register(nonce) {
        return Err((StatusCode::UNAUTHORIZED, serde_json::json!({"ok":false,"error":"Повторное использование nonce (replay)"})));
    }
    let claims = security::verify_jwt(jwt)
        .map_err(|e| (StatusCode::UNAUTHORIZED, serde_json::json!({"ok":false,"error":format!("Недействительный сессионный токен: {}", e)})))?;
    if claims.hwid != hwid {
        security::log_action(token, "osint_failed", "hwid_token_mismatch", "blocked", "", &hwid);
        return Err((StatusCode::UNAUTHORIZED, serde_json::json!({"ok":false,"error":"HWID не совпадает с токеном"})));
    }
    if !security::verify_request_signature(raw_hwid, nonce, ts, sig, &claims.salt) {
        security::log_action(token, "osint_failed", "bad_signature", "blocked", "", &hwid);
        return Err((StatusCode::UNAUTHORIZED, serde_json::json!({"ok":false,"error":"Неверная подпись запроса"})));
    }

    // 2. Rate limit (per HWID)
    if !state.rate_limiter.check(&hwid) {
        return Err((StatusCode::TOO_MANY_REQUESTS, serde_json::json!({"ok":false,"error":"Превышен лимит запросов, попробуйте позже"})));
    }

    // 3. Bans
    if let Some(sid) = session_id {
        if let Some(sess) = state.sessions.read().unwrap().get(sid) {
            if sess.banned {
                return Err((StatusCode::FORBIDDEN, serde_json::json!({"ok":false,"error":"Сессия заблокирована"})));
            }
        }
    }
    if state.banned_hwids.read().unwrap().contains(&hwid) {
        return Err((StatusCode::FORBIDDEN, serde_json::json!({"ok":false,"error":"Устройство заблокировано"})));
    }

    // 4. License checks + quota consume
    let remaining = {
        let mut licenses = state.licenses.write().unwrap();
        let license = licenses.get_mut(token)
            .ok_or((StatusCode::NOT_FOUND, serde_json::json!({"ok":false,"error":"Токен не найден"})))?;
        ensure_daily_reset(license);
        if !license.active || license.is_expired() {
            return Err((StatusCode::FORBIDDEN, serde_json::json!({"ok":false,"error":"Лицензия недействительна"})));
        }
        if license.hwid.as_deref() != Some(&hwid) {
            return Err((StatusCode::FORBIDDEN, serde_json::json!({"ok":false,"error":"HWID не совпадает"})));
        }
        let canon = osint::canonical_module(module);
        if !license.tier.allowed_modules().contains(&canon) {
            return Err((StatusCode::FORBIDDEN, serde_json::json!({"ok":false,"error":"Модуль недоступен на вашем тарифе"})));
        }
        if license.requests_limit != -1 && license.requests_used >= license.requests_limit {
            return Err((StatusCode::PAYMENT_REQUIRED, serde_json::json!({"ok":false,"error":"Лимит запросов исчерпан"})));
        }
        if license.requests_limit != -1 {
            license.requests_used += 1;
        }
        license.requests_remaining()
    };

    // Touch session last_seen
    if let Some(sid) = session_id {
        if let Some(sess) = state.sessions.write().unwrap().get_mut(sid) {
            sess.last_seen = Utc::now();
        }
    }
    Ok(remaining)
}

/// Legacy quota-only endpoint (kept for compatibility). Prefer /api/osint/run.
async fn consume_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ConsumeRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    match authorize_and_consume(&state, &headers, &token, req.hwid.trim(), &req.module, &req.session_id) {
        Ok(remaining) => {
            state.persist();
            security::log_action(&token, "consume", &req.module, "success", "", "");
            signed_envelope(serde_json::json!({"ok":true,"requests_remaining":remaining})).into_response()
        }
        Err((code, body)) => (code, Json(body)).into_response(),
    }
}

/// Authenticated OSINT execution — the server runs the module and returns results.
async fn osint_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<OsintRunRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    let target = req.target.trim().to_string();
    if target.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok":false,"error":"Пустая цель"}))).into_response();
    }
    if target.len() > 256 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok":false,"error":"Слишком длинная цель"}))).into_response();
    }

    let remaining = match authorize_and_consume(&state, &headers, &token, req.hwid.trim(), &req.module, &req.session_id) {
        Ok(r) => r,
        Err((code, body)) => return (code, Json(body)).into_response(),
    };
    state.persist();

    let findings = req.findings.unwrap_or_default();
    let events = osint::run_module(&req.module, &target, &findings).await;
    security::log_action(&token, "osint", &req.module, "success", "", "");

    signed_envelope(serde_json::json!({
        "ok": true,
        "requests_remaining": remaining,
        "module": osint::canonical_module(&req.module),
        "events": events,
    })).into_response()
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
    if !ct_eq(&req.admin_secret, admin_secret()) {
        return Json(serde_json::json!({"ok":false,"error":"Неверный секрет"}));
    }
    let token = req.token.trim().to_uppercase();
    let new_hwid = hash_hwid(&req.new_hwid);
    {
        let mut licenses = state.licenses.write().unwrap();
        match licenses.get_mut(&token) {
            Some(l) => {
                if let Some(old) = &l.hwid {
                    state.hwid_index.write().unwrap().remove(old);
                }
                l.hwid = Some(new_hwid.clone());
                state.hwid_index.write().unwrap().insert(new_hwid, token.clone());
            }
            None => return Json(serde_json::json!({"ok":false,"error":"Токен не найден"})),
        }
    }
    state.persist();
    Json(serde_json::json!({"ok":true,"message":"HWID успешно переназначен"}))
}

// ── File Upload ───────────────────────────────────────────────────────────────
const MAX_IMAGE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_VIDEO_SIZE: u64 = 100 * 1024 * 1024; // 100MB
const ALLOWED_IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];
const ALLOWED_VIDEO_TYPES: &[&str] = &["video/mp4", "video/webm", "video/quicktime"];

async fn upload_file(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut saved_files: Vec<UploadedFile> = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => continue,
        };
        let size = data.len() as u64;

        let is_image = ALLOWED_IMAGE_TYPES.contains(&content_type.as_str());
        let is_video = ALLOWED_VIDEO_TYPES.contains(&content_type.as_str());
        if !is_image && !is_video { continue; }
        if is_image && size > MAX_IMAGE_SIZE { continue; }
        if is_video && size > MAX_VIDEO_SIZE { continue; }

        let ext = match content_type.as_str() {
            "image/png" => "png", "image/jpeg" => "jpg", "image/gif" => "gif", "image/webp" => "webp",
            "video/mp4" => "mp4", "video/webm" => "webm", "video/quicktime" => "mov", _ => "bin",
        };
        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        let uploads_dir = StdPath::new("uploads");
        let _ = tokio_fs::create_dir_all(uploads_dir).await;
        let filepath = uploads_dir.join(&filename);
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
    let events = state.news.read().unwrap().clone();
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
        "social": {"status":"ok","note":"Reddit, GitHub, Telegram"},
        "ip_geo": {"status":"ok","note":"ip-api.com"},
        "whois": {"status":"ok","note":"rdap.org + cloudflare-dns"},
        "hibp": {"status":"ok","note":"haveibeenpwned.com"},
        "dorks": {"status":"ok","note":"static dork generation"},
        "paste": {"status":"ok","note":"doxbin scraping"},
        "darkweb": {"status":"ok","note":"ahmia.fi"},
        "phone": {"status":"ok","note":"veriphone.io"},
        "intelx": {"status": if std::env::var("INTELX_API_KEY").is_ok() {"ok"} else {"degraded"}, "note":"intelx.io (needs INTELX_API_KEY)"},
        "ai": {"status": if std::env::var("NVIDIA_NIM_API_KEY").is_ok() {"ok"} else {"degraded"}, "note":"NVIDIA NIM (needs NVIDIA_NIM_API_KEY)"},
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

// ── Discord State Endpoint ────────────────────────────────────────────────────
async fn discord_state(State(state): State<SharedState>) -> impl IntoResponse {
    Json(serde_json::to_value(state.discord_gateway.get_state()).unwrap())
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
    match payment::get_subscription_status(&token.trim().to_uppercase(), &state.licenses) {
        Some(sub) => (StatusCode::OK, Json(serde_json::to_value(sub).unwrap())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Token not found"}))).into_response(),
    }
}

// ── Admin Handlers ────────────────────────────────────────────────────────────
async fn admin_create_token(
    State(state): State<SharedState>,
    Json(req): Json<CreateTokenRequest>,
) -> impl IntoResponse {
    let tier = Tier::from_str(&req.tier);
    let note = req.note.unwrap_or_default();
    let license = License::new(tier.clone(), req.expires_days, &note);
    let token = license.token.clone();
    state.licenses.write().unwrap().insert(token.clone(), license);
    state.persist();
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
    let ok = {
        let mut licenses = state.licenses.write().unwrap();
        match licenses.get_mut(&token) {
            Some(l) => { l.active = false; true }
            None => false,
        }
    };
    if ok { state.persist(); Json(serde_json::json!({"ok":true})) }
    else { Json(serde_json::json!({"ok":false,"error":"Не найден"})) }
}

async fn admin_change_tier(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let new_tier = Tier::from_str(body["tier"].as_str().unwrap_or("free"));
    let ok = {
        let mut licenses = state.licenses.write().unwrap();
        match licenses.get_mut(&token) {
            Some(l) => {
                l.tier = new_tier.clone();
                l.requests_limit = new_tier.daily_requests();
                true
            }
            None => false,
        }
    };
    if ok { state.persist(); Json(serde_json::json!({"ok":true,"tier":new_tier.name()})) }
    else { Json(serde_json::json!({"ok":false,"error":"Не найден"})) }
}

async fn admin_reset_requests(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let ok = {
        let mut licenses = state.licenses.write().unwrap();
        match licenses.get_mut(&token) {
            Some(l) => { l.requests_used = 0; true }
            None => false,
        }
    };
    if ok { state.persist(); Json(serde_json::json!({"ok":true})) }
    else { Json(serde_json::json!({"ok":false,"error":"Не найден"})) }
}

async fn admin_rebind_hwid(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let new_hwid_raw = body["new_hwid"].as_str().unwrap_or("").to_string();
    let ok = {
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
                true
            }
            None => false,
        }
    };
    if ok { state.persist(); Json(serde_json::json!({"ok":true,"message":"HWID переназначен"})) }
    else { Json(serde_json::json!({"ok":false,"error":"Не найден"})) }
}

/// Hard-delete a token: removes it entirely (revoke only deactivates it).
async fn admin_purge_token(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let removed = {
        let mut licenses = state.licenses.write().unwrap();
        match licenses.remove(&token) {
            Some(lic) => {
                if let Some(h) = &lic.hwid {
                    state.hwid_index.write().unwrap().remove(h);
                }
                true
            }
            None => false,
        }
    };
    if removed {
        // Drop any live sessions issued for this token.
        state.sessions.write().unwrap().retain(|_, s| s.token != token);
        state.persist();
        Json(serde_json::json!({"ok":true,"message":"Токен удалён"}))
    } else {
        Json(serde_json::json!({"ok":false,"error":"Не найден"}))
    }
}

/// Ban a HWID directly — does not require an active session (sessions are ephemeral).
async fn admin_ban_hwid(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let hwid = body["hwid"].as_str().unwrap_or("").trim().to_string();
    let reason = body["reason"].as_str().unwrap_or("Нарушение правил").to_string();
    let admin_name = body["admin_name"].as_str().unwrap_or("admin").to_string();
    if hwid.is_empty() {
        return Json(serde_json::json!({"ok":false,"error":"HWID required"}));
    }
    state.banned_hwids.write().unwrap().insert(hwid.clone());
    state.ban_records.write().unwrap().push(BanRecord {
        hwid: hwid.clone(), reason, banned_at: Utc::now(), banned_by: admin_name,
        unbanned: false, unbanned_at: None, unbanned_by: None,
    });
    {
        let mut licenses = state.licenses.write().unwrap();
        for lic in licenses.values_mut() {
            if lic.hwid.as_deref() == Some(&hwid) {
                lic.hwid_banned = true;
                lic.active = false;
            }
        }
    }
    {
        let mut sessions = state.sessions.write().unwrap();
        for s in sessions.values_mut() {
            if s.hwid == hwid { s.banned = true; }
        }
    }
    state.persist();
    Json(serde_json::json!({"ok":true}))
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

    let (hwid, token) = {
        let mut sessions = state.sessions.write().unwrap();
        match sessions.get_mut(&session_id) {
            Some(sess) => {
                sess.banned = true;
                (sess.hwid.clone(), sess.token.clone())
            }
            None => return Json(serde_json::json!({"ok":false,"error":"Сессия не найдена"})),
        }
    };

    if ban_hwid {
        state.banned_hwids.write().unwrap().insert(hwid.clone());
        state.ban_records.write().unwrap().push(BanRecord {
            hwid: hwid.clone(), reason, banned_at: Utc::now(), banned_by: admin_name,
            unbanned: false, unbanned_at: None, unbanned_by: None,
        });
        {
            let mut licenses = state.licenses.write().unwrap();
            if let Some(lic) = licenses.get_mut(&token) {
                lic.hwid_banned = true;
                lic.active = false;
            }
        }
        state.persist();
    }
    Json(serde_json::json!({"ok":true,"banned_hwid":ban_hwid}))
}

async fn admin_list_bans(State(state): State<SharedState>) -> impl IntoResponse {
    let records = state.ban_records.read().unwrap();
    Json(serde_json::to_value(&*records).unwrap())
}

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
    {
        let mut records = state.ban_records.write().unwrap();
        for rec in records.iter_mut() {
            if rec.hwid == hwid && !rec.unbanned {
                rec.unbanned = true;
                rec.unbanned_at = Some(Utc::now());
                rec.unbanned_by = Some(admin_name.clone());
            }
        }
    }
    {
        let mut licenses = state.licenses.write().unwrap();
        for lic in licenses.values_mut() {
            if lic.hwid.as_deref() == Some(&hwid) {
                lic.hwid_banned = false;
                lic.active = true;
            }
        }
    }
    state.persist();
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
        attachments: req.attachments.unwrap_or_default(),
    };
    let id = item.id.clone();
    state.news.write().unwrap().push(item);
    state.persist();
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
    let ok = {
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
                if let Some(v) = body.get("attachments") {
                    if v.is_array() {
                        if let Ok(atts) = serde_json::from_value::<Vec<UploadedFile>>(v.clone()) {
                            item.attachments = atts;
                        }
                    }
                }
                true
            }
            None => false,
        }
    };
    if ok { state.persist(); Json(serde_json::json!({"ok":true})) }
    else { Json(serde_json::json!({"ok":false,"error":"Новость не найдена"})) }
}

async fn admin_delete_news(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let removed = {
        let mut news = state.news.write().unwrap();
        let before = news.len();
        news.retain(|n| n.id != id);
        news.len() < before
    };
    if removed { state.persist(); Json(serde_json::json!({"ok":true})) }
    else { Json(serde_json::json!({"ok":false,"error":"Не найдено"})) }
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

// ── Seed (only when no persisted state exists) ─────────────────────────────────
fn seed_demo(state: &SharedState) {
    {
        let mut licenses = state.licenses.write().unwrap();
        let mut admin = License::new(Tier::Admin, None, "Demo admin");
        admin.token = "RVN-ADMIN-DEMO-0000-0000".to_string();
        licenses.insert(admin.token.clone(), admin);
        let free = License::new(Tier::Free, Some(30), "Demo free 30 days");
        licenses.insert(free.token.clone(), free);
        let pro = License::new(Tier::Pro, Some(30), "Demo pro");
        licenses.insert(pro.token.clone(), pro);
    }
    state.news.write().unwrap().push(NewsItem {
        id: Uuid::new_v4().to_string(),
        title: "Ravens Nexus v2.8 — Новые возможности".to_string(),
        preview_text: "OSINT теперь выполняется на сервере: HWID-привязка, сессии, лента новостей и обновлённый интерфейс.".to_string(),
        full_text: "В этом обновлении:\n\n• OSINT-движок полностью на сервере\n• Привязка токена к HWID устройства\n• Сессии с баном по железу\n• Новостная лента с медиа\n• Обновлённая админ-панель\n• FREE: 2 запроса/день · PRO: 1000 · ELITE: ∞ + AI".to_string(),
        image_url: Some("https://images.unsplash.com/photo-1614064641938-3bbee52942c7?w=800".to_string()),
        video_url: None,
        media_position: "top".to_string(),
        media_rounded: true,
        created_at: Utc::now(),
        published: true,
        author: "Ravens Team".to_string(),
        attachments: vec![],
    });
    state.persist();
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let discord_gateway = Arc::new(discord::DiscordGateway::new());
    let bot_token = env_or("DISCORD_BOT_TOKEN", "");
    if !bot_token.is_empty() {
        discord_gateway.start(Some(bot_token)).await;
    }

    let state = Arc::new(AppState { discord_gateway, ..Default::default() });

    // Load persisted state, else seed demo data.
    match state.store.load::<Snapshot>() {
        Some(snap) if !snap.licenses.is_empty() => {
            let n = snap.licenses.len();
            state.load_snapshot(snap);
            tracing::info!("Loaded persisted state: {} licenses", n);
        }
        _ => {
            seed_demo(&state);
            tracing::info!("No persisted state — seeded demo data");
        }
    }

    let admin_routes = Router::new()
        .route("/tokens", post(admin_create_token))
        .route("/tokens", get(admin_list_tokens))
        .route("/tokens/:token", delete(admin_revoke_token))
        .route("/tokens/:token/tier", put(admin_change_tier))
        .route("/tokens/:token/reset-requests", post(admin_reset_requests))
        .route("/tokens/:token/rebind-hwid", post(admin_rebind_hwid))
        .route("/tokens/:token/purge", post(admin_purge_token))
        .route("/sessions", get(admin_list_sessions))
        .route("/sessions/:id/ban", post(admin_ban_session))
        .route("/hwid/ban", post(admin_ban_hwid))
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
        .route("/osint/run", post(osint_run))
        .route("/hwid/check", post(check_hwid_ban))
        .route("/news", get(get_news_public))
        .route("/news/stream", get(news_sse))
        .route("/discord/state", get(discord_state))
        .route("/payment/validate", post(validate_license))
        .route("/payment/create", post(create_payment))
        .route("/payment/status/:token", get(subscription_status));

    // CORS: the desktop webview fetches news/health cross-origin, so the API allows any
    // origin. State-changing endpoints are additionally gated by the admin secret or the
    // signed session-token scheme, so permissive CORS does not weaken auth.
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
    let using_env_secret = std::env::var("RAVENS_ADMIN_SECRET")
        .map(|v| v.len() >= 8 && v != "RAVENS_ADMIN_SECRET_CHANGE_ME")
        .unwrap_or(false);

    let public_url = std::env::var("REPLIT_DEV_DOMAIN")
        .ok()
        .map(|d| format!("https://{}", d))
        .or_else(|| std::env::var("REPLIT_DOMAINS").ok().map(|d| {
            let first = d.split(',').next().unwrap_or(&d).to_string();
            format!("https://{}", first)
        }))
        .unwrap_or_else(|| format!("http://localhost:{}", port));

    tracing::info!("================================================================");
    tracing::info!("  RAVENS NEXUS SERVER — STARTED");
    tracing::info!("================================================================");
    tracing::info!("  Local bind:      {}", addr);
    tracing::info!("  Public URL:      {}", public_url);
    tracing::info!("  Health check:    {}/health", public_url);
    tracing::info!("  Admin Panel:     {}/admin-ui/admin.html", public_url);
    tracing::info!("  API base:        {}/api", public_url);
    tracing::info!("----------------------------------------------------------------");
    // Touch the secret so a random one (if any) is generated and logged now.
    let _ = admin_secret();
    if using_env_secret {
        tracing::info!("  ADMIN SECRET: configured via RAVENS_ADMIN_SECRET env var");
    } else {
        tracing::warn!("  ADMIN SECRET: not configured — see generated value above (set RAVENS_ADMIN_SECRET!)");
    }
    tracing::info!("================================================================");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
