// ═══════════════════════════════════════════════════════════════════════════════
// Telegram MTProto (user account) + TG→Discord forwarding — grammers 0.10.
// ═══════════════════════════════════════════════════════════════════════════════
// Connects a real USER account so the operator can pick one of their own channels as a
// forwarding source. Credentials (api_id/api_hash, phone, code, 2FA password) are supplied
// by the operator at runtime and never hard-coded.
//
// v1 uses an in-memory session (no on-disk persistence) to avoid pulling grammers' SQLite
// (libsql) backend, which needs a C toolchain to build. Consequence: the account must be
// re-connected after the app restarts. Persisting the session is a tracked follow-up.

use std::sync::Arc;

use grammers_client::client::{LoginToken, PasswordToken, SignInError, UpdatesConfiguration};
use grammers_client::peer::Peer;
use grammers_client::update::Update;
use grammers_client::{Client, SenderPool};
use grammers_session::storages::MemorySession;
use grammers_session::SessionData;
use serde::Serialize;
use tauri::Manager;
use tokio::sync::Mutex;

/// Update receiver produced by the SenderPool; consumed once by `stream_updates`.
type UpdatesRx = tokio::sync::mpsc::UnboundedReceiver<grammers_client::session::updates::UpdatesLike>;

/// A live connection + in-flight login state, kept alive between Tauri commands.
struct TgConn {
    client: Client,
    // Keeping the runner's JoinHandle lets us abort the network task on logout.
    runner: tokio::task::JoinHandle<()>,
    // Single-use update stream receiver; taken when forwarding starts.
    updates: Option<UpdatesRx>,
    login: Option<LoginToken>,
    password: Option<PasswordToken>,
    authorized: bool,
}

#[derive(Default)]
pub struct TgState {
    inner: Mutex<Option<TgConn>>,
}

/// Forwarding runs as one long-lived update loop driven by a shared config, so the UI can
/// start/stop/retarget without reconnecting Telegram.
#[derive(Default)]
pub struct ForwardState {
    config: Arc<Mutex<Option<ForwardCfg>>>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Clone)]
struct ForwardCfg {
    tg_source_id: String,
    discord_target_id: String,
}

#[derive(Serialize)]
pub struct TgStatus {
    /// "code_sent" | "password_required" | "authorized" | "disconnected"
    pub stage: String,
    pub user: Option<String>,
}

#[derive(Serialize)]
pub struct TgChannel {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
}

#[derive(Serialize)]
pub struct ForwardStatus {
    pub active: bool,
    pub tg_source_id: Option<String>,
    pub discord_target_id: Option<String>,
}

/// Build a fresh in-memory client and spawn its network runner.
fn spawn_client(api_id: i32) -> (Client, tokio::task::JoinHandle<()>, UpdatesRx) {
    let session = Arc::new(MemorySession::from(SessionData::default()));
    let SenderPool { runner, handle, updates } = SenderPool::new(session, api_id);
    let client = Client::new(handle);
    let task = tokio::spawn(async move {
        runner.run().await;
    });
    (client, task, updates)
}

/// Stop any running forwarder (used on logout / re-login).
async fn stop_forwarder(fwd: &ForwardState) {
    if let Some(h) = fwd.task.lock().await.take() {
        h.abort();
    }
    *fwd.config.lock().await = None;
}

/// Step 1 — connect and request a login code for `phone`.
#[tauri::command]
pub async fn tg_request_code(
    api_id: i32,
    api_hash: String,
    phone: String,
    state: tauri::State<'_, TgState>,
    fwd: tauri::State<'_, ForwardState>,
) -> Result<TgStatus, String> {
    if api_id <= 0 || api_hash.trim().is_empty() {
        return Err("Укажите api_id и api_hash (my.telegram.org)".into());
    }
    let phone = phone.trim().to_string();
    if phone.is_empty() {
        return Err("Укажите номер телефона".into());
    }

    // Tear down any previous forwarder + connection before starting a new login.
    stop_forwarder(&fwd).await;
    {
        let mut guard = state.inner.lock().await;
        if let Some(old) = guard.take() {
            old.runner.abort();
        }
    }

    let (client, runner, updates) = spawn_client(api_id);
    let token = client
        .request_login_code(&phone, api_hash.trim())
        .await
        .map_err(|e| format!("Не удалось запросить код: {}", e))?;

    let mut guard = state.inner.lock().await;
    *guard = Some(TgConn {
        client,
        runner,
        updates: Some(updates),
        login: Some(token),
        password: None,
        authorized: false,
    });
    Ok(TgStatus { stage: "code_sent".into(), user: None })
}

