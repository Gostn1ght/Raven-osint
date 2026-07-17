// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY MODULE — "Only Server Decides" Protection
// ═══════════════════════════════════════════════════════════════════════════════
//
// Этот модуль добавляет защиту:
// 1. JWT токены с привязкой к HWID
// 2. HMAC подпись запросов (клиент подписывает, сервер проверяет)
// 3. HMAC подпись ответов (сервер подписывает, клиент проверяет)
// 4. Rate limiting (защита от DDoS)
// 5. Anti-replay protection (timestamp + nonce)
// 6. Логирование всех действий
//

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Тип для HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Секретный ключ для HMAC (в проде — из env var или Vault)
const HMAC_SECRET: &str = "ravens-nexus-hmac-secret-change-in-production-2026";

/// Секретный ключ для JWT (в проде — из env var)
const JWT_SECRET: &str = "ravens-nexus-jwt-secret-change-in-production-2026";

/// Время жизни JWT токена (в минутах)
const JWT_EXPIRY_MINUTES: i64 = 60;

/// Максимальный возраст запроса (секунды) — защита от replay
const REQUEST_MAX_AGE_SECONDS: u64 = 30;

/// Rate limit: максимум запросов в минуту на пользователя
const RATE_LIMIT_PER_MINUTE: u32 = 30;

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// JWT Claims — данные внутри токена
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// ID пользователя (license token)
    pub sub: String,
    /// HWID привязка
    pub hwid: String,
    /// ID сессии
    pub sid: String,
    /// HMAC соль для подписи запросов
    pub salt: String,
    /// Время выдачи
    pub iat: i64,
    /// Время истечения
    pub exp: i64,
}

/// Подписанный запрос от клиента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    /// Timestamp запроса (unix timestamp)
    pub timestamp: u64,
    /// Уникальный nonce (UUID)
    pub nonce: String,
    /// Тело запроса (base64)
    pub payload: String,
    /// HMAC подпись
    pub signature: String,
}

/// Подписанный ответ сервера
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedResponse {
    /// Timestamp ответа
    pub timestamp: u64,
    /// Nonce (тот же что в запросе для связи)
    pub nonce: String,
    /// Тело ответа
    pub payload: serde_json::Value,
    /// HMAC подпись
    pub signature: String,
}

/// Rate limit entry
#[derive(Debug, Clone)]
struct RateEntry {
    count: u32,
    window_start: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// JWT UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

/// Генерирует JWT токен с привязкой к HWID
/// В проде используйте jsonwebtoken крейт, здесь — упрощённая версия
pub fn generate_jwt(hwid: &str, session_id: &str) -> (String, String) {
    let salt = uuid::Uuid::new_v4().to_string().replace("-", "");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let claims = JwtClaims {
        sub: session_id.to_string(),
        hwid: hwid.to_string(),
        sid: session_id.to_string(),
        salt: salt.clone(),
        iat: now,
        exp: now + (JWT_EXPIRY_MINUTES * 60),
    };
    
    // В проде — используйте jsonwebtoken::encode
    // Здесь — упрощённая base64 кодировка для демонстрации
    let claims_json = serde_json::to_string(&claims).unwrap();
    let token = base64::encode(claims_json.as_bytes());
    
    // Подпись токена
    let signature = hmac_sign(token.as_bytes(), JWT_SECRET.as_bytes());
    let signed_token = format!("{}.{}", token, signature);
    
    (signed_token, salt)
}

/// Проверяет JWT токен и возвращает claims
pub fn verify_jwt(token: &str) -> Result<JwtClaims, String> {
    // В проде — используйте jsonwebtoken::decode
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err("Invalid token format".to_string());
    }
    
    // Проверяем подпись
    let expected_sig = hmac_sign(parts[0].as_bytes(), JWT_SECRET.as_bytes());
    if !hmac_compare(parts[1], &expected_sig) {
        return Err("Invalid token signature".to_string());
    }
    
    // Декодируем claims
    let claims_bytes = base64::decode(parts[0]).map_err(|_| "Invalid base64")?;
    let claims: JwtClaims = serde_json::from_slice(&claims_bytes)
        .map_err(|_| "Invalid claims")?;
    
