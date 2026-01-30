#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod bridge;
mod commands;
mod devices;
mod logging;
mod performance;

use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::prelude::*;

use crate::bridge::{AppState, BridgeServer};
use crate::commands::*;
use crate::logging::{set_app_handle, TauriLogLayer};

#[tokio::main]
async fn main() {
    // Initialize tracing with both stdout formatting and Tauri event emission
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()) // Keep stdout logging
        .with(TauriLogLayer::new()) // Add Tauri event emission
        .init();

    info!("Starting HyperStudy Bridge");

    let app_state = Arc::new(AppState::new());
    let state_clone = app_state.clone();

    tauri::Builder::default()
        .setup(move |app| {
            let state = state_clone.clone();
            let app_handle = app.handle().clone();

            // Set the app handle for the logging layer to enable event emission
            set_app_handle(app_handle.clone());

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
            list_all_serial_ports_debug,
            list_ttl_devices,
            find_ttl_port_by_serial,
            discover_devices,
            get_device_metrics,
            get_system_diagnostics,
            load_configuration,
            save_configuration,
            get_performance_metrics,
            get_device_performance_metrics,
            get_performance_summary,
            check_ttl_latency_compliance,
            reset_performance_metrics,
            get_logs,
            export_logs,
            set_log_level,
            test_ttl_device,
            reset_device
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