/// Step 2 — submit the login code.
#[tauri::command]
pub async fn tg_sign_in(
    code: String,
    state: tauri::State<'_, TgState>,
) -> Result<TgStatus, String> {
    let code = code.trim().to_string();
    let mut guard = state.inner.lock().await;
    let conn = guard.as_mut().ok_or("Нет активного подключения — запросите код заново")?;
    let token = conn.login.take().ok_or("Код не запрашивался")?;
    let client = conn.client.clone();

    match client.sign_in(&token, &code).await {
        Ok(user) => {
            conn.authorized = true;
            Ok(TgStatus { stage: "authorized".into(), user: Some(user.full_name()) })
        }
        Err(SignInError::PasswordRequired(pt)) => {
            conn.password = Some(pt);
            Ok(TgStatus { stage: "password_required".into(), user: None })
        }
        Err(SignInError::InvalidCode) => {
            conn.login = Some(token); // let the user re-enter the code
            Err("Неверный код".into())
        }
        Err(e) => {
            conn.login = Some(token);
            Err(format!("Ошибка входа: {}", e))
        }
    }
}

/// Step 2b — submit the 2FA password when required.
#[tauri::command]
pub async fn tg_check_password(
    password: String,
    state: tauri::State<'_, TgState>,
) -> Result<TgStatus, String> {
    let mut guard = state.inner.lock().await;
    let conn = guard.as_mut().ok_or("Нет активного подключения")?;
    let token = conn.password.take().ok_or("Пароль не требуется")?;
    let client = conn.client.clone();

    match client.check_password(token, password.as_bytes()).await {
        Ok(user) => {
            conn.authorized = true;
            Ok(TgStatus { stage: "authorized".into(), user: Some(user.full_name()) })
        }
        Err(e) => Err(format!("Неверный пароль: {}", e)),
    }
}

/// List the channels (and supergroups) the connected account belongs to.
#[tauri::command]
pub async fn tg_list_channels(
    state: tauri::State<'_, TgState>,
) -> Result<Vec<TgChannel>, String> {
    let client = {
        let guard = state.inner.lock().await;
        let conn = guard.as_ref().ok_or("Аккаунт Telegram не подключён")?;
        if !conn.authorized {
            return Err("Сначала войдите в аккаунт".into());
        }
        conn.client.clone()
    };

    let mut out = Vec::new();
    let mut dialogs = client.iter_dialogs();
    while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
        if let Peer::Channel(ch) = dialog.peer() {
            out.push(TgChannel {
                id: ch.id().to_string(),
                title: ch.title().to_string(),
                username: ch.username().map(|s| s.to_string()),
            });
        }
    }
    Ok(out)
}

/// Current connection state (used by the UI on load).
#[tauri::command]
pub async fn tg_status(state: tauri::State<'_, TgState>) -> Result<TgStatus, String> {
    let guard = state.inner.lock().await;
    match guard.as_ref() {
        Some(conn) if conn.authorized => Ok(TgStatus { stage: "authorized".into(), user: None }),
        Some(conn) if conn.password.is_some() => Ok(TgStatus { stage: "password_required".into(), user: None }),
        Some(_) => Ok(TgStatus { stage: "code_sent".into(), user: None }),
        None => Ok(TgStatus { stage: "disconnected".into(), user: None }),
    }
}

