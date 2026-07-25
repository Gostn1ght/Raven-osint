// ═══════════════════════════════════════════════════════════════════════════════
// Discord self-bot bridge — talks to the Node sidecar (discord.js-selfbot-v13).
// ═══════════════════════════════════════════════════════════════════════════════
// WARNING: self-bots automate a USER account and violate Discord's Terms of Service
// (account-ban risk). The operator supplies their own token at runtime; nothing is
// hard-coded. We spawn the sidecar with tokio::process and speak newline-delimited JSON
// over stdio (see sidecar/discord-selfbot.js).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>;

struct Bridge {
    child: Child,
    stdin: ChildStdin,
    pending: Pending,
    next_id: AtomicU64,
    ready: Arc<AtomicBool>,
    user: Arc<Mutex<Option<String>>>,
}

#[derive(Default)]
pub struct DiscordState {
    inner: Mutex<Option<Bridge>>,
}

/// Find the sidecar entry script across dev and bundled layouts.
fn resolve_sidecar(app: &tauri::AppHandle) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RAVENS_DISCORD_SIDECAR") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(res) = app.path().resource_dir() {
        let pb = res.join("sidecar").join("discord-selfbot.js");
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..4 {
            if let Some(d) = &dir {
                for cand in [
                    d.join("sidecar").join("discord-selfbot.js"),
                    d.join("src-tauri").join("sidecar").join("discord-selfbot.js"),
                ] {
                    if cand.exists() {
                        return Some(cand);
                    }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    None
}

/// Spawn the sidecar (once) and start the stdout reader task.
async fn ensure_bridge(app: &tauri::AppHandle, state: &DiscordState) -> Result<(), String> {
    let mut guard = state.inner.lock().await;
    if guard.is_some() {
        return Ok(());
    }
    let script = resolve_sidecar(app).ok_or("Не найден скрипт Discord-сайдкара")?;
    let workdir = script.parent().map(|p| p.to_path_buf());

    let mut cmd = Command::new("node");
    cmd.arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(d) = workdir {
        cmd.current_dir(d);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Не удалось запустить node (установлен?): {}", e))?;

    let stdin = child.stdin.take().ok_or("нет stdin у сайдкара")?;
    let stdout = child.stdout.take().ok_or("нет stdout у сайдкара")?;

    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let ready = Arc::new(AtomicBool::new(false));
    let user = Arc::new(Mutex::new(None));

    {
        let pending = pending.clone();
        let ready = ready.clone();
        let user = user.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                if let Some(ev) = v.get("event").and_then(|e| e.as_str()) {
                    if ev == "ready" {
                        ready.store(true, Ordering::SeqCst);
                        *user.lock().await = v.get("user").and_then(|x| x.as_str()).map(String::from);
                    }
                    continue;
                }
                if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                    if let Some(tx) = pending.lock().await.remove(&id) {
                        let _ = tx.send(v);
                    }
                }
            }
        });
    }

    *guard = Some(Bridge {
        child,
        stdin,
        pending,
        next_id: AtomicU64::new(1),
        ready,
        user,
    });
    Ok(())
}

/// Send one request and await its correlated response.
async fn call(state: &DiscordState, mut payload: Value, timeout_s: u64) -> Result<Value, String> {
    let (rx, id) = {
        let mut guard = state.inner.lock().await;
        let bridge = guard.as_mut().ok_or("Discord-сайдкар не запущен")?;
        let id = bridge.next_id.fetch_add(1, Ordering::SeqCst);
        payload["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        bridge.pending.lock().await.insert(id, tx);
        let mut line = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        line.push('\n');
        bridge
            .stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("сайдкар недоступен: {}", e))?;
        let _ = bridge.stdin.flush().await;
        (rx, id)
    };

    match tokio::time::timeout(Duration::from_secs(timeout_s), rx).await {
        Ok(Ok(v)) => {
            if v.get("ok").and_then(|o| o.as_bool()) == Some(false) {
                Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("ошибка сайдкара").to_string())
            } else {
                Ok(v.get("result").cloned().unwrap_or(Value::Null))
            }
        }
        Ok(Err(_)) => Err("сайдкар закрыл канал".into()),
        Err(_) => {
            if let Some(bridge) = state.inner.lock().await.as_ref() {
                bridge.pending.lock().await.remove(&id);
            }
            Err("таймаут ответа Discord-сайдкара".into())
        }
    }
}

/// Forward helper used by the Telegram→Discord pipeline (not a Tauri command).
pub async fn send_message(state: &DiscordState, channel_id: &str, content: &str) -> Result<(), String> {
    call(state, json!({ "cmd": "send", "channelId": channel_id, "content": content }), 30)
        .await
        .map(|_| ())
}

/// Log in the self-bot with a user token.
#[tauri::command]
pub async fn discord_login(
    app: tauri::AppHandle,
    token: String,
    state: tauri::State<'_, DiscordState>,
) -> Result<Value, String> {
    if token.trim().is_empty() {
        return Err("Укажите токен пользователя Discord".into());
    }
    ensure_bridge(&app, &state).await?;
    call(&state, json!({ "cmd": "login", "token": token.trim() }), 60).await
}

/// List text channels the self-bot account can post to.
#[tauri::command]
pub async fn discord_list_channels(state: tauri::State<'_, DiscordState>) -> Result<Value, String> {
    call(&state, json!({ "cmd": "listChannels" }), 30).await
}

/// Send a message to a channel (used by the forwarder).
#[tauri::command]
pub async fn discord_send(
    channel_id: String,
    content: String,
    state: tauri::State<'_, DiscordState>,
) -> Result<Value, String> {
    call(&state, json!({ "cmd": "send", "channelId": channel_id, "content": content }), 30).await
}

/// Local connection status (no round-trip to the sidecar).
#[tauri::command]
pub async fn discord_status(state: tauri::State<'_, DiscordState>) -> Result<Value, String> {
    let guard = state.inner.lock().await;
    match guard.as_ref() {
        Some(b) => {
            let ready = b.ready.load(Ordering::SeqCst);
            let user = b.user.lock().await.clone();
            Ok(json!({ "loggedIn": true, "ready": ready, "user": user }))
        }
        None => Ok(json!({ "loggedIn": false, "ready": false, "user": null })),
    }
}

/// Kill the sidecar and forget the session.
#[tauri::command]
pub async fn discord_logout(state: tauri::State<'_, DiscordState>) -> Result<(), String> {
    let mut guard = state.inner.lock().await;
    if let Some(mut b) = guard.take() {
        let _ = b.child.start_kill();
    }
    Ok(())
}
