#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod bridge;
mod commands;
mod devices;
mod logging;
mod performance;
mod storage;

use std::sync::Arc;
use tauri::Manager;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

use crate::bridge::{AppState, BridgeServer};
use crate::commands::*;
use crate::logging::{init_log_emitter, init_log_persister, set_app_handle, TauriLogLayer};

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

            // Initialize log emitter for batched frontend events
            init_log_emitter();

            // Initialize database storage
            let app_handle_for_db = app_handle.clone();
            tokio::spawn(async move {
                // Get app data directory for database
                let db_path = match app_handle_for_db.path().app_data_dir() {
                    Ok(dir) => dir.join("hyperstudy-bridge.db"),
                    Err(e) => {
                        error!("Failed to get app data directory: {}", e);
                        return;
                    }
                };

                // Initialize storage
                match storage::init_storage(&db_path).await {
                    Ok(storage) => {
                        info!("Database initialized at {:?}", db_path);

                        // Start a default session for logging
                        if let Err(e) = storage.start_session(None).await {
                            warn!("Failed to start default session: {}", e);
                        }

                        // Initialize log persister after storage is ready
                        init_log_persister();
                        info!("Log persistence enabled");
                    }
                    Err(e) => {
                        error!("Failed to initialize database: {}", e);
                        // App continues without persistence
                    }
                }
            });

            // Start WebSocket server
            let app_handle_for_ws = app_handle.clone();
            tokio::spawn(async move {
                let mut server = BridgeServer::new(state, app_handle_for_ws);
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
            query_logs,
            get_log_stats,
            get_storage_stats,
            start_session,
            end_session,
            list_sessions,
            cleanup_old_logs,
            export_logs,
            set_log_level,
            test_ttl_device,
            reset_device
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
