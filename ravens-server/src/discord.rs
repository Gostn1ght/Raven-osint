// Discord Gateway Client (2.7.5)
// Connects to Discord WebSocket gateway and listens for events

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordGuild {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub member_count: u64,
    pub roles: Vec<DiscordRole>,
    pub channels: Vec<DiscordChannel>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordRole {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordChannel {
    pub id: String,
    pub name: String,
    pub kind: String, // "text", "voice", "category"
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordState {
    pub connected: bool,
    pub bot_token_configured: bool,
    pub guilds: Vec<DiscordGuild>,
    pub last_event: Option<String>,
    pub last_event_time: Option<String>,
}

impl Default for DiscordState {
    fn default() -> Self {
        Self {
            connected: false,
            bot_token_configured: false,
            guilds: vec![],
            last_event: None,
            last_event_time: None,
        }
    }
}

/// Discord Gateway state shared across handlers
pub struct DiscordGateway {
    pub state: Arc<DashMap<String, DiscordGuild>>,
    pub event_tx: broadcast::Sender<DiscordGatewayEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordGatewayEvent {
    pub event_type: String, // "guild_create", "guild_update", "channel_create", etc.
    pub guild_id: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: String,
}

impl DiscordGateway {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            state: Arc::new(DashMap::new()),
            event_tx,
        }
    }

    /// Start the Discord gateway connection (if token is configured)
    pub async fn start(&self, bot_token: Option<String>) {
        let token = match bot_token {
            Some(t) if !t.is_empty() => t,
            _ => {
                tracing::info!("Discord bot token not configured — gateway disabled");
                return;
            }
        };

        tracing::info!("Starting Discord Gateway connection...");
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            // Discord Gateway WebSocket URL
            let gateway_url = "wss://gateway.discord.gg/?v=10&encoding=json";
            
            loop {
                match tokio_tungstenite::connect_async(gateway_url).await {
                    Ok((ws_stream, _)) => {
                        tracing::info!("Discord Gateway: connected");
                        let (mut write, mut read) = ws_stream.split();
                        
                        // Send Identify payload
                        let identify = serde_json::json!({
                            "op": 2,
                            "d": {
                                "token": token,
                                "intents": 33280, // GUILDS + GUILD_MEMBERS + GUILD_MESSAGES
                                "properties": {
                                    "os": "linux",
                                    "browser": "ravens-nexus",
                                    "device": "ravens-nexus"
                                }
                            }
                        });
                        
                        if let Ok(msg) = serde_json::to_string(&identify) {
                            let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(msg)).await;
                        }

                        // Read events
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                        let op = json["op"].as_u64().unwrap_or(0);
                                        
                                        match op {
                                            0 => {
                                                // Dispatch event
                                                let event_type = json["t"].as_str().unwrap_or("").to_string();
                                                let guild_id = json["d"]["guild_id"].as_str().map(|s| s.to_string());
                                                
                                                // Store guild data
                                                if event_type == "GUILD_CREATE" || event_type == "GUILD_UPDATE" {
                                                    if let Some(guild) = json["d"].as_object() {
                                                        let guild_data = DiscordGuild {
                                                            id: guild["id"].as_str().unwrap_or("").to_string(),
                                                            name: guild["name"].as_str().unwrap_or("").to_string(),
                                                            owner_id: guild["owner_id"].as_str().unwrap_or("").to_string(),
                                                            member_count: guild["member_count"].as_u64().unwrap_or(0),
                                                            roles: guild["roles"].as_array()
                                                                .map(|arr| arr.iter().filter_map(|r| {
                                                                    Some(DiscordRole {
                                                                        id: r["id"].as_str()?.to_string(),
                                                                        name: r["name"].as_str()?.to_string(),
                                                                        color: r["color"].as_u64().unwrap_or(0) as u32,
                                                                        position: r["position"].as_u64().unwrap_or(0) as u32,
                                                                    })
                                                                }).collect())
                                                                .unwrap_or_default(),
                                                            channels: guild["channels"].as_array()
                                                                .map(|arr| arr.iter().filter_map(|c| {
                                                                    Some(DiscordChannel {
                                                                        id: c["id"].as_str()?.to_string(),
                                                                        name: c["name"].as_str()?.to_string(),
                                                                        kind: c["type"].as_u64().map(|t| match t {
                                                                            0 => "text",
                                                                            2 => "voice",
                                                                            4 => "category",
                                                                            _ => "unknown",
                                                                        }).unwrap_or("unknown").to_string(),
                                                                        parent_id: c["parent_id"].as_str().map(|s| s.to_string()),
                                                                    })
                                                                }).collect())
                                                                .unwrap_or_default(),
                                                            updated_at: chrono::Utc::now().to_rfc3339(),
                                                        };
                                                        state.insert(guild_data.id.clone(), guild_data);
                                                    }
                                                }
                                                
                                                if event_type == "GUILD_DELETE" {
                                                    if let Some(id) = &guild_id {
                                                        state.remove(id);
                                                    }
                                                }

                                                // Broadcast event
                                                let _ = event_tx.send(DiscordGatewayEvent {
                                                    event_type,
                                                    guild_id,
                                                    data: json["d"].clone(),
                                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                                });
                                            }
                                            10 => {
                                                // Hello — start heartbeat
                                                tracing::info!("Discord Gateway: received Hello");
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                                    tracing::warn!("Discord Gateway: connection closed");
                                    break;
                                }
                                Err(e) => {
                                    tracing::error!("Discord Gateway error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Discord Gateway connection failed: {}", e);
                    }
                }

                // Reconnect after delay
                tracing::info!("Discord Gateway: reconnecting in 5s...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }

    /// Get current state for API response
    pub fn get_state(&self) -> DiscordState {
        let guilds: Vec<DiscordGuild> = self.state.iter().map(|entry| entry.value().clone()).collect();
        DiscordState {
            connected: !self.state.is_empty(),
            bot_token_configured: true,
            guilds,
            last_event: None,
            last_event_time: None,
        }
    }
}
