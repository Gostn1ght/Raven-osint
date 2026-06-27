//! Manages lifecycle of the bundled ravens-nexus-server binary.

use std::path::PathBuf;
use std::time::Duration;
use reqwest::Client;

const HEALTH_URL: &str = "http://127.0.0.1:8080/health";
const MAX_WAIT_SECS: u64 = 20;

/// Checks if server is already responding; if not, spawns it.
pub async fn ensure_running(binary: PathBuf, _port: u16) {
    if is_healthy().await {
        tracing::info!("Server already running");
        return;
    }

    if !binary.exists() {
        tracing::warn!("Server binary not found at {:?} — skipping launch", binary);
        return;
    }

    tracing::info!("Launching server at {:?}", binary);
    let mut cmd = std::process::Command::new(&binary);
    cmd.env("PORT", "8080");

    // Detach — Tauri will send SIGTERM on exit via graceful shutdown
    #[cfg(unix)]
    { use std::os::unix::process::CommandExt; cmd.process_group(0); }

    match cmd.spawn() {
        Ok(_child) => {
            wait_healthy().await;
        }
        Err(e) => {
            tracing::error!("Failed to start server: {}", e);
        }
    }
}

async fn is_healthy() -> bool {
    Client::new().get(HEALTH_URL).timeout(Duration::from_secs(2))
        .send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

async fn wait_healthy() {
    let deadline = std::time::Instant::now() + Duration::from_secs(MAX_WAIT_SECS);
    while std::time::Instant::now() < deadline {
        if is_healthy().await {
            tracing::info!("Server is healthy");
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    tracing::warn!("Server did not become healthy within {}s", MAX_WAIT_SECS);
}
