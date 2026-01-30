use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::measure_latency;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serialport::{self, SerialPort};
use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

const PULSE_COMMAND: &[u8] = b"PULSE\n";
const PULSE_DURATION_MS: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub pulse_duration_ms: u64,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            port_name: String::new(),
            baud_rate: 115200,
            pulse_duration_ms: PULSE_DURATION_MS,
        }
    }
}

/// Type alias for performance callback
type PerformanceCallback = Box<dyn Fn(&str, Duration, u64, u64) + Send + Sync>;

pub struct TtlDevice {
    port: Option<Mutex<Box<dyn SerialPort>>>,
    port_name: String,
    status: DeviceStatus,
    config: TtlConfig,
    device_config: DeviceConfig,
    /// Performance callback for recording metrics
    performance_callback: Option<PerformanceCallback>,
}

impl std::fmt::Debug for TtlDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TtlDevice")
            .field("port_name", &self.port_name)
            .field("status", &self.status)
            .field("config", &self.config)
            .field("device_config", &self.device_config)
            .field(
                "has_performance_callback",
                &self.performance_callback.is_some(),
            )
            .finish()
    }
}

impl TtlDevice {
    pub fn new(port_name: String) -> Self {
        Self {
            port: None,
            port_name: port_name.clone(),
            status: DeviceStatus::Disconnected,
            config: TtlConfig {
                port_name,
                ..Default::default()
            },
            device_config: DeviceConfig::default(),
            performance_callback: None,
        }
    }

