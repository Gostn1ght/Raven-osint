use axum::{Router, routing::{get, post}, Json, extract::State, middleware};
use serde::{Deserialize, Serialize};
use crate::{AppState, auth::require_admin, errors::{AppError, AppResult}};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/users",            get(list_users))
        .route("/grant-subscription", post(grant_subscription))
        .route("/stats",            get(stats))
        .layer(middleware::from_fn(require_admin))
        .with_state(state)
}

#[derive(Serialize)]
struct UserSummary { id: String, username: String, email: String, rank: String, created_at: String }

async fn list_users(State(s): State<AppState>) -> AppResult<Json<Vec<UserSummary>>> {
    let rows = sqlx::query_as::<_, (String,String,String,String,String)>(
        "SELECT id,username,email,rank,created_at FROM users ORDER BY created_at DESC LIMIT 500"
    ).fetch_all(&s.db.pool).await?;
    Ok(Json(rows.into_iter().map(|(id,username,email,rank,created_at)|
        UserSummary { id, username, email, rank, created_at }).collect()))
}

#[derive(Deserialize)]
struct GrantReq {
    user_id: String,
    plan:    String,
    days:    i64,
    limit:   i64,
}

async fn grant_subscription(
    State(s): State<AppState>,
    Json(body): Json<GrantReq>,
) -> AppResult<Json<serde_json::Value>> {
    let expires = chrono::Utc::now() + chrono::Duration::days(body.days);
    s.db.create_subscription(&body.user_id, &body.plan, Some(expires), 0, body.limit).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
struct Stats { users: i64, requests_today: i64, active_subscriptions: i64 }

async fn stats(State(s): State<AppState>) -> AppResult<Json<Stats>> {
    let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&s.db.pool).await?;
    let (requests_today,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM request_log WHERE created_at >= date('now')"
    ).fetch_one(&s.db.pool).await?;
    let (active_subscriptions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM subscriptions WHERE plan != 'free' AND (expires_at IS NULL OR expires_at > datetime('now'))"
    ).fetch_one(&s.db.pool).await?;
    Ok(Json(Stats { users, requests_today, active_subscriptions }))
}
