// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY MODULE — "Only Server Decides" Protection
// ═══════════════════════════════════════════════════════════════════════════════
//
// Protection layers:
//   1. Session tokens (HMAC-signed, HWID-bound, expiring) — verified server-side
//   2. Per-request HMAC signatures (client signs, server verifies)
//   3. Signed responses (server signs, client verifies — anti-tamper)
//   4. Rate limiting (per HWID) — anti-abuse / DoS
//   5. Anti-replay (timestamp freshness + one-time nonce cache)
//   6. Structured action logging
//
// Secrets are loaded from the environment at startup. If unset, a random secret is
// generated per-process (safe default: never a hard-coded production key).

use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION (env-driven, never hard-coded in production)
// ═══════════════════════════════════════════════════════════════════════════════

/// Token lifetime (minutes).
const TOKEN_EXPIRY_MINUTES: i64 = 60;
/// Maximum request age (seconds) — anti-replay freshness window.
pub const REQUEST_MAX_AGE_SECONDS: u64 = 30;
/// Rate limit: max requests per minute per key.
const RATE_LIMIT_PER_MINUTE: u32 = 30;

/// Generates a cryptographically-random hex secret.
fn random_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// HMAC secret for response/request signing — from `RAVENS_HMAC_SECRET` or random.
fn hmac_secret() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| {
        std::env::var("RAVENS_HMAC_SECRET").ok().filter(|v| v.len() >= 16).unwrap_or_else(|| {
            tracing::warn!("RAVENS_HMAC_SECRET not set (or too short) — using a random per-process secret");
            random_secret()
        })
    })
}

/// Session-token signing secret — from `RAVENS_JWT_SECRET` or random.
fn token_secret() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| {
        std::env::var("RAVENS_JWT_SECRET").ok().filter(|v| v.len() >= 16).unwrap_or_else(|| {
            tracing::warn!("RAVENS_JWT_SECRET not set (or too short) — using a random per-process secret");
            random_secret()
        })
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// Claims embedded in a session token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject — the session id.
    pub sub: String,
    /// HWID this token is bound to (hashed).
    pub hwid: String,
    /// Session id.
    pub sid: String,
    /// Per-session salt used to sign subsequent requests.
    pub salt: String,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds).
    pub exp: i64,
}

/// Rate-limit bucket.
#[derive(Debug, Clone)]
struct RateEntry {
    count: u32,
    window_start: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// SESSION TOKEN (HMAC-signed, self-describing — a compact JWS-style token)
// ═══════════════════════════════════════════════════════════════════════════════

/// Issues a session token bound to `hwid` + `session_id`. Returns `(token, salt)`.
pub fn generate_jwt(hwid: &str, session_id: &str) -> (String, String) {
    let salt = uuid::Uuid::new_v4().to_string().replace('-', "");
    let now = unix_now() as i64;
    let claims = TokenClaims {
        sub: session_id.to_string(),
        hwid: hwid.to_string(),
        sid: session_id.to_string(),
        salt: salt.clone(),
        iat: now,
        exp: now + TOKEN_EXPIRY_MINUTES * 60,
    };
    let claims_json = serde_json::to_string(&claims).unwrap_or_default();
    let payload = b64_encode(claims_json.as_bytes());
    let signature = hmac_sign(payload.as_bytes(), token_secret().as_bytes());
    (format!("{}.{}", payload, signature), salt)
}

/// Verifies a session token and returns its claims.
pub fn verify_jwt(token: &str) -> Result<TokenClaims, String> {
    let (payload, sig) = token.split_once('.').ok_or("invalid token format")?;
    let expected = hmac_sign(payload.as_bytes(), token_secret().as_bytes());
    if !hmac_compare(sig, &expected) {
        return Err("invalid token signature".to_string());
    }
    let claims_bytes = b64_decode(payload).map_err(|_| "invalid base64")?;
    let claims: TokenClaims = serde_json::from_slice(&claims_bytes).map_err(|_| "invalid claims")?;
    if claims.exp < unix_now() as i64 {
        return Err("token expired".to_string());
    }
    Ok(claims)
}

// ═══════════════════════════════════════════════════════════════════════════════
// HMAC UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

pub fn hmac_sign(data: &[u8], key: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

pub fn hmac_sign_str(data: &str, key: &str) -> String {
    hmac_sign(data.as_bytes(), key.as_bytes())
}

/// Constant-time string comparison.
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

/// Signs a server response body so the client can detect tampering / MITM.
pub fn sign_response(payload: &serde_json::Value, nonce: &str) -> String {
    let message = format!("{}:{}", nonce, serde_json::to_string(payload).unwrap_or_default());
    hmac_sign_str(&message, hmac_secret())
}

/// Verifies a per-request signature produced by the client.
/// The client signs `hwid:nonce:timestamp` with its per-session salt.
pub fn verify_request_signature(
    hwid: &str,
    nonce: &str,
    timestamp: u64,
    signature: &str,
    salt: &str,
) -> bool {
    let message = format!("{}:{}:{}", hwid, nonce, timestamp);
    hmac_compare(signature, &hmac_sign_str(&message, salt))
}

/// Checks anti-replay freshness of a request timestamp.
pub fn timestamp_is_fresh(timestamp: u64) -> bool {
    timestamp != 0 && unix_now().abs_diff(timestamp) <= REQUEST_MAX_AGE_SECONDS
}

// ═══════════════════════════════════════════════════════════════════════════════
// RATE LIMITER (in-memory; per-key sliding minute window)
// ═══════════════════════════════════════════════════════════════════════════════

pub struct RateLimiter {
    entries: Mutex<HashMap<String, RateEntry>>,
    limit: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RATE_LIMIT_PER_MINUTE)
    }
}

