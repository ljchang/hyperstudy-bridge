use crate::bridge::{AppState, BridgeCommand, BridgeResponse, MessageHandler};
use crate::bridge::message::{CommandAction, QueryTarget};
use crate::devices::{mock::MockDevice, ttl::TtlDevice, kernel::KernelDevice,
                      pupil::PupilDevice, biopac::BiopacDevice};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn, debug};
use uuid::Uuid;

const WS_PORT: u16 = 9000;
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

pub struct BridgeServer {
    state: Arc<AppState>,
    app_handle: AppHandle,
}

impl BridgeServer {
    pub fn new(state: Arc<AppState>, app_handle: AppHandle) -> Self {
        Self { state, app_handle }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = ([127, 0, 0, 1], WS_PORT).into();
        let listener = TcpListener::bind(&addr).await?;

        info!("WebSocket server listening on ws://{}", addr);

        while let Ok((stream, peer_addr)) = listener.accept().await {
            info!("New connection from: {}", peer_addr);

            let state = self.state.clone();
            let app_handle = self.app_handle.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, peer_addr, state, app_handle).await {
                    error!("Error handling connection from {}: {}", peer_addr, e);
                }
            });
        }

        Ok(())
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    state: Arc<AppState>,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    let connection_id = Uuid::new_v4().to_string();

    state.add_connection(connection_id.clone(), peer_addr.to_string());

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel::<BridgeResponse>(100);

    let state_clone = state.clone();
    let connection_id_clone = connection_id.clone();

    let send_task = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            if let Ok(msg) = MessageHandler::serialize_response(&response) {
                if ws_sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        }
    });

    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    state_clone.update_connection_activity(&connection_id_clone);
                    info!("Received WebSocket message: {}", text);

                    match MessageHandler::parse_command(&text) {
                        Ok(command) => {
                            info!("Parsed command successfully");
                            handle_command(
                                command,
                                &state_clone,
                                &tx,
                                &app_handle,
                            ).await;
                        }
                        Err(e) => {
                            warn!("Failed to parse command: {}", e);
                            let _ = tx.send(BridgeResponse::error(e)).await;
                        }
                    }
                }
                Ok(Message::Binary(_bin)) => {
                    warn!("Binary messages not supported");
                    let _ = tx.send(BridgeResponse::error(
                        "Binary messages not supported".to_string()
                    )).await;
                }
                Ok(Message::Ping(_data)) => {
                    debug!("Received ping, sending pong");
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong");
                }
                Ok(Message::Close(_)) => {
                    info!("Client {} disconnected", peer_addr);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = receive_task => {},
    }

    state.remove_connection(&connection_id);
    info!("Connection {} closed", connection_id);

    Ok(())
}

async fn handle_command(
    command: BridgeCommand,
    state: &Arc<AppState>,
    tx: &mpsc::Sender<BridgeResponse>,
    _app_handle: &AppHandle,
) {
    match command {
        BridgeCommand::Command { device, action, payload, id } => {
            handle_device_command(state, device, action, payload, id, tx).await;
        }
        BridgeCommand::Query { target, id } => {
            handle_query(state, target, id, tx).await;
        }
        BridgeCommand::Subscribe { device, events } => {
            let _ = tx.send(BridgeResponse::event(
                device,
                "subscribed".to_string(),
                json!({ "events": events })
            )).await;
        }
        BridgeCommand::Unsubscribe { device, events } => {
            let _ = tx.send(BridgeResponse::event(
                device,
                "unsubscribed".to_string(),
                json!({ "events": events })
            )).await;
        }
    }
}