/// Disconnect, stop forwarding, and drop the session.
#[tauri::command]
pub async fn tg_logout(
    state: tauri::State<'_, TgState>,
    fwd: tauri::State<'_, ForwardState>,
) -> Result<(), String> {
    stop_forwarder(&fwd).await;
    let mut guard = state.inner.lock().await;
    if let Some(conn) = guard.take() {
        conn.runner.abort();
    }
    Ok(())
}

/// Start (or retarget) forwarding: posts in `tg_channel_id` are sent to `discord_channel_id`.
#[tauri::command]
pub async fn tg_start_forward(
    app: tauri::AppHandle,
    tg_channel_id: String,
    discord_channel_id: String,
    state: tauri::State<'_, TgState>,
    fwd: tauri::State<'_, ForwardState>,
) -> Result<(), String> {
    if tg_channel_id.trim().is_empty() || discord_channel_id.trim().is_empty() {
        return Err("Выберите канал Telegram и канал Discord".into());
    }
    *fwd.config.lock().await = Some(ForwardCfg {
        tg_source_id: tg_channel_id.trim().to_string(),
        discord_target_id: discord_channel_id.trim().to_string(),
    });

    // Spawn the update loop once; retargeting afterwards only rewrites the config.
    let mut task_guard = fwd.task.lock().await;
    if task_guard.as_ref().map_or(true, |h| h.is_finished()) {
        let (client, updates) = {
            let mut guard = state.inner.lock().await;
            let conn = guard.as_mut().ok_or("Telegram не подключён")?;
            if !conn.authorized {
                return Err("Сначала войдите в Telegram".into());
            }
            let rx = conn.updates.take().ok_or("Поток обновлений недоступен — переподключите Telegram")?;
            (conn.client.clone(), rx)
        };
        let config = fwd.config.clone();
        let app2 = app.clone();
        let handle = tokio::spawn(async move {
            forward_loop(app2, client, updates, config).await;
        });
        *task_guard = Some(handle);
    }
    Ok(())
}

/// Stop forwarding (keeps the Telegram session; you can start again without re-login).
#[tauri::command]
pub async fn tg_stop_forward(fwd: tauri::State<'_, ForwardState>) -> Result<(), String> {
    *fwd.config.lock().await = None;
    Ok(())
}

/// Report the current forwarding target (for the UI).
#[tauri::command]
pub async fn tg_forward_status(fwd: tauri::State<'_, ForwardState>) -> Result<ForwardStatus, String> {
    let cfg = fwd.config.lock().await.clone();
    Ok(match cfg {
        Some(c) => ForwardStatus {
            active: true,
            tg_source_id: Some(c.tg_source_id),
            discord_target_id: Some(c.discord_target_id),
        },
        None => ForwardStatus { active: false, tg_source_id: None, discord_target_id: None },
    })
}

/// The long-lived forwarding loop. Consumes the Telegram update stream and, whenever a new
/// message lands in the configured source channel, relays its text to the Discord target.
async fn forward_loop(
    app: tauri::AppHandle,
    client: Client,
    updates: UpdatesRx,
    config: Arc<Mutex<Option<ForwardCfg>>>,
) {
    let mut stream = match client.stream_updates(updates, UpdatesConfiguration::default()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("TG update stream failed: {}", e);
            return;
        }
    };

    loop {
        let update = match stream.next().await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("TG update stream ended: {}", e);
                break;
            }
        };

        let Update::NewMessage(msg) = update else { continue };
        let cfg = match config.lock().await.clone() {
            Some(c) => c,
            None => continue, // forwarding paused
        };
        let Some(peer) = msg.peer() else { continue };
        if peer.id().to_string() != cfg.tg_source_id {
            continue;
        }
        let text = msg.text();
        if text.is_empty() {
            continue; // media-only posts are skipped in v1
        }
        let ds = app.state::<crate::discord::DiscordState>();
        if let Err(e) = crate::discord::send_message(ds.inner(), &cfg.discord_target_id, text).await {
            tracing::warn!("Discord forward failed: {}", e);
        }
    }
}
