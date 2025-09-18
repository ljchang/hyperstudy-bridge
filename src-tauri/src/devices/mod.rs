pub mod biopac;
pub mod kernel;
pub mod lsl;
pub mod mock;
pub mod pupil;
pub mod ttl;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Device not connected")]
    NotConnected,

    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serial port error: {0}")]
    SerialError(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DeviceType {
    TTL,
    Kernel,
    Pupil,
    Biopac,
    LSL,
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub auto_reconnect: bool,
    pub reconnect_interval_ms: u64,
    pub timeout_ms: u64,
    pub custom_settings: serde_json::Value,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            reconnect_interval_ms: 1000,
            timeout_ms: 5000,
            custom_settings: serde_json::Value::Null,
        }
    }
}

#[async_trait]
pub trait Device: Send + Sync + Debug {
    async fn connect(&mut self) -> Result<(), DeviceError>;

    async fn disconnect(&mut self) -> Result<(), DeviceError>;

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError>;

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError>;

    fn get_info(&self) -> DeviceInfo;

    fn get_status(&self) -> DeviceStatus;

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError>;

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }

    /// Test if the device can be reached without establishing a persistent connection
    async fn test_connection(&mut self) -> Result<bool, DeviceError> {
        // Default implementation: try to connect and disconnect
        match self.connect().await {
            Ok(_) => {
                let _ = self.disconnect().await;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Send a device-specific event (for Kernel, Pupil, etc.)
    async fn send_event(&mut self, event: serde_json::Value) -> Result<(), DeviceError> {
        // Default implementation: serialize and send as bytes
        let data = event.to_string();
        self.send(data.as_bytes()).await
    }
}

pub type BoxedDevice = Box<dyn Device + Send + Sync + 'static>;
