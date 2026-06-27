use axum::{Router, routing::{get, post}, Json, extract::State, Extension};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use crate::{AppState, errors::{AppError, AppResult}, auth::AuthUser,
            osint::{sherlock, holehe, whois, geodata, dorks, image_search}};

pub fn router(state: AppState) -> Router {
    use crate::auth::require_auth;
    use axum::middleware;
    Router::new()
        .route("/username",    post(username_search))
        .route("/email",       post(email_check))
        .route("/whois",       post(whois_lookup))
        .route("/dns",         post(dns_lookup))
        .route("/ip",          post(ip_lookup))
        .route("/dorks",       post(generate_dorks))
        .route("/image",       post(image_geo))
        .route("/geoip",       post(geoip))
        .route("/aircraft",    get(aircraft))
        .route("/earthquakes", get(earthquakes))
        .route("/history",     get(user_history))
        .layer(middleware::from_fn(require_auth))
        .with_state(state)
}

// ─── helpers ──────────────────────────────────────────────────────────────

async fn check_limit(state: &AppState, user_id: &str, tool: &str) -> AppResult<()> {
    if !state.db.increment_requests(user_id).await? {
        return Err(AppError::LimitReached);
    }
    Ok(())
}

// ─── Username search ───────────────────────────────────────────────────────

#[derive(Deserialize)] struct UsernameReq { username: String }

async fn username_search(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<UsernameReq>,
) -> AppResult<Json<sherlock::SherlockResult>> {
    check_limit(&s, &au.id, "sherlock").await?;
    let t = Instant::now();
    let result = sherlock::search(&body.username, 25).await;
    s.db.log_request(&au.id, "sherlock", &body.username,
        Some(&serde_json::to_string(&result).unwrap_or_default()),
        t.elapsed().as_millis() as i64).await?;
    Ok(Json(result))
}

// ─── Email check ───────────────────────────────────────────────────────────

#[derive(Deserialize)] struct EmailReq { email: String }

async fn email_check(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<EmailReq>,
) -> AppResult<Json<holehe::HoleheResult>> {
    check_limit(&s, &au.id, "holehe").await?;
    let t = Instant::now();
    let result = holehe::check(&body.email, s.settings.hibp_api_key.as_deref()).await;
    s.db.log_request(&au.id, "holehe", &body.email,
        Some(&serde_json::to_string(&result).unwrap_or_default()),
        t.elapsed().as_millis() as i64).await?;
    Ok(Json(result))
}

// ─── WHOIS ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)] struct DomainReq { domain: String }

async fn whois_lookup(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<DomainReq>,
) -> AppResult<Json<whois::WhoisResult>> {
    check_limit(&s, &au.id, "whois").await?;
    let result = whois::lookup_whois(&body.domain).await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(result))
}

async fn dns_lookup(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<DomainReq>,
) -> AppResult<Json<whois::DnsResult>> {
    check_limit(&s, &au.id, "dns").await?;
    let result = whois::lookup_dns(&body.domain).await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(result))
}

// ─── IP ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)] struct IpReq { ip: String }

async fn ip_lookup(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<IpReq>,
) -> AppResult<Json<whois::IpInfoResult>> {
    check_limit(&s, &au.id, "ip").await?;
    let result = whois::lookup_ip(&body.ip).await
        .map_err(|e| AppError::Internal(e))?;
    Ok(Json(result))
}

// ─── Dorks ─────────────────────────────────────────────────────────────────

async fn generate_dorks(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<dorks::DorkRequest>,
) -> AppResult<Json<dorks::DorkResult>> {
    check_limit(&s, &au.id, "dorks").await?;
    Ok(Json(dorks::generate(&body)))
}

// ─── Image geo ─────────────────────────────────────────────────────────────

#[derive(Deserialize)] struct ImageReq { url: String }

async fn image_geo(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<ImageReq>,
) -> AppResult<Json<serde_json::Value>> {
    check_limit(&s, &au.id, "image_geo").await?;
    let key = s.settings.picarta_api_key.as_deref()
        .ok_or_else(|| AppError::BadRequest("Picarta API key not configured".into()))?;
    let geo  = geodata::picarta_geolocate(&body.url, key).await
        .map_err(|e| AppError::Internal(e))?;
    let rimg = if let Some(lk) = &s.settings.lenso_api_key {
        image_search::lenso_search(&body.url, lk).await.ok()
    } else { None };
    Ok(Json(serde_json::json!({ "geo": geo, "reverse_search": rimg })))
}

// ─── GeoIP ─────────────────────────────────────────────────────────────────

async fn geoip(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<IpReq>,
) -> AppResult<Json<geodata::GeoIpResult>> {
    check_limit(&s, &au.id, "geoip").await?;
    Ok(Json(geodata::geoip(&body.ip).await.map_err(|e| AppError::Internal(e))?))
}

// ─── Aircraft ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BboxQuery {
    lat_min: f64, lat_max: f64,
    lon_min: f64, lon_max: f64,
}

async fn aircraft(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    axum::extract::Query(q): axum::extract::Query<BboxQuery>,
) -> AppResult<Json<Vec<geodata::Aircraft>>> {
    check_limit(&s, &au.id, "aircraft").await?;
    Ok(Json(geodata::opensky_states(q.lat_min, q.lat_max, q.lon_min, q.lon_max).await
        .map_err(|e| AppError::Internal(e))?))
}

// ─── Earthquakes ───────────────────────────────────────────────────────────

#[derive(Deserialize)] struct QuakeQuery { min_magnitude: Option<f64> }

async fn earthquakes(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    axum::extract::Query(q): axum::extract::Query<QuakeQuery>,
) -> AppResult<Json<Vec<geodata::Earthquake>>> {
    check_limit(&s, &au.id, "earthquakes").await?;
    Ok(Json(geodata::usgs_earthquakes(q.min_magnitude.unwrap_or(4.0)).await
        .map_err(|e| AppError::Internal(e))?))
}

// ─── History ───────────────────────────────────────────────────────────────

async fn user_history(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
) -> AppResult<Json<Vec<crate::db::RequestLog>>> {
    Ok(Json(s.db.get_user_history(&au.id, 50).await?))
}
