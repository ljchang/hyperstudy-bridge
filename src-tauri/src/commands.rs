use crate::bridge::{AppState, BridgeServer};
use crate::devices::{Device, DeviceError, DeviceInfo, DeviceStatus, DeviceConfig};
use crate::devices::{ttl::TtlDevice, kernel::KernelDevice, pupil::PupilDevice, biopac::BiopacDevice, mock::MockDevice};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Emitter};
use tokio::sync::RwLock;
use tracing::{info, error, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> CommandResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub running: bool,
    pub port: u16,
    pub connected_clients: usize,
    pub device_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

// Bridge server management commands

#[tauri::command]
pub async fn start_bridge_server(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<String>, ()> {
    info!("Starting bridge server...");

    let state_clone = state.inner().clone();
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let mut server = BridgeServer::new(state_clone, app_handle_clone);
        if let Err(e) = server.start().await {
            error!("Bridge server error: {}", e);
        }
    });

    Ok(CommandResult::success("Bridge server started on port 9000".to_string()))
}

#[tauri::command]
pub async fn stop_bridge_server(
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<String>, ()> {
    info!("Stopping bridge server...");

    // Disconnect all devices
    let devices = state.devices.write().await;
    for (_id, device) in devices.iter() {
        let mut device = device.write().await;
        let _ = device.disconnect().await;
    }
    drop(devices);

    // Clear all connections
    state.connections.clear();

    Ok(CommandResult::success("Bridge server stopped".to_string()))
}

#[tauri::command]
pub async fn get_bridge_status(
    state: State<'_, Arc<AppState>>,
) -> Result<BridgeStatus, ()> {
    let devices = state.devices.read().await;

    Ok(BridgeStatus {
        running: true, // TODO: Track actual server state
        port: 9000,
        connected_clients: state.connections.len(),
        device_count: devices.len(),
    })
}

// Device management commands

#[tauri::command]
pub async fn connect_device(
    device_type: String,
    config: serde_json::Value,
    state: State<'_, Arc<AppState>>,
    app_handle: AppHandle,
) -> Result<CommandResult<DeviceInfo>, ()> {
    info!("Connecting device: {} with config: {:?}", device_type, config);

    let mut device: Box<dyn Device> = match device_type.as_str() {
        "ttl" => {
            let port = config.get("port")
                .and_then(|v| v.as_str())
                .unwrap_or("/dev/ttyUSB0");
            let mut ttl_device = TtlDevice::new(port.to_string());

            // Set up performance monitoring callback
            let state_clone = state.inner().clone();
            ttl_device.set_performance_callback(move |device_id, latency, bytes_sent, bytes_received| {
                let state_clone = state_clone.clone();
                let device_id = device_id.to_string();
                tokio::spawn(async move {
                    state_clone.record_device_operation(&device_id, latency, bytes_sent, bytes_received).await;
                });
            });

            Box::new(ttl_device)
        }
        "kernel" => {
            let ip = config.get("ip")
                .and_then(|v| v.as_str())
                .unwrap_or("192.168.1.100");
            Box::new(KernelDevice::new(ip.to_string()))
        }
        "pupil" => {
            let url = config.get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost:8081");
            Box::new(PupilDevice::new(url.to_string()))
        }
        "biopac" => {
            let address = config.get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost");
            Box::new(BiopacDevice::new(address.to_string()))
        }
        "mock" => {
            Box::new(MockDevice::new(
                format!("mock_{}", uuid::Uuid::new_v4()),
                "Mock Device".to_string()
            ))
        }
        _ => {
            return Ok(CommandResult::error(format!("Unknown device type: {}", device_type)));
        }
    };

    let device_id = device.get_info().id.clone();

    match device.connect().await {
        Ok(_) => {
            let info = device.get_info();

            // Record successful connection attempt
            state.record_connection_attempt(&device_id, true).await;

            state.add_device(device_id.clone(), device).await;

            // Emit status update
            app_handle.emit("device_status_changed", json!({
                "device": device_id,
                "status": "Connected"
            })).unwrap_or_else(|e| error!("Failed to emit event: {}", e));

            Ok(CommandResult::success(info))
        }
        Err(e) => {
            error!("Failed to connect device: {}", e);

            // Record failed connection attempt
            state.record_connection_attempt(&device_id, false).await;
            state.record_device_error(&device_id, &e.to_string()).await;

            Ok(CommandResult::error(e.to_string()))
        }
    }
}

#[tauri::command]
pub async fn disconnect_device(
    device_id: String,
    state: State<'_, Arc<AppState>>,
    app_handle: AppHandle,
) -> Result<CommandResult<String>, ()> {
    info!("Disconnecting device: {}", device_id);

    if let Some(device_lock) = state.get_device(&device_id).await {
        let mut device = device_lock.write().await;
        match device.disconnect().await {
            Ok(_) => {
                drop(device);
                state.remove_device(&device_id).await;

                // Emit status update
                app_handle.emit("device_status_changed", json!({
                    "device": device_id,
                    "status": "Disconnected"
                })).unwrap_or_else(|e| error!("Failed to emit event: {}", e));

                Ok(CommandResult::success(format!("Device {} disconnected", device_id)))
            }
            Err(e) => {
                error!("Failed to disconnect device: {}", e);
                Ok(CommandResult::error(e.to_string()))
            }
        }
    } else {
        Ok(CommandResult::error(format!("Device {} not found", device_id)))
    }
}

#[tauri::command]
pub async fn send_device_command(
    device_id: String,
    command: String,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<String>, ()> {
    info!("Sending command to device {}: {}", device_id, command);

    if let Some(device_lock) = state.get_device(&device_id).await {
        let mut device = device_lock.write().await;
        match device.send(command.as_bytes()).await {
            Ok(_) => Ok(CommandResult::success("Command sent".to_string())),
            Err(e) => {
                error!("Failed to send command: {}", e);

                // Record device error
                state.record_device_error(&device_id, &e.to_string()).await;

                Ok(CommandResult::error(e.to_string()))
            }
        }
    } else {
        Ok(CommandResult::error(format!("Device {} not found", device_id)))
    }
}

// TTL-specific high-performance command
#[tauri::command]
pub async fn send_ttl_pulse(
    port: Option<String>,
    state: State<'_, Arc<AppState>>,
    app_handle: AppHandle,
) -> Result<CommandResult<u64>, ()> {
    let start_time = std::time::Instant::now();

    // Try to find existing TTL device or use provided port
    let device_id = "ttl".to_string();

    if let Some(device_lock) = state.get_device(&device_id).await {
        let mut device = device_lock.write().await;
        match device.send(b"PULSE\n").await {
            Ok(_) => {
                let latency_us = start_time.elapsed().as_micros() as u64;

                // Emit performance metric
                app_handle.emit("performance_metrics", json!({
                    "device": "ttl",
                    "operation": "pulse",
                    "latency_us": latency_us
                })).unwrap_or_else(|e| error!("Failed to emit event: {}", e));

                Ok(CommandResult::success(latency_us))
            }
            Err(e) => {
                error!("Failed to send TTL pulse: {}", e);

                // Record device error
                state.record_device_error(&device_id, &e.to_string()).await;

                Ok(CommandResult::error(e.to_string()))
            }
        }
    } else if let Some(port) = port {
        // Quick connect and pulse for lowest latency
        let mut device = TtlDevice::new(port);
        match device.connect().await {
            Ok(_) => {
                match device.send(b"PULSE\n").await {
                    Ok(_) => {
                        let latency_us = start_time.elapsed().as_micros() as u64;
                        let _ = device.disconnect().await;
                        Ok(CommandResult::success(latency_us))
                    }
                    Err(e) => {
                        let _ = device.disconnect().await;
                        Ok(CommandResult::error(e.to_string()))
                    }
                }
            }
            Err(e) => Ok(CommandResult::error(e.to_string()))
        }
    } else {
        Ok(CommandResult::error("TTL device not connected and no port specified".to_string()))
    }
}

#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, ()> {
    match serialport::available_ports() {
        Ok(ports) => {
            let port_info: Vec<SerialPortInfo> = ports
                .into_iter()
                .map(|p| {
                    let (manufacturer, product) = match p.port_type {
                        serialport::SerialPortType::UsbPort(info) => {
                            (info.manufacturer, info.product)
                        }
                        _ => (None, None),
                    };

                    SerialPortInfo {
                        name: p.port_name,
                        manufacturer,
                        product,
                    }
                })
                .collect();

            Ok(port_info)
        }
        Err(e) => {
            error!("Failed to list serial ports: {}", e);
            Ok(vec![])
        }
    }
}

#[tauri::command]
pub async fn discover_devices() -> Result<Vec<DeviceInfo>, ()> {
    let mut discovered = Vec::new();

    // Check for serial devices (TTL)
    if let Ok(ports) = serialport::available_ports() {
        for port in ports {
            if let serialport::SerialPortType::UsbPort(info) = port.port_type {
                if info.product.as_deref() == Some("RP2040") ||
                   info.manufacturer.as_deref() == Some("Adafruit") {
                    discovered.push(DeviceInfo {
                        id: "ttl".to_string(),
                        name: "TTL Pulse Generator".to_string(),
                        device_type: crate::devices::DeviceType::TTL,
                        status: DeviceStatus::Disconnected,
                        metadata: json!({
                            "port": port.port_name,
                            "manufacturer": info.manufacturer,
                            "product": info.product
                        }),
                    });
                }
            }
        }
    }

    // TODO: Implement network discovery for Kernel, Pupil, Biopac devices

    Ok(discovered)
}

#[tauri::command]
pub async fn get_device_metrics(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<serde_json::Value>, ()> {
    if let Some(metrics) = state.get_device_metrics(&device_id).await {
        Ok(CommandResult::success(json!(metrics)))
    } else {
        Ok(CommandResult::error(format!("No metrics available for device {}", device_id)))
    }
}

#[tauri::command]
pub async fn get_system_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, ()> {
    let devices = state.devices.read().await;
    let device_statuses: HashMap<String, String> = devices
        .iter()
        .map(|(id, device)| {
            let device = device.blocking_read();
            (id.clone(), format!("{:?}", device.get_status()))
        })
        .collect();

    Ok(json!({
        "devices": device_statuses,
        "connections": state.connections.len(),
        "uptime_seconds": state.get_uptime().as_secs(),
        "message_count": state.get_message_count().await,
        "last_error": state.get_last_error().await,
    }))
}

#[tauri::command]
pub async fn load_configuration() -> Result<serde_json::Value, ()> {
    // TODO: Load from config file
    Ok(json!({
        "auto_connect": false,
        "default_ttl_port": "/dev/ttyUSB0",
        "websocket_port": 9000,
        "log_level": "info"
    }))
}

#[tauri::command]
pub async fn save_configuration(config: serde_json::Value) -> Result<CommandResult<String>, ()> {
    // TODO: Save to config file
    info!("Saving configuration: {:?}", config);
    Ok(CommandResult::success("Configuration saved".to_string()))
}

// Performance monitoring commands

#[tauri::command]
pub async fn get_performance_metrics(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::performance::PerformanceMetrics, ()> {
    Ok(state.get_performance_metrics().await)
}

#[tauri::command]
pub async fn get_device_performance_metrics(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<crate::performance::DevicePerformanceMetrics>, ()> {
    if let Some(metrics) = state.get_device_performance_metrics(&device_id).await {
        Ok(CommandResult::success(metrics))
    } else {
        Ok(CommandResult::error(format!("No performance metrics available for device {}", device_id)))
    }
}

#[tauri::command]
pub async fn get_performance_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, ()> {
    Ok(state.get_performance_summary().await)
}

#[tauri::command]
pub async fn check_ttl_latency_compliance(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<bool>, ()> {
    if let Some(is_compliant) = state.check_ttl_latency_compliance(&device_id).await {
        Ok(CommandResult::success(is_compliant))
    } else {
        Ok(CommandResult::error(format!("No TTL latency data available for device {}", device_id)))
    }
}

#[tauri::command]
pub async fn reset_performance_metrics(
    device_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<CommandResult<String>, ()> {
    match device_id {
        Some(id) => {
            // Reset specific device metrics by removing and re-adding
            state.performance_monitor.remove_device(&id).await;
            state.performance_monitor.add_device(id.clone()).await;
            Ok(CommandResult::success(format!("Performance metrics reset for device {}", id)))
        }
        None => {
            // Create new performance monitor to reset all metrics
            *state.performance_monitor.device_counters.write().await = std::collections::HashMap::new();
            Ok(CommandResult::success("All performance metrics reset".to_string()))
        }
    }
}