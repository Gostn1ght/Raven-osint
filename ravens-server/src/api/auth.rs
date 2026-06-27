use axum::{Router, routing::post, Json, extract::State, Extension};
use serde::{Deserialize, Serialize};
use crate::{AppState, errors::{AppError, AppResult}, auth};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login",    post(login))
        .route("/refresh",  post(refresh))
        .route("/hwid",     post(bind_hwid))
        .with_state(state)
}

// ─── Register ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub email:    String,
    pub password: String,
    pub hwid:     Option<String>,
}

#[derive(Serialize)]
pub struct AuthResp {
    pub token:    String,
    pub user_id:  String,
    pub username: String,
    pub rank:     String,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterReq>,
) -> AppResult<Json<AuthResp>> {
    if body.username.len() < 3 || body.username.len() > 40 {
        return Err(AppError::BadRequest("username must be 3-40 chars".into()));
    }
    if body.password.len() < 8 {
        return Err(AppError::BadRequest("password must be 8+ chars".into()));
    }
    if state.db.find_user_by_email(&body.email).await?.is_some() {
        return Err(AppError::BadRequest("email already taken".into()));
    }
    if state.db.find_user_by_username(&body.username).await?.is_some() {
        return Err(AppError::BadRequest("username already taken".into()));
    }

    let hash = auth::hash_password(&body.password)
        .map_err(|e| AppError::Internal(e))?;
    let user = state.db.create_user(&body.username, &body.email, &hash).await?;

    if let Some(hwid) = &body.hwid {
        state.db.set_hwid(&user.id, hwid).await?;
    }

    let token = auth::issue_token(&user.id, &user.rank, &state.settings.jwt_secret, 30)
        .map_err(|e| AppError::Internal(e))?;

    Ok(Json(AuthResp { token, user_id: user.id, username: user.username, rank: user.rank }))
}

// ─── Login ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginReq {
    pub email:    String,
    pub password: String,
    pub hwid:     Option<String>,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginReq>,
) -> AppResult<Json<AuthResp>> {
    let user = state.db.find_user_by_email(&body.email).await?
        .ok_or(AppError::Unauthorized)?;

    // Brute-force lockout
    if let Some(locked) = user.locked_until {
        if locked > chrono::Utc::now() {
            return Err(AppError::Forbidden);
        }
    }
    if !auth::verify_password(&body.password, &user.password_hash) {
        let attempts = state.db.record_failed_login(&user.id).await?;
        if attempts >= 5 {
            sqlx::query("UPDATE users SET locked_until = datetime('now','+15 minutes') WHERE id=?")
                .bind(&user.id).execute(&state.db.pool).await?;
        }
        return Err(AppError::Unauthorized);
    }

    // HWID check
    if let Some(existing_hwid) = &user.hwid {
        if let Some(req_hwid) = &body.hwid {
            if existing_hwid != req_hwid {
                return Err(AppError::Forbidden); // wrong machine
            }
        }
    } else if let Some(hwid) = &body.hwid {
        state.db.set_hwid(&user.id, hwid).await?;
    }

    state.db.update_last_login(&user.id).await?;
    let token = auth::issue_token(&user.id, &user.rank, &state.settings.jwt_secret, 30)
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(AuthResp { token, user_id: user.id, username: user.username, rank: user.rank }))
}

// ─── Refresh ───────────────────────────────────────────────────────────────

async fn refresh(
    State(state): State<AppState>,
    Extension(au): Extension<crate::auth::AuthUser>,
) -> AppResult<Json<AuthResp>> {
    let user = state.db.find_user_by_id(&au.id).await?
        .ok_or(AppError::Unauthorized)?;
    let token = auth::issue_token(&user.id, &user.rank, &state.settings.jwt_secret, 30)
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(AuthResp { token, user_id: user.id, username: user.username, rank: user.rank }))
}

// ─── Bind HWID ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HwidReq { hwid: String }

async fn bind_hwid(
    State(state): State<AppState>,
    Extension(au): Extension<crate::auth::AuthUser>,
    Json(body): Json<HwidReq>,
) -> AppResult<Json<serde_json::Value>> {
    state.db.set_hwid(&au.id, &body.hwid).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
