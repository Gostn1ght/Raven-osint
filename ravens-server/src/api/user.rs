use axum::{Router, routing::{get, put}, Json, extract::State, Extension, middleware};
use serde::{Deserialize, Serialize};
use crate::{AppState, auth::{require_auth, AuthUser}, errors::{AppError, AppResult}};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/me",          get(me))
        .route("/me",          put(update_profile))
        .route("/subscription",get(subscription))
        .route("/api-key",     get(get_api_key))
        .layer(middleware::from_fn(require_auth))
        .with_state(state)
}

#[derive(Serialize)]
struct UserResp {
    id:         String,
    username:   String,
    email:      String,
    rank:       String,
    avatar_url: Option<String>,
    bio:        Option<String>,
    created_at: String,
    last_login: Option<String>,
}

async fn me(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
) -> AppResult<Json<UserResp>> {
    let u = s.db.find_user_by_id(&au.id).await?.ok_or(AppError::Unauthorized)?;
    Ok(Json(UserResp {
        id: u.id, username: u.username, email: u.email, rank: u.rank,
        avatar_url: u.avatar_url, bio: u.bio,
        created_at: u.created_at.to_rfc3339(),
        last_login: u.last_login.map(|d| d.to_rfc3339()),
    }))
}

#[derive(Deserialize)]
struct UpdateProfileReq {
    bio:        Option<String>,
    avatar_url: Option<String>,
}

async fn update_profile(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<UpdateProfileReq>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET bio=COALESCE(?,bio), avatar_url=COALESCE(?,avatar_url) WHERE id=?")
        .bind(&body.bio).bind(&body.avatar_url).bind(&au.id)
        .execute(&s.db.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn subscription(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
) -> AppResult<Json<Option<crate::db::Subscription>>> {
    Ok(Json(s.db.get_subscription(&au.id).await?))
}

async fn get_api_key(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
) -> AppResult<Json<serde_json::Value>> {
    let (key,): (String,) = sqlx::query_as("SELECT api_key FROM users WHERE id=?")
        .bind(&au.id).fetch_one(&s.db.pool).await?;
    Ok(Json(serde_json::json!({ "api_key": key })))
}