impl RateLimiter {
    pub fn new(limit: u32) -> Self {
        Self { entries: Mutex::new(HashMap::new()), limit }
    }

    /// Returns `true` if the request is allowed, `false` if the limit is exceeded.
    pub fn check(&self, key: &str) -> bool {
        let now = unix_now();
        let mut entries = self.entries.lock().unwrap();
        // Opportunistic prune of stale buckets to bound memory.
        entries.retain(|_, e| now.saturating_sub(e.window_start) <= 120);
        match entries.get_mut(key) {
            Some(entry) if now.saturating_sub(entry.window_start) <= 60 => {
                if entry.count < self.limit {
                    entry.count += 1;
                    true
                } else {
                    false
                }
            }
            _ => {
                entries.insert(key.to_string(), RateEntry { count: 1, window_start: now });
                true
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NONCE CACHE (one-time-use nonces — closes the replay window)
// ═══════════════════════════════════════════════════════════════════════════════

pub struct NonceCache {
    seen: Mutex<HashMap<String, u64>>,
}

impl Default for NonceCache {
    fn default() -> Self {
        Self { seen: Mutex::new(HashMap::new()) }
    }
}

impl NonceCache {
    /// Records a nonce. Returns `false` if it was already used (replay).
    pub fn register(&self, nonce: &str) -> bool {
        if nonce.is_empty() {
            return false;
        }
        let now = unix_now();
        let mut seen = self.seen.lock().unwrap();
        seen.retain(|_, ts| now.saturating_sub(*ts) <= REQUEST_MAX_AGE_SECONDS * 2);
        if seen.contains_key(nonce) {
            return false;
        }
        seen.insert(nonce.to_string(), now);
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ACTION LOGGER
// ═══════════════════════════════════════════════════════════════════════════════

pub fn log_action(user_id: &str, action: &str, target: &str, result: &str, ip: &str, hwid: &str) {
    tracing::info!(
        "[ACTION] {} | user={} | action={} | target={} | result={} | ip={} | hwid={}",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        user_id, action, target, result, ip, short(hwid)
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Truncates a long id (e.g. HWID hash) for log hygiene.
fn short(s: &str) -> String {
    if s.len() > 12 { format!("{}…", &s[..12]) } else { s.to_string() }
}

fn b64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn b64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip_and_signature() {
        let (tok, salt) = generate_jwt("hwidhash", "sess-1");
        let claims = verify_jwt(&tok).expect("valid token");
        assert_eq!(claims.hwid, "hwidhash");
        assert_eq!(claims.salt, salt);

        // A tampered payload must fail verification.
        let mut parts = tok.split('.');
        let bad = format!("{}.{}", parts.next().unwrap(), "deadbeef");
        assert!(verify_jwt(&bad).is_err());
    }

    #[test]
    fn request_signature_matches_client_scheme() {
        let (_tok, salt) = generate_jwt("hw", "s");
        let sig = hmac_sign_str(&format!("{}:{}:{}", "hw", "nonce123", 1000u64), &salt);
        assert!(verify_request_signature("hw", "nonce123", 1000, &sig, &salt));
        assert!(!verify_request_signature("hw", "nonce123", 1001, &sig, &salt));
    }

    #[test]
    fn nonce_cache_rejects_replay() {
        let cache = NonceCache::default();
        assert!(cache.register("abc"));
        assert!(!cache.register("abc"));
    }

    #[test]
    fn rate_limiter_enforces_limit() {
        let rl = RateLimiter::new(2);
        assert!(rl.check("k"));
        assert!(rl.check("k"));
        assert!(!rl.check("k"));
    }
}
