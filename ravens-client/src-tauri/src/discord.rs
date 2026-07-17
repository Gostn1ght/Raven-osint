use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEvent {
    pub module: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

impl DiscordEvent {
    pub fn new(module: &str, kind: &str, text: impl Into<String>) -> Self {
        Self {
            module: module.to_string(),
            kind: kind.to_string(),
            text: text.into(),
        }
    }
}

fn discord_client(token: &str) -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                token.parse().unwrap_or_default(),
            );
            headers
        })
        .build()
        .unwrap_or_default()
}

/// Get user info from Discord API
pub async fn get_discord_user(token: &str, user_id: &str) -> Vec<DiscordEvent> {
    let mut events = vec![DiscordEvent::new("discord", "running", format!("Discord: получение данных пользователя {}...", user_id))];
    let c = discord_client(token);
    
    let url = format!("https://discord.com/api/v10/users/{}", user_id);
    match c.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                let username = json["username"].as_str().unwrap_or("?");
                let discriminator = json["discriminator"].as_str().unwrap_or("0");
                let id = json["id"].as_str().unwrap_or("?");
                events.push(DiscordEvent::new("discord", "found", 
                    format!("Пользователь: #{} (ID: {})", username, id)));
                
                if let Some(banner) = json["banner"].as_str() {
                    events.push(DiscordEvent::new("discord", "found", 
                        format!("Баннер: {}", banner)));
                }
                if let Some(bio) = json["bio"].as_str() {
                    if !bio.is_empty() {
                        events.push(DiscordEvent::new("discord", "found", 
                            format!("Био: {}", bio)));
                    }
                }
            }
        }
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 401 {
                events.push(DiscordEvent::new("discord", "error", "Неверный токен Discord"));
            } else if status == 404 {
                events.push(DiscordEvent::new("discord", "error", "Пользователь не найден"));
            } else if status == 429 {
                events.push(DiscordEvent::new("discord", "error", "Discord API: лимит запросов"));
            } else {
                events.push(DiscordEvent::new("discord", "error", format!("Discord API: HTTP {}", status)));
            }
        }
        Err(e) => {
            events.push(DiscordEvent::new("discord", "error", format!("Ошибка сети: {}", e)));
        }
    }
    
    events.push(DiscordEvent::new("discord", "done", "Discord: завершён"));
    events
}

/// Get guilds (servers) for a user
pub async fn get_discord_guilds(token: &str) -> Vec<DiscordEvent> {
    let mut events = vec![DiscordEvent::new("discord", "running", "Discord: получение списка серверов...")];
    let c = discord_client(token);
    
    let url = "https://discord.com/api/v10/users/@me/guilds";
    match c.get(url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(arr) = r.json::<serde_json::Value>().await {
                if let Some(guilds) = arr.as_array() {
                    events.push(DiscordEvent::new("discord", "found", 
                        format!("Найдено серверов: {}", guilds.len())));
                    for guild in guilds.iter().take(20) {
                        let name = guild["name"].as_str().unwrap_or("?");
                        let id = guild["id"].as_str().unwrap_or("?");
                        let owner = guild["owner"].as_bool().unwrap_or(false);
                        if owner {
                            events.push(DiscordEvent::new("discord", "found", 
                                format!("★ {} (ID: {}) — ВЛАДЕЛЕЦ", name, id)));
                        } else {
                            events.push(DiscordEvent::new("discord", "found", 
                                format!("{} (ID: {})", name, id)));
                        }
                    }
                }
            }
        }
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 401 {
                events.push(DiscordEvent::new("discord", "error", "Неверный токен Discord"));
            } else {
                events.push(DiscordEvent::new("discord", "error", format!("Discord API: HTTP {}", status)));
            }
        }
        Err(e) => {
            events.push(DiscordEvent::new("discord", "error", format!("Ошибка сети: {}", e)));
        }
    }
    
    events.push(DiscordEvent::new("discord", "done", "Discord: список серверов получен"));
    events
}

/// Search messages in a channel (requires bot token with proper permissions)
pub async fn search_discord_messages(
    token: &str,
    channel_id: &str,
    query: &str,
    limit: u32,
) -> Vec<DiscordEvent> {
    let mut events = vec![DiscordEvent::new("discord", "running", 
        format!("Discord: поиск сообщений в канале {}...", channel_id))];
    let c = discord_client(token);
    
    let url = format!(
        "https://discord.com/api/v10/channels/{}/messages?limit={}&search={}",
        channel_id, limit, query
    );
    
    match c.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(arr) = r.json::<serde_json::Value>().await {
                if let Some(messages) = arr.as_array() {
                    events.push(DiscordEvent::new("discord", "found", 
                        format!("Найдено сообщений: {}", messages.len())));
                    for msg in messages.iter().take(50) {
                        let author = msg["author"]["username"].as_str().unwrap_or("?");
                        let content = msg["content"].as_str().unwrap_or("");
                        let timestamp = msg["timestamp"].as_str().unwrap_or("");
                        if !content.is_empty() {
                            events.push(DiscordEvent::new("discord", "found", 
                                format!("[{}] {}: {}", timestamp, author, 
                                    &content[..content.len().min(100)])));
                        }
                    }
                }
            }
        }
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 403 {
                events.push(DiscordEvent::new("discord", "error", "Нет доступа к каналу"));
            } else if status == 401 {
                events.push(DiscordEvent::new("discord", "error", "Неверный токен"));
            } else {
                events.push(DiscordEvent::new("discord", "error", format!("Discord API: HTTP {}", status)));
            }
        }
        Err(e) => {
            events.push(DiscordEvent::new("discord", "error", format!("Ошибка сети: {}", e)));
        }
    }
    
    events.push(DiscordEvent::new("discord", "done", "Discord: поиск завершён"));
    events
}
