// Payment Integration Module
// Supports: YooKassa, Paddle, SBP, Crypto (USDT)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub token: String,           // License token to activate/extend
    pub tier: String,            // "pro", "elite"
    pub period: String,          // "monthly", "annual"
    pub method: String,          // "yookassa", "paddle", "sbp", "crypto"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub success: bool,
    pub payment_id: Option<String>,
    pub payment_url: Option<String>,  // URL to redirect user
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub token: String,
    pub tier: String,
    pub activated_at: String,
    pub expires_at: String,
    pub payment_method: String,
    pub amount: f64,
    pub currency: String,
}

/// Validate a license key
pub fn validate_license_key(key: &str, licenses: &RwLock<HashMap<String, crate::License>>) -> Result<crate::LicenseInfo, String> {
    let licenses = licenses.read().map_err(|_| "Lock error")?;
    let key = key.trim().to_uppercase();
    match licenses.get(&key) {
        Some(lic) => {
            if !lic.active {
                Err("Лицензия деактивирована".to_string())
            } else if lic.is_expired() {
                Err("Лицензия истекла".to_string())
            } else {
                Ok(crate::LicenseInfo::from(lic))
            }
        }
        None => Err("Лицензия не найдена".to_string()),
    }
}

/// Process payment and activate/extend subscription
pub fn process_payment(
    req: PaymentRequest,
    licenses: &RwLock<HashMap<String, crate::License>>,
) -> Result<PaymentResponse, String> {
    // Calculate price based on tier and period
    let (amount, currency) = match req.tier.as_str() {
        "pro" => match req.period.as_str() {
            "monthly" => (990.0, "RUB"),
            "annual" => (9900.0, "RUB"),
            _ => return Err("Неверный период".to_string()),
        },
        "elite" => match req.period.as_str() {
            "monthly" => (2490.0, "RUB"),
            "annual" => (24900.0, "RUB"),
            _ => return Err("Неверный период".to_string()),
        },
        _ => return Err("Неверный тариф".to_string()),
    };

    // Generate payment ID
    let payment_id = format("PAY-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or(""));

    // Generate payment URL based on method
    let payment_url = match req.method.as_str() {
        "yookassa" => {
            // YooKassa payment URL (redirect to their payment page)
            Some(format!("https://yookassa.ru/checkout/payments/{}", payment_id))
        }
        "paddle" => {
            // Paddle checkout URL
            Some(format("https://checkout.paddle.com/checkout/custom/{}", payment_id))
        }
        "sbp" => {
            // SBP (СБП) payment - QR code or deep link
            Some(format("https://qr.nspk.ru/{}", payment_id))
        }
        "crypto" => {
            // Crypto payment (USDT TRC-20)
            Some(format("/payment/crypto/{}", payment_id))
        }
        _ => return Err("Неверный метод оплаты".to_string()),
    };

    // In production, this would integrate with actual payment provider APIs
    // For now, we return the payment URL for redirect

    Ok(PaymentResponse {
        success: true,
        payment_id: Some(payment_id),
        payment_url,
        message: format!("Счёт на {:.0} {} создан. Перенаправление на оплату...", amount, currency),
    })
}

/// Confirm payment called by payment provider webhook
pub fn confirm_payment(
    payment_id: &str,
    licenses: &RwLock<HashMap<String, crate::License>>,
) -> Result<String, String> {
    // In production, verify payment with provider
    // For demo, we just return success
    tracing::info!("Payment confirmed: {}", payment_id);
    Ok("Платёж подтверждён".to_string())
}

/// Get subscription status for a token
pub fn get_subscription_status(
    token: &str,
    licenses: &RwLock<HashMap<String, crate::License>>,
) -> Option<Subscription> {
    let licenses = licenses.read().ok()?;
    let lic = licenses.get(&token)?;
    
    Some(Subscription {
        token: token.to_string(),
        tier: lic.tier.name().to_string(),
        activated_at: lic.created_at.to_rfc3339(),
        expires_at: lic.expires_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
        payment_method: "manual".to_string(),
        amount: match lic.tier {
            crate::Tier::Pro => 990.0,
            crate::Tier::Elite => 2490.0,
            _ => 0.0,
        },
        currency: "RUB".to_string(),
    })
}
