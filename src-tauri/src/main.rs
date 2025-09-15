#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod bridge;
mod devices;

use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber;

use crate::bridge::{BridgeServer, AppState};

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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
