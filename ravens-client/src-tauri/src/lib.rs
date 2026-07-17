pub mod commands;
pub mod hwid;
pub mod config;
pub mod osint;

use commands::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get server URL from env or use default
    let server_url = std::env::var("RAVENS_SERVER_URL")
        .unwrap_or_else(|_| "https://69e0e937-387f-4aa3-9d15-8e9ffcd3b057-00-2i89nydyt7vcb.sisko.replit.dev:5000".to_string());

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
            commands::authenticate,
            commands::execute_action,
            commands::save_config,
            commands::load_config,
            commands::clear_config,
            commands::osint_scan,
            commands::osint_module,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
