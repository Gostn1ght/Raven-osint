#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod hwid;
mod server;
mod config;

use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const SERVER_PORT: u16 = 8080;
const SERVER_BIN:  &str = if cfg!(windows) { "ravens-nexus-server.exe" } else { "ravens-nexus-server" };

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .setup(|app| {
            // Launch backend server if not already running
            let resource_path = app.path().resource_dir()
                .unwrap_or_default()
                .join(SERVER_BIN);
            tauri::async_runtime::spawn(async move {
                server::ensure_running(resource_path, SERVER_PORT).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_hwid,
            commands::api_get,
            commands::api_post,
            commands::save_token,
            commands::load_token,
            commands::clear_token,
            commands::server_health,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