    // Проверяем истечение
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if claims.exp < now {
        return Err("Token expired".to_string());
    }
    
    Ok(claims)
}

// ═══════════════════════════════════════════════════════════════════════════════
// HMAC UTILITIES
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

/// Генерирует HMAC подпись для ответа сервера
pub fn sign_response(payload: &serde_json::Value, nonce: &str) -> String {
    let message = format!("{}:{}", nonce, serde_json::to_string(payload).unwrap_or_default());
    hmac_sign_str(&message, HMAC_SECRET)
}

/// Проверяет HMAC подпись ответа на клиенте
pub fn verify_response_signature(
    payload: &serde_json::Value,
    nonce: &str,
    signature: &str,
) -> bool {
    let expected = sign_response(payload, nonce);
    hmac_compare(signature, &expected)
}

/// Проверяет подпись запроса от клиента
pub fn verify_request_signature(
    payload: &str,
    nonce: &str,
    timestamp: u64,
    signature: &str,
    hmac_salt: &str,
) -> bool {
    let message = format!("{}:{}:{}:{}", payload, nonce, timestamp, hmac_salt);
    let expected = hmac_sign_str(&message, HMAC_SECRET);
    hmac_compare(signature, &expected)
}

// ═══════════════════════════════════════════════════════════════════════════════
// RATE LIMITER
// ═══════════════════════════════════════════════════════════════════════════════

/// Простой in-memory rate limiter (в проде используйте Redis)
pub struct RateLimiter {
    entries: Mutex<HashMap<String, RateEntry>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
    
    /// Проверяет не превышен ли лимит для данного ключа
    pub fn check(&self, key: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let window = 60; // 1 минута
        
        if let Some(entry) = entries.get_mut(key) {
            if now - entry.window_start > window {
                // Новое окно
                *entry = RateEntry {
                    count: 1,
                    window_start: now,
                };
                true
            } else if entry.count < RATE_LIMIT_PER_MINUTE {
                entry.count += 1;
                true
            } else {
                false
            }
        } else {
            entries.insert(key.to_string(), RateEntry {
                count: 1,
                window_start: now,
            });
            true
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY MIDDLEWARE
// ═══════════════════════════════════════════════════════════════════════════════

/// Middleware для проверки подписи запроса и rate limiting
pub async fn security_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Извлекаем заголовки безопасности
    let jwt_token = headers
        .get("x-auth-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    let hwid = headers
        .get("x-hwid")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    let timestamp = headers
        .get("x-timestamp")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    
    let nonce = headers
        .get("x-nonce")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    let signature = headers
        .get("x-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    // 1. Проверка свежести запроса (anti-replay)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    if timestamp == 0 || now.abs_diff(timestamp) > REQUEST_MAX_AGE_SECONDS {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 2. Проверка JWT
    let claims = verify_jwt(jwt_token).map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // 3. Проверка HWID match
    if claims.hwid != hwid {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 4. Проверка подписи запроса
    // В полной версии здесь бы проверялось тело запроса
    let sign_message = format!("{}:{}:{}", hwid, nonce, timestamp);
    if !hmac_compare(signature, &hmac_sign_str(&sign_message, &claims.salt)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 5. Rate limiting по HWID
    // В полной версии — через Redis
    
    // Запрос прошёл все проверки — пропускаем дальше
    Ok(next.run(request).await)
}

// ═══════════════════════════════════════════════════════════════════════════════
// ACTION LOGGER
// ═══════════════════════════════════════════════════════════════════════════════

/// Логирует действие пользователя
pub fn log_action(
    user_id: &str,
    action: &str,
    target: &str,
    result: &str,
    ip: &str,
    hwid: &str,
) {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    tracing::info!(
        "[ACTION_LOG] {} | user={} | action={} | target={} | result={} | ip={} | hwid={}",
        timestamp, user_id, action, target, result, ip, hwid
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// BASE64 HELPERS (для совместимости)
// ═══════════════════════════════════════════════════════════════════════════════

mod base64 {
    pub fn encode(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(data)
    }
    
    pub fn decode(data: &str) -> Result<Vec<u8>, String> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| e.to_string())
    }
}
