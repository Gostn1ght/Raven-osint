pub mod commands;
pub mod hwid;
pub mod config;
pub mod osint;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .setup(|_app| {
            tracing::info!("Ravens Nexus standalone started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_hwid,
            commands::save_config,
            commands::load_config,
            commands::clear_config,
            commands::osint_scan,
            commands::osint_module,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
