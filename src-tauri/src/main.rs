#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod bridge;
mod devices;
mod commands;

use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber;

use crate::bridge::{BridgeServer, AppState};
use crate::commands::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting HyperStudy Bridge");

    let app_state = Arc::new(AppState::new());
    let state_clone = app_state.clone();

    tauri::Builder::default()
        .setup(move |app| {
            let state = state_clone.clone();
            let app_handle = app.handle().clone();

            tokio::spawn(async move {
                let mut server = BridgeServer::new(state, app_handle);
                if let Err(e) = server.start().await {
                    error!("Failed to start WebSocket server: {}", e);
                }
            });

            Ok(())
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            start_bridge_server,
            stop_bridge_server,
            get_bridge_status,
            connect_device,
            disconnect_device,
            send_device_command,
            send_ttl_pulse,
            list_serial_ports,
            discover_devices,
            get_device_metrics,
            get_system_diagnostics,
            load_configuration,
            save_configuration
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
