pub mod commands;
pub mod hwid;
pub mod config;

use commands::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Default backend URL (overridable in Settings → Server, or via RAVENS_SERVER_URL).
    let server_url = std::env::var("RAVENS_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new(server_url))
        .setup(|_app| {
            tracing::info!("Ravens Nexus secure client started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_hwid,
            commands::set_server_url,
            commands::authenticate,
            commands::osint_run,
            commands::save_config,
            commands::load_config,
            commands::clear_config,
            commands::minimize_window,
            commands::maximize_window,
            commands::close_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
