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
mod usb_monitor;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::bridge::{AppState, BridgeServer};
use crate::commands::*;
use crate::logging::{init_log_emitter, init_log_persister, set_app_handle, TauriLogLayer};

#[tokio::main]
async fn main() {
    // Create filters to prevent log explosion from dependencies.
    // Without filtering, sqlx logs every INSERT statement, which triggers more logging,
    // causing an infinite feedback loop that fills the database and consumes all memory.
    //
    // Console filter: Show our logs at INFO+, dependencies at WARN+
    let console_filter =
        EnvFilter::new("warn").add_directive("hyperstudy_bridge=info".parse().unwrap());

    // Tauri layer filter: ONLY log our crate's messages to frontend/database
    // This completely prevents dependency logs from being stored
    let tauri_filter =
        EnvFilter::new("off").add_directive("hyperstudy_bridge=info".parse().unwrap());

    // Initialize tracing with filtering to prevent log explosion from dependencies
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(console_filter))
        .with(TauriLogLayer::new().with_filter(tauri_filter))
        .init();

    info!("Starting HyperStudy Bridge");

    let app_state = Arc::new(AppState::new());
    let state_clone = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_stronghold::Builder::new(|password| {
                use argon2::{hash_raw, Config, Variant, Version};
                let config = Config {
                    lanes: 4,
                    mem_cost: 10_000,
                    time_cost: 10,
                    variant: Variant::Argon2id,
                    version: Version::Version13,
                    ..Default::default()
                };
                let salt = b"hyperstudy-bridge-vault-salt";
                hash_raw(password.as_ref(), salt, &config)
                    .expect("failed to hash password")
                    .to_vec()
            })
            .build(),
        )
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
            let state_for_ws = state.clone();
            tokio::spawn(async move {
                let mut server = BridgeServer::new(state_for_ws, app_handle_for_ws);
                if let Err(e) = server.start().await {
                    error!("Failed to start WebSocket server: {}", e);
                }
            });

            // Start USB device monitoring for TTL disconnect detection
            let state_for_usb = state.clone();
            let app_handle_for_usb = app_handle.clone();
            tokio::spawn(async move {
                let mut usb_rx = usb_monitor::start_usb_monitor();
                info!("USB device monitoring started");

                while let Some(event) = usb_rx.recv().await {
                    if event.is_ttl_device() && event.is_disconnect() {
                        let port = event.port_name();
                        let serial = event.serial_number();
                        info!(
                            "TTL USB disconnect detected on port {} (S/N: {:?}), updating device status",
                            port, serial
                        );

                        // Handle the disconnect
                        let updated = state_for_usb.handle_ttl_usb_disconnect().await;

                        if updated {
                            // Emit Tauri event to notify frontend
                            if let Err(e) = app_handle_for_usb.emit("device_status_changed", serde_json::json!({
                                "device": "ttl",
                                "status": "Disconnected",
                                "reason": "USB device unplugged",
                                "port": port,
                                "serial_number": serial
                            })) {
                                warn!("Failed to emit device_status_changed event: {}", e);
                            }
                        }
                    }
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
            clear_all_logs,
            export_logs,
            set_log_level,
            test_ttl_device,
            reset_device,
            get_app_info,
            start_frenz_bridge,
            stop_frenz_bridge,
            get_frenz_bridge_status,
            check_frenz_bridge_available
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