async fn handle_device_command(
    state: &Arc<AppState>,
    device_id: String,
    action: CommandAction,
    payload: Option<serde_json::Value>,
    id: Option<String>,
    tx: &mpsc::Sender<BridgeResponse>,
) {
    info!("Handling device command: device={}, action={:?}", device_id, action);
    match action {
        CommandAction::Connect => {
            info!("Processing connect for device: {}", device_id);
            let device_type = MessageHandler::validate_device_type(&device_id);

            if device_type.is_none() {
                warn!("Invalid device type: {}", device_id);
                let _ = tx.send(BridgeResponse::device_error(
                    device_id,
                    "Invalid device type".to_string()
                )).await;
                return;
            }

            let config = if let Some(p) = payload {
                p
            } else {
                json!({})
            };

            let mut device: Box<dyn crate::devices::Device> = match device_id.as_str() {
                "ttl" => {
                    let port = config.get("port")
                        .and_then(|v| v.as_str())
                        .unwrap_or("/dev/ttyUSB0");
                    Box::new(TtlDevice::new(port.to_string()))
                }
                "kernel" => {
                    let ip = config.get("ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("127.0.0.1");
                    Box::new(KernelDevice::new(ip.to_string()))
                }
                "pupil" => {
                    let url = config.get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("localhost:8081");
                    Box::new(PupilDevice::new(url.to_string()))
                }
                "biopac" => {
                    let addr = config.get("address")
                        .and_then(|v| v.as_str())
                        .unwrap_or("localhost");
                    Box::new(BiopacDevice::new(addr.to_string()))
                }
                "mock" => {
                    Box::new(MockDevice::new(
                        format!("mock_{}", Uuid::new_v4()),
                        "Mock Device".to_string()
                    ))
                }
                _ => {
                    let _ = tx.send(BridgeResponse::device_error(
                        device_id,
                        "Unsupported device type".to_string()
                    )).await;
                    return;
                }
            };

            match device.connect().await {
                Ok(_) => {
                    let status = device.get_status();
                    state.add_device(device_id.clone(), device).await;

                    let _ = tx.send(BridgeResponse::status(device_id.clone(), status)).await;

                    if let Some(req_id) = id {
                        let _ = tx.send(BridgeResponse::ack(
                            req_id,
                            true,
                            Some("Device connected".to_string())
                        )).await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(BridgeResponse::device_error(
                        device_id.clone(),
                        e.to_string()
                    )).await;

                    if let Some(req_id) = id {
                        let _ = tx.send(BridgeResponse::ack(
                            req_id,
                            false,
                            Some(e.to_string())
                        )).await;
                    }
                }
            }
        }
        CommandAction::Disconnect => {
            if let Some(device_lock) = state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                match device.disconnect().await {
                    Ok(_) => {
                        drop(device);
                        state.remove_device(&device_id).await;

                        let _ = tx.send(BridgeResponse::status(
                            device_id,
                            crate::devices::DeviceStatus::Disconnected
                        )).await;

                        if let Some(req_id) = id {
                            let _ = tx.send(BridgeResponse::ack(
                                req_id,
                                true,
                                Some("Device disconnected".to_string())
                            )).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(BridgeResponse::device_error(
                            device_id,
                            e.to_string()
                        )).await;

                        if let Some(req_id) = id {
                            let _ = tx.send(BridgeResponse::ack(
                                req_id,
                                false,
                                Some(e.to_string())
                            )).await;
                        }
                    }
                }
            } else {
                let _ = tx.send(BridgeResponse::device_error(
                    device_id,
                    "Device not found".to_string()
                )).await;

                if let Some(req_id) = id {
                    let _ = tx.send(BridgeResponse::ack(
                        req_id,
                        false,
                        Some("Device not found".to_string())
                    )).await;
                }
            }
        }
        CommandAction::Send => {
            if let Some(device_lock) = state.get_device(&device_id).await {
                let mut device = device_lock.write().await;

                let data = if let Some(p) = payload {
                    if let Some(cmd) = p.get("command").and_then(|v| v.as_str()) {
                        cmd.as_bytes().to_vec()
                    } else if let Some(data) = p.get("data").and_then(|v| v.as_str()) {
                        data.as_bytes().to_vec()
                    } else {
                        p.to_string().as_bytes().to_vec()
                    }
                } else {
                    Vec::new()
                };

                match device.send(&data).await {
                    Ok(_) => {
                        if let Some(req_id) = id {
                            let _ = tx.send(BridgeResponse::ack(
                                req_id,
                                true,
                                Some("Data sent".to_string())
                            )).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(BridgeResponse::device_error(
                            device_id,
                            e.to_string()
                        )).await;

                        if let Some(req_id) = id {
                            let _ = tx.send(BridgeResponse::ack(
                                req_id,
                                false,
                                Some(e.to_string())
                            )).await;
                        }
                    }
                }
            } else {
                let _ = tx.send(BridgeResponse::device_error(
                    device_id,
                    "Device not found".to_string()
                )).await;

                if let Some(req_id) = id {
                    let _ = tx.send(BridgeResponse::ack(
                        req_id,
                        false,
                        Some("Device not found".to_string())
                    )).await;
                }
            }
        }
        CommandAction::Status => {
            if let Some(status) = state.get_device_status(&device_id).await {
                let _ = tx.send(BridgeResponse::status(device_id, status)).await;

                if let Some(req_id) = id {
                    let _ = tx.send(BridgeResponse::ack(
                        req_id,
                        true,
                        None
                    )).await;
                }
            } else {
                let _ = tx.send(BridgeResponse::device_error(
                    device_id,
                    "Device not found".to_string()
                )).await;

                if let Some(req_id) = id {
                    let _ = tx.send(BridgeResponse::ack(
                        req_id,
                        false,
                        Some("Device not found".to_string())
                    )).await;
                }
            }
        }
        _ => {
            let _ = tx.send(BridgeResponse::error(
                "Unsupported action".to_string()
            )).await;

            if let Some(req_id) = id {
                let _ = tx.send(BridgeResponse::ack(
                    req_id,
                    false,
                    Some("Unsupported action".to_string())
                )).await;
            }
        }
    }
}

async fn handle_query(
    state: &Arc<AppState>,
    target: QueryTarget,
    id: Option<String>,
    tx: &mpsc::Sender<BridgeResponse>,
) {
    let data = match target {
        QueryTarget::Devices => {
            let devices = state.list_devices().await;
            json!(devices)
        }
        QueryTarget::Device(device_id) => {
            if let Some(device_lock) = state.get_device(&device_id).await {
                let device = device_lock.read().await;
                json!(device.get_info())
            } else {
                json!({ "error": "Device not found" })
            }
        }
        QueryTarget::Metrics => {
            let metrics = state.get_metrics().await;
            json!(metrics)
        }
        QueryTarget::Connections => {
            let connections: Vec<_> = state.connections
                .iter()
                .map(|entry| entry.value().clone())
                .collect();
            json!(connections)
        }
        QueryTarget::Status => {
            let devices = state.devices.read().await;
            json!({
                "server": "running",
                "port": WS_PORT,
                "devices": devices.len(),
                "connections": state.connections.len(),
            })
        }
    };

    let _ = tx.send(BridgeResponse::query_result(id, data)).await;
}