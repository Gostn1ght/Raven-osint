use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};

// ─── JWT ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub:  String,   // user_id
    pub exp:  i64,
    pub iat:  i64,
    pub rank: String,
}

pub fn issue_token(user_id: &str, rank: &str, secret: &str, ttl_days: i64) -> Result<String> {
    let now = Utc::now();
    let claims = Claims {
        sub:  user_id.to_string(),
        rank: rank.to_string(),
        iat:  now.timestamp(),
        exp:  (now + Duration::days(ttl_days)).timestamp(),
    };
    Ok(encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))?)
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

// ─── Passwords ─────────────────────────────────────────────────────────────

pub fn hash_password(pw: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok()
}

// ─── HWID ──────────────────────────────────────────────────────────────────

/// Derive a deterministic HWID fingerprint from raw hardware strings.
/// The client collects: CPU-id, motherboard serial, MAC, disk serial.
pub fn derive_hwid(parts: &[&str]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    for p in parts { hasher.update(p.as_bytes()); hasher.update(b"|"); }
    hex::encode(hasher.finalize())
}

// ─── Middleware extractor ───────────────────────────────────────────────────

use axum::{extract::{FromRequestParts, Request}, http::request::Parts, middleware::Next,
           response::Response};
use crate::{AppState, errors::AppError};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id:   String,
    pub rank: String,
}

pub async fn require_auth(
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let state = req.extensions().get::<AppState>()
        .ok_or(AppError::Internal(anyhow::anyhow!("no state")))?.clone();

    let token = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = verify_token(token, &state.settings.jwt_secret)
        .map_err(|_| AppError::Unauthorized)?;

    req.extensions_mut().insert(AuthUser { id: claims.sub, rank: claims.rank });
    Ok(next.run(req).await)
}

pub async fn require_admin(
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let state = req.extensions().get::<AppState>()
        .ok_or(AppError::Internal(anyhow::anyhow!("no state")))?.clone();

    let token = req.headers()
        .get("X-Admin-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Forbidden)?;

    if token != state.settings.admin_token {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(req).await)
}
