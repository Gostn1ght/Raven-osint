use anyhow::Result;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub port:         u16,
    pub database_url: String,
    pub jwt_secret:   String,
    pub admin_token:  String,

    // External APIs
    pub nvidia_nim_api_key: Option<String>,
    pub hibp_api_key:       Option<String>,
    pub lenso_api_key:      Option<String>,
    pub picarta_api_key:    Option<String>,
    pub openai_api_key:     Option<String>,

    // Payments
    pub yoomoney_token:      Option<String>,
    pub cryptomus_merchant:  Option<String>,
    pub cryptomus_api_key:   Option<String>,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            port:         std::env::var("PORT").unwrap_or("8080".into()).parse()?,
            database_url: std::env::var("DATABASE_URL").unwrap_or("sqlite://ravens.db".into()),
            jwt_secret:   std::env::var("JWT_SECRET")
                              .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()),
            admin_token:  std::env::var("ADMIN_TOKEN")
                              .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()),
            nvidia_nim_api_key: std::env::var("NVIDIA_NIM_API_KEY").ok(),
            hibp_api_key:       std::env::var("HIBP_API_KEY").ok(),
            lenso_api_key:      std::env::var("LENSO_API_KEY").ok(),
            picarta_api_key:    std::env::var("PICARTA_API_KEY").ok(),
            openai_api_key:     std::env::var("OPENAI_API_KEY").ok(),
            yoomoney_token:     std::env::var("YOOMONEY_TOKEN").ok(),
            cryptomus_merchant: std::env::var("CRYPTOMUS_MERCHANT").ok(),
            cryptomus_api_key:  std::env::var("CRYPTOMUS_API_KEY").ok(),
        })
    }
}
