use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

const DEFAULT_PORT: u16 = 6767;

#[derive(Debug)]
pub struct KernelDevice {
    socket: Option<TcpStream>,
    ip_address: String,
    port: u16,
    status: DeviceStatus,
    config: DeviceConfig,
    buffer: Vec<u8>,
}

impl KernelDevice {
    pub fn new(ip_address: String) -> Self {
        Self {
            socket: None,
            ip_address,
            port: DEFAULT_PORT,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            buffer: Vec::with_capacity(4096),
        }
    }
}

#[async_trait]
impl Device for KernelDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting to Kernel Flow2 at {}:{}", self.ip_address, self.port);
        self.status = DeviceStatus::Connecting;

        let addr = format!("{}:{}", self.ip_address, self.port);

        match TcpStream::connect(&addr).await {
            Ok(socket) => {
                self.socket = Some(socket);
                self.status = DeviceStatus::Connected;
                info!("Successfully connected to Kernel Flow2");
                Ok(())
            }
            Err(e) => {
                self.status = DeviceStatus::Error;
                error!("Failed to connect to Kernel Flow2: {}", e);
                Err(DeviceError::ConnectionFailed(e.to_string()))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting from Kernel Flow2");

        if let Some(mut socket) = self.socket.take() {
            socket.shutdown().await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
        }

        self.status = DeviceStatus::Disconnected;
        self.buffer.clear();
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if let Some(ref mut socket) = self.socket {
            socket.write_all(data).await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
            socket.flush().await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if let Some(ref mut socket) = self.socket {
            self.buffer.clear();
            self.buffer.resize(4096, 0);

            match socket.read(&mut self.buffer).await {
                Ok(0) => {
                    self.status = DeviceStatus::Disconnected;
                    self.socket = None;
                    Err(DeviceError::ConnectionFailed("Connection closed by remote".to_string()))
                }
                Ok(n) => {
                    self.buffer.truncate(n);
                    Ok(self.buffer.clone())
                }
                Err(e) => Err(DeviceError::CommunicationError(e.to_string()))
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: format!("kernel_{}_{}", self.ip_address.replace('.', "_"), self.port),
            name: format!("Kernel Flow2 ({}:{})", self.ip_address, self.port),
            device_type: DeviceType::Kernel,
            status: self.status,
            metadata: serde_json::json!({
                "ip_address": self.ip_address,
                "port": self.port,
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.config = config;

        if let Some(custom) = self.config.custom_settings.as_object() {
            if let Some(ip) = custom.get("ip_address").and_then(|v| v.as_str()) {
                self.ip_address = ip.to_string();
            }

            if let Some(port) = custom.get("port").and_then(|v| v.as_u64()) {
                self.port = port as u16;
            }
        }

        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.socket.is_some() {
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}