// Database persistence layer (SQLite)
// Stores: licenses, sessions, ban_records, news, subscriptions

use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbLicense {
    pub token: String,
    pub tier: String,
    pub hwid: Option<String>,
    pub requests_used: i64,
    pub requests_limit: i64,
    pub active: bool,
    pub hwid_banned: bool,
    pub note: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbBanRecord {
    pub hwid: String,
    pub reason: String,
    pub banned_at: String,
    pub banned_by: String,
    pub unbanned: bool,
    pub unbanned_at: Option<String>,
    pub unbanned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbNews {
    pub id: String,
    pub title: String,
    pub preview_text: String,
    pub full_text: String,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub published: bool,
    pub author: String,
    pub created_at: String,
}

impl Database {
    /// Initialize database connection and create tables
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS licenses (
                token TEXT PRIMARY KEY,
                tier TEXT NOT NULL,
                hwid TEXT,
                requests_used INTEGER DEFAULT 0,
                requests_limit INTEGER DEFAULT 0,
                active BOOLEAN DEFAULT 1,
                hwid_banned BOOLEAN DEFAULT 0,
                note TEXT DEFAULT '',
                created_at TEXT NOT NULL,
                expires_at TEXT
            )
            "#
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ban_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hwid TEXT NOT NULL,
                reason TEXT NOT NULL,
                banned_at TEXT NOT NULL,
                banned_by TEXT NOT NULL,
                unbanned BOOLEAN DEFAULT 0,
                unbanned_at TEXT,
                unbanned_by TEXT
            )
            "#
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS news (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                preview_text TEXT NOT NULL,
                full_text TEXT NOT NULL,
                image_url TEXT,
                video_url TEXT,
                published BOOLEAN DEFAULT 1,
                author TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                token TEXT NOT NULL,
                hwid TEXT NOT NULL,
                ip TEXT,
                app_version TEXT,
                banned BOOLEAN DEFAULT 0,
                created_at TEXT NOT NULL,
                last_seen TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Save or update a license
    pub async fn save_license(&self, license: &DbLicense) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO licenses 
            (token, tier, hwid, requests_used, requests_limit, active, hwid_banned, note, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&license.token)
        .bind(&license.tier)
        .bind(&license.hwid)
        .bind(license.requests_used)
        .bind(license.requests_limit)
        .bind(license.active)
        .bind(license.hwid_banned)
        .bind(&license.note)
        .bind(&license.created_at)
        .bind(&license.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all licenses
    pub async fn get_licenses(&self) -> Result<Vec<DbLicense>, sqlx::Error> {
        sqlx::query_as!(
            DbLicense,
            r#"SELECT token, tier, hwid, requests_used, requests_limit, active, hwid_banned, note, created_at, expires_at FROM licenses"#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get a single license by token
    pub async fn get_license(&self, token: &str) -> Result<Option<DbLicense>, sqlx::Error> {
        sqlx::query_as!(
            DbLicense,
            r#"SELECT token, tier, hwid, requests_used, requests_limit, active, hwid_banned, note, created_at, expires_at FROM licenses WHERE token = ?"#,
            token
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Save a ban record
    pub async fn save_ban(&self, ban: &DbBanRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO ban_records (hwid, reason, banned_at, banned_by, unbanned, unbanned_at, unbanned_by)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&ban.hwid)
        .bind(&ban.reason)
        .bind(&ban.banned_at)
        .bind(&ban.banned_by)
        .bind(ban.unbanned)
        .bind(&ban.unbanned_at)
        .bind(&ban.unbanned_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all ban records
    pub async fn get_bans(&self) -> Result<Vec<DbBanRecord>, sqlx::Error> {
        sqlx::query_as!(
            DbBanRecord,
            r#"SELECT hwid, reason, banned_at, banned_by, unbanned, unbanned_at, unbanned_by FROM ban_records ORDER BY banned_at DESC"#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Save news item
    pub async fn save_news(&self, news: &DbNews) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO news (id, title, preview_text, full_text, image_url, video_url, published, author, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&news.id)
        .bind(&news.title)
        .bind(&news.preview_text)
        .bind(&news.full_text)
        .bind(&news.image_url)
        .bind(&news.video_url)
        .bind(news.published)
        .bind(&news.author)
        .bind(&news.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all news
    pub async fn get_news(&self) -> Result<Vec<DbNews>, sqlx::Error> {
        sqlx::query_as!(
            DbNews,
            r#"SELECT id, title, preview_text, full_text, image_url, video_url, published, author, created_at FROM news ORDER BY created_at DESC"#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Delete news by ID
    pub async fn delete_news(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM news WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

pub type SharedDatabase = Arc<Database>;
