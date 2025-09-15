use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::measure_latency;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serialport::{self, SerialPort};
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

    pub fn list_ports() -> Result<Vec<String>, DeviceError> {
        let ports =
            serialport::available_ports().map_err(|e| DeviceError::SerialError(e.to_string()))?;

        Ok(ports.into_iter().map(|p| p.port_name).collect())
    }

    async fn send_pulse(&mut self) -> Result<(), DeviceError> {
        if let Some(ref port_mutex) = self.port {
            let device_id = self.get_info().id;
            let (result, latency) = measure_latency::<_, (), DeviceError>(async {
                let mut port = port_mutex.lock().unwrap();
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

        let port = serialport::new(&self.port_name, self.config.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| {
                self.status = DeviceStatus::Error;
                DeviceError::ConnectionFailed(format!("Failed to open serial port: {}", e))
            })?;

        self.port = Some(Mutex::new(port));
        self.status = DeviceStatus::Connected;

        info!("Successfully connected to TTL device");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting TTL device");

        if let Some(port_mutex) = self.port.take() {
            let mut port = port_mutex.lock().unwrap();
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
                let mut port = port_mutex.lock().unwrap();
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
                let mut port = port_mutex.lock().unwrap();
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
