use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

// ─── Connection ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self> {
        // SQLite needs the file to exist first
        if let Some(path) = url.strip_prefix("sqlite://") {
            if path != ":memory:" {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                if !std::path::Path::new(path).exists() {
                    tokio::fs::File::create(path).await?;
                }
            }
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}

// ─── Models ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id:             String,         // UUID
    pub username:       String,
    pub email:          String,
    pub password_hash:  String,
    pub rank:           String,         // ANALYST | PRO | ELITE | ADMIN
    pub hwid:           Option<String>,
    pub api_key:        String,
    pub created_at:     DateTime<Utc>,
    pub last_login:     Option<DateTime<Utc>>,
    pub login_attempts: i64,
    pub locked_until:   Option<DateTime<Utc>>,
    pub avatar_url:     Option<String>,
    pub bio:            Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Subscription {
    pub id:          String,
    pub user_id:     String,
    pub plan:        String,            // free | basic | pro | elite
    pub expires_at:  Option<DateTime<Utc>>,
    pub requests_used:   i64,
    pub requests_limit:  i64,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RequestLog {
    pub id:         String,
    pub user_id:    String,
    pub tool:       String,
    pub query:      String,
    pub result:     Option<String>,     // JSON
    pub duration_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id:          String,
    pub user_id:     String,
    pub provider:    String,            // yoomoney | cryptomus
    pub amount:      f64,
    pub currency:    String,
    pub status:      String,            // pending | paid | failed
    pub plan:        String,
    pub external_id: Option<String>,
    pub created_at:  DateTime<Utc>,
    pub paid_at:     Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id:         String,
    pub user_id:    String,
    pub title:      String,
    pub body:       String,
    pub tags:       Option<String>,     // comma-separated
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comment {
    pub id:         String,
    pub post_id:    String,
    pub user_id:    String,
    pub body:       String,
    pub created_at: DateTime<Utc>,
}

// ─── Repositories ──────────────────────────────────────────────────────────

impl Database {
    // Users
    pub async fn create_user(&self, username: &str, email: &str, hash: &str) -> Result<User> {
        let id  = Uuid::new_v4().to_string();
        let api = format!("rvn_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, rank, api_key, created_at, login_attempts)
             VALUES (?,?,?,?,'ANALYST',?,datetime('now'),0)"
        )
        .bind(&id).bind(username).bind(email).bind(hash).bind(&api)
        .execute(&self.pool).await?;

        // Create free subscription
        self.create_subscription(&id, "free", None, 0, 20).await?;

        Ok(self.find_user_by_id(&id).await?.unwrap())
    }

    pub async fn find_user_by_id(&self, id: &str) -> Result<Option<User>> {
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<User>> {
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email).fetch_optional(&self.pool).await?)
    }

    pub async fn find_user_by_username(&self, username: &str) -> Result<Option<User>> {
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username).fetch_optional(&self.pool).await?)
    }

    pub async fn update_last_login(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET last_login = datetime('now'), login_attempts = 0 WHERE id = ?")
            .bind(user_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn record_failed_login(&self, user_id: &str) -> Result<i64> {
        sqlx::query(
            "UPDATE users SET login_attempts = login_attempts + 1 WHERE id = ?"
        ).bind(user_id).execute(&self.pool).await?;
        let (attempts,): (i64,) = sqlx::query_as(
            "SELECT login_attempts FROM users WHERE id = ?"
        ).bind(user_id).fetch_one(&self.pool).await?;
        Ok(attempts)
    }

    pub async fn set_hwid(&self, user_id: &str, hwid: &str) -> Result<()> {
        sqlx::query("UPDATE users SET hwid = ? WHERE id = ?")
            .bind(hwid).bind(user_id).execute(&self.pool).await?;
        Ok(())
    }

    // Subscriptions
    pub async fn create_subscription(
        &self,
        user_id: &str,
        plan: &str,
        expires_at: Option<DateTime<Utc>>,
        used: i64,
        limit: i64,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO subscriptions (id, user_id, plan, expires_at, requests_used, requests_limit, created_at)
             VALUES (?,?,?,?,?,?,datetime('now'))
             ON CONFLICT(user_id) DO UPDATE SET plan=excluded.plan, expires_at=excluded.expires_at,
             requests_used=excluded.requests_used, requests_limit=excluded.requests_limit"
        )
        .bind(&id).bind(user_id).bind(plan)
        .bind(expires_at.map(|d| d.to_rfc3339()))
        .bind(used).bind(limit)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_subscription(&self, user_id: &str) -> Result<Option<Subscription>> {
        Ok(sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions WHERE user_id = ?")
            .bind(user_id).fetch_optional(&self.pool).await?)
    }

    pub async fn increment_requests(&self, user_id: &str) -> Result<bool> {
        let sub = self.get_subscription(user_id).await?;
        match sub {
            None => return Ok(false),
            Some(s) if s.requests_used >= s.requests_limit => return Ok(false),
            Some(_) => {}
        }
        sqlx::query("UPDATE subscriptions SET requests_used = requests_used + 1 WHERE user_id = ?")
            .bind(user_id).execute(&self.pool).await?;
        Ok(true)
    }

    // Request logs
    pub async fn log_request(&self, user_id: &str, tool: &str, query: &str,
                              result: Option<&str>, duration_ms: i64) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO request_log (id, user_id, tool, query, result, duration_ms, created_at)
             VALUES (?,?,?,?,?,?,datetime('now'))"
        )
        .bind(&id).bind(user_id).bind(tool).bind(query).bind(result).bind(duration_ms)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_user_history(&self, user_id: &str, limit: i64) -> Result<Vec<RequestLog>> {
        Ok(sqlx::query_as::<_, RequestLog>(
            "SELECT * FROM request_log WHERE user_id = ? ORDER BY created_at DESC LIMIT ?"
        ).bind(user_id).bind(limit).fetch_all(&self.pool).await?)
    }

    // Payments
    pub async fn create_payment(&self, user_id: &str, provider: &str,
                                 amount: f64, currency: &str, plan: &str) -> Result<Payment> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO payments (id, user_id, provider, amount, currency, status, plan, created_at)
             VALUES (?,?,?,?,?,'pending',?,datetime('now'))"
        )
        .bind(&id).bind(user_id).bind(provider).bind(amount).bind(currency).bind(plan)
        .execute(&self.pool).await?;
        Ok(sqlx::query_as::<_, Payment>("SELECT * FROM payments WHERE id = ?")
            .bind(&id).fetch_one(&self.pool).await?)
    }

    pub async fn mark_payment_paid(&self, payment_id: &str, external_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE payments SET status='paid', external_id=?, paid_at=datetime('now') WHERE id = ?"
        ).bind(external_id).bind(payment_id).execute(&self.pool).await?;
        Ok(())
    }
}