    /// Set performance callback for metrics recording
    pub fn set_performance_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, Duration, u64, u64) + Send + Sync + 'static,
    {
        self.performance_callback = Some(Box::new(callback));
    }

    // Adafruit RP2040 USB VID/PID
    const TTL_USB_VID: u16 = 0x239A;
    const TTL_USB_PID: u16 = 0x80F1;

    pub fn list_ports() -> Result<Vec<String>, DeviceError> {
        let ports =
            serialport::available_ports().map_err(|e| DeviceError::SerialError(e.to_string()))?;

        Ok(ports.into_iter().map(|p| p.port_name).collect())
    }

    /// Debug function to list ALL serial ports with detailed USB info
    pub fn list_all_ports_debug() -> Result<Vec<serde_json::Value>, DeviceError> {
        let ports =
            serialport::available_ports().map_err(|e| DeviceError::SerialError(e.to_string()))?;

        let mut all_ports = Vec::new();

        for port in ports {
            let port_info = match &port.port_type {
                serialport::SerialPortType::UsbPort(usb_info) => {
                    serde_json::json!({
                        "port": port.port_name,
                        "type": "USB",
                        "vid": format!("0x{:04X}", usb_info.vid),
                        "pid": format!("0x{:04X}", usb_info.pid),
                        "serial_number": usb_info.serial_number.as_ref().unwrap_or(&"None".to_string()),
                        "manufacturer": usb_info.manufacturer.as_ref().unwrap_or(&"None".to_string()),
                        "product": usb_info.product.as_ref().unwrap_or(&"None".to_string()),
                    })
                }
                serialport::SerialPortType::BluetoothPort => {
                    serde_json::json!({
                        "port": port.port_name,
                        "type": "Bluetooth",
                    })
                }
                serialport::SerialPortType::PciPort => {
                    serde_json::json!({
                        "port": port.port_name,
                        "type": "PCI",
                    })
                }
                serialport::SerialPortType::Unknown => {
                    serde_json::json!({
                        "port": port.port_name,
                        "type": "Unknown",
                    })
                }
            };
            all_ports.push(port_info);
        }

        info!("Found {} serial ports total", all_ports.len());
        Ok(all_ports)
    }

    /// List TTL devices by filtering on VID/PID and return detailed info
    /// Returns a JSON object with 'devices' array and 'autoSelected' port if only one device found
    pub fn list_ttl_devices() -> Result<serde_json::Value, DeviceError> {
        let ports =
            serialport::available_ports().map_err(|e| DeviceError::SerialError(e.to_string()))?;

        let mut ttl_devices = Vec::new();

        for port in ports {
            if let serialport::SerialPortType::UsbPort(usb_info) = &port.port_type {
                // Check if this is an Adafruit RP2040 (our TTL device)
                // On macOS, skip /dev/tty.* ports (duplicates of /dev/cu.*)
                if usb_info.vid == Self::TTL_USB_VID
                    && usb_info.pid == Self::TTL_USB_PID
                    && !port.port_name.starts_with("/dev/tty.")
                {
                    let device_info = serde_json::json!({
                        "port": port.port_name,
                        "serial_number": usb_info.serial_number.as_ref().unwrap_or(&"Unknown".to_string()),
                        "manufacturer": usb_info.manufacturer.as_ref().unwrap_or(&"Unknown".to_string()),
                        "product": usb_info.product.as_ref().unwrap_or(&"Unknown".to_string()),
                        "vid": format!("0x{:04X}", usb_info.vid),
                        "pid": format!("0x{:04X}", usb_info.pid),
                    });
                    ttl_devices.push(device_info);
                    info!(
                        "Found TTL device: {} (S/N: {})",
                        port.port_name,
                        usb_info
                            .serial_number
                            .as_ref()
                            .unwrap_or(&"Unknown".to_string())
                    );
                }
            }
        }

        let result = if ttl_devices.is_empty() {
            info!(
                "No TTL devices found (VID: 0x{:04X}, PID: 0x{:04X})",
                Self::TTL_USB_VID,
                Self::TTL_USB_PID
            );
            serde_json::json!({
                "devices": ttl_devices,
                "autoSelected": null,
                "count": 0
            })
        } else if ttl_devices.len() == 1 {
            let auto_port = ttl_devices[0]["port"].as_str().unwrap_or("");
            info!("Auto-selecting single TTL device: {}", auto_port);
            serde_json::json!({
                "devices": ttl_devices,
                "autoSelected": auto_port,
                "count": 1
            })
        } else {
            info!(
                "Found {} TTL devices - manual selection required",
                ttl_devices.len()
            );
            serde_json::json!({
                "devices": ttl_devices,
                "autoSelected": null,
                "count": ttl_devices.len()
            })
        };

        Ok(result)
    }

    /// Find a TTL device port by serial number
    pub fn find_port_by_serial(serial_number: &str) -> Result<Option<String>, DeviceError> {
        let ports =
            serialport::available_ports().map_err(|e| DeviceError::SerialError(e.to_string()))?;

        for port in ports {
            if let serialport::SerialPortType::UsbPort(usb_info) = &port.port_type {
                // Only check devices with matching VID/PID
                if usb_info.vid == Self::TTL_USB_VID && usb_info.pid == Self::TTL_USB_PID {
                    if let Some(ref sn) = usb_info.serial_number {
                        if sn == serial_number {
                            info!(
                                "Found TTL device with serial number {} at port {}",
                                serial_number, port.port_name
                            );
                            return Ok(Some(port.port_name));
                        }
                    }
                }
            }
        }

        info!("No TTL device found with serial number: {}", serial_number);
        Ok(None)
    }

    async fn send_pulse(&mut self) -> Result<(), DeviceError> {
        if let Some(ref port_mutex) = self.port {
            let device_id = self.get_info().id;
            let (result, latency) = measure_latency::<_, (), DeviceError>(async {
                let mut port = port_mutex
                    .lock()
                    .map_err(|_| DeviceError::CommunicationError("Mutex poisoned".to_string()))?;
                port.write_all(PULSE_COMMAND)
                    .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

                port.flush()
                    .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

                Ok(())
            })
            .await;

            // Record performance metrics
            if let Some(ref callback) = self.performance_callback {
                callback(&device_id, latency, PULSE_COMMAND.len() as u64, 0);
            }

            debug!("TTL pulse sent with latency: {:?}", latency);

            // Check for compliance with <1ms requirement
            if latency > Duration::from_millis(1) {
                warn!(
                    "TTL pulse latency exceeded 1ms: {:?} - Performance requirement not met!",
                    latency
                );
            } else if latency > Duration::from_micros(500) {
                warn!("TTL pulse latency approaching limit: {:?}", latency);
            }

            result?;

            sleep(Duration::from_millis(self.config.pulse_duration_ms)).await;

            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}

#[async_trait]
impl Device for TtlDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting to TTL device on port: {}", self.port_name);

        self.status = DeviceStatus::Connecting;

        let mut port = serialport::new(&self.port_name, self.config.baud_rate)
            .timeout(Duration::from_millis(500))
            .open()
            .map_err(|e| {
                self.status = DeviceStatus::Error;
                DeviceError::ConnectionFailed(format!("Failed to open serial port: {}", e))
            })?;

        // Validate connection by sending TEST command
        info!("Validating TTL device connection...");
        port.write_all(b"TEST\n").map_err(|e| {
            self.status = DeviceStatus::Error;
            DeviceError::ConnectionFailed(format!("Failed to send TEST command: {}", e))
        })?;

        port.flush().map_err(|e| {
            self.status = DeviceStatus::Error;
            DeviceError::ConnectionFailed(format!("Failed to flush: {}", e))
        })?;

        // Small delay for device to respond
        sleep(Duration::from_millis(100)).await;

        // Read response
        let mut buffer = vec![0u8; 256];
        match port.read(&mut buffer) {
            Ok(bytes_read) => {
                buffer.truncate(bytes_read);
                let response = String::from_utf8_lossy(&buffer).trim().to_string();
                if response.is_empty() {
                    self.status = DeviceStatus::Error;
                    return Err(DeviceError::ConnectionFailed(
                        "Device did not respond to TEST command. Is this the correct device?".to_string()
                    ));
                }
                info!("TTL device validated. Response: {}", response);
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                self.status = DeviceStatus::Error;
                return Err(DeviceError::ConnectionFailed(
                    "Device timeout - no response to TEST command. Check device connection.".to_string()
                ));
            }
            Err(e) => {
                self.status = DeviceStatus::Error;
                return Err(DeviceError::ConnectionFailed(format!(
                    "Failed to read validation response: {}", e
                )));
            }
        }

        // Reset timeout to normal operation value
        port.set_timeout(Duration::from_millis(100)).map_err(|e| {
            warn!("Failed to reset port timeout: {}", e);
            DeviceError::ConnectionFailed(format!("Failed to configure port: {}", e))
        })?;

        self.port = Some(Mutex::new(port));
        self.status = DeviceStatus::Connected;

        info!("Successfully connected to TTL device");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting TTL device");

        if let Some(port_mutex) = self.port.take() {
            let mut port = port_mutex
                .lock()
                .map_err(|_| DeviceError::CommunicationError("Mutex poisoned".to_string()))?;
            port.flush()
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
        }

        self.status = DeviceStatus::Disconnected;
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if data == PULSE_COMMAND || data == b"PULSE" {
            self.send_pulse().await
        } else if let Some(ref port_mutex) = self.port {
            let device_id = self.get_info().id;
            let (result, latency) = measure_latency::<_, (), DeviceError>(async {
                let mut port = port_mutex
                    .lock()
                    .map_err(|_| DeviceError::CommunicationError("Mutex poisoned".to_string()))?;
                port.write_all(data)
                    .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
                port.flush()
                    .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
                Ok(())
            })
            .await;

            // Record performance metrics
            if let Some(ref callback) = self.performance_callback {
                callback(&device_id, latency, data.len() as u64, 0);
            }

            result
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if let Some(ref port_mutex) = self.port {
            let device_id = self.get_info().id;
            let (result, latency) = measure_latency::<_, Vec<u8>, DeviceError>(async {
                let mut buffer = vec![0u8; 256];
                let mut port = port_mutex
                    .lock()
                    .map_err(|_| DeviceError::CommunicationError("Mutex poisoned".to_string()))?;
                match port.read(&mut buffer) {
                    Ok(bytes_read) => {
                        buffer.truncate(bytes_read);
                        Ok(buffer)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(Vec::new()),
                    Err(e) => Err(DeviceError::CommunicationError(e.to_string())),
                }
            })
            .await;

            // Record performance metrics
            if let Ok(ref data) = result {
                if let Some(ref callback) = self.performance_callback {
                    callback(&device_id, latency, 0, data.len() as u64);
                }
            }

            result
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: format!("ttl_{}", self.port_name.replace('/', "_")),
            name: format!("TTL Pulse Generator ({})", self.port_name),
            device_type: DeviceType::TTL,
            status: self.status,
            metadata: serde_json::json!({
                "port": self.port_name,
                "baud_rate": self.config.baud_rate,
                "pulse_duration_ms": self.config.pulse_duration_ms,
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.device_config = config;

        if let Some(custom) = self.device_config.custom_settings.as_object() {
            if let Some(port) = custom.get("port_name").and_then(|v| v.as_str()) {
                self.config.port_name = port.to_string();
                self.port_name = port.to_string();
            }

            if let Some(baud) = custom.get("baud_rate").and_then(|v| v.as_u64()) {
                self.config.baud_rate = baud as u32;
            }

            if let Some(duration) = custom.get("pulse_duration_ms").and_then(|v| v.as_u64()) {
                self.config.pulse_duration_ms = duration;
            }
        }

        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.port.is_some() {
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}

#[cfg(test)]
#[path = "ttl_tests.rs"]
mod ttl_tests;
