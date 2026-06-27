use axum::{Router, routing::post, Json, extract::State, Extension, middleware};
use serde::{Deserialize, Serialize};
use crate::{AppState, auth::{require_auth, AuthUser}, errors::{AppError, AppResult},
            payments::{yoomoney, cryptomus}};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/create",           post(create_payment))
        .route("/yoomoney/callback",post(yoomoney_callback))
        .route("/cryptomus/callback",post(cryptomus_callback))
        .layer(middleware::from_fn(require_auth))
        .with_state(state)
}

#[derive(Deserialize)]
struct PaymentReq {
    provider: String,   // "yoomoney" | "cryptomus"
    plan:     String,   // "basic" | "pro" | "elite"
}

#[derive(Serialize)]
struct PaymentResp {
    payment_id:   String,
    redirect_url: String,
}

static PLANS: &[(&str, f64, i64, &str)] = &[
    // (plan, price_rub, monthly_limit, label)
    ("basic", 299.0,  500,  "Basic"),
    ("pro",   799.0,  2000, "Pro"),
    ("elite", 1999.0, 10000,"Elite"),
];

fn plan_price(plan: &str) -> Option<(f64, i64)> {
    PLANS.iter().find(|(p,_,_,_)| *p == plan).map(|(_,price,limit,_)| (*price, *limit))
}

async fn create_payment(
    State(s): State<AppState>, Extension(au): Extension<AuthUser>,
    Json(body): Json<PaymentReq>,
) -> AppResult<Json<PaymentResp>> {
    let (price, _limit) = plan_price(&body.plan)
        .ok_or_else(|| AppError::BadRequest("unknown plan".into()))?;

    let payment = s.db.create_payment(&au.id, &body.provider, price, "RUB", &body.plan).await?;

    let redirect_url = match body.provider.as_str() {
        "yoomoney" => {
            let token = s.settings.yoomoney_token.as_deref()
                .ok_or_else(|| AppError::BadRequest("YooMoney not configured".into()))?;
            yoomoney::create_payment_url(token, &payment.id, price, &body.plan)
        }
        "cryptomus" => {
            let merchant = s.settings.cryptomus_merchant.as_deref()
                .ok_or_else(|| AppError::BadRequest("Cryptomus not configured".into()))?;
            let key = s.settings.cryptomus_api_key.as_deref()
                .ok_or_else(|| AppError::BadRequest("Cryptomus not configured".into()))?;
            cryptomus::create_invoice(merchant, key, &payment.id, price).await
                .map_err(|e| AppError::Internal(e))?
        }
        _ => return Err(AppError::BadRequest("unknown provider".into())),
    };

    Ok(Json(PaymentResp { payment_id: payment.id, redirect_url }))
}

// ─── Callbacks ─────────────────────────────────────────────────────────────

#[derive(Deserialize)] struct YooCallback {
    label:        String,
    operation_id: String,
    amount:       f64,
    withdraw_amount: f64,
}

async fn yoomoney_callback(
    State(s): State<AppState>,
    Json(body): Json<YooCallback>,
) -> AppResult<Json<serde_json::Value>> {
    // body.label == payment_id
    s.db.mark_payment_paid(&body.label, &body.operation_id).await?;
    upgrade_subscription(&s, &body.label).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)] struct CryptoCallback {
    order_id: String,
    uuid:     String,
    status:   String,
}

async fn cryptomus_callback(
    State(s): State<AppState>,
    Json(body): Json<CryptoCallback>,
) -> AppResult<Json<serde_json::Value>> {
    if body.status == "paid" {
        s.db.mark_payment_paid(&body.order_id, &body.uuid).await?;
        upgrade_subscription(&s, &body.order_id).await?;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn upgrade_subscription(s: &AppState, payment_id: &str) -> AppResult<()> {
    let payment = sqlx::query_as::<_, crate::db::Payment>(
        "SELECT * FROM payments WHERE id=?"
    ).bind(payment_id).fetch_optional(&s.db.pool).await?
     .ok_or(AppError::NotFound)?;

    let (_price, limit) = plan_price(&payment.plan)
        .ok_or_else(|| AppError::BadRequest("unknown plan".into()))?;

    let expires = chrono::Utc::now() + chrono::Duration::days(30);
    s.db.create_subscription(&payment.user_id, &payment.plan, Some(expires), 0, limit).await?;
    Ok(())
}
