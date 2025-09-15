use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use tracing::{error, info, debug};

const DISCOVERY_PORT: u16 = 8080;
const WS_PORT: u16 = 8081;

#[derive(Debug)]
pub struct PupilDevice {
    ws_client: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    device_url: String,
    status: DeviceStatus,
    config: DeviceConfig,
    streaming: bool,
}

impl PupilDevice {
    pub fn new(device_url: String) -> Self {
        Self {
            ws_client: None,
            device_url,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            streaming: false,
        }
    }

    pub async fn discover_devices() -> Result<Vec<String>, DeviceError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl Device for PupilDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting to Pupil Labs Neon at {}", self.device_url);
        self.status = DeviceStatus::Connecting;

        let url = if !self.device_url.starts_with("ws://") && !self.device_url.starts_with("wss://") {
            format!("ws://{}", self.device_url)
        } else {
            self.device_url.clone()
        };

        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                self.ws_client = Some(ws_stream);
                self.status = DeviceStatus::Connected;
                info!("Successfully connected to Pupil Labs Neon");
                Ok(())
            }
            Err(e) => {
                self.status = DeviceStatus::Error;
                error!("Failed to connect to Pupil Labs Neon: {}", e);
                Err(DeviceError::WebSocketError(e.to_string()))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting from Pupil Labs Neon");

        if let Some(mut ws) = self.ws_client.take() {
            let _ = ws.close(None).await;
        }

        self.status = DeviceStatus::Disconnected;
        self.streaming = false;
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            let message = String::from_utf8_lossy(data);

            ws.send(Message::Text(message.to_string())).await
                .map_err(|e| DeviceError::WebSocketError(e.to_string()))?;

            debug!("Sent message to Pupil: {}", message);
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => {
                    debug!("Received text from Pupil: {}", text);
                    Ok(text.into_bytes())
                }
                Some(Ok(Message::Binary(data))) => {
                    debug!("Received {} bytes from Pupil", data.len());
                    Ok(data)
                }
                Some(Ok(Message::Close(_))) => {
                    self.status = DeviceStatus::Disconnected;
                    self.ws_client = None;
                    Err(DeviceError::ConnectionFailed("WebSocket closed".to_string()))
                }
                Some(Ok(_)) => Ok(Vec::new()),
                Some(Err(e)) => Err(DeviceError::WebSocketError(e.to_string())),
                None => {
                    self.status = DeviceStatus::Disconnected;
                    self.ws_client = None;
                    Err(DeviceError::ConnectionFailed("WebSocket stream ended".to_string()))
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: format!("pupil_{}", self.device_url.replace([':', '/', '.'], "_")),
            name: format!("Pupil Labs Neon ({})", self.device_url),
            device_type: DeviceType::Pupil,
            status: self.status,
            metadata: serde_json::json!({
                "device_url": self.device_url,
                "streaming": self.streaming,
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.config = config;

        if let Some(custom) = self.config.custom_settings.as_object() {
            if let Some(url) = custom.get("device_url").and_then(|v| v.as_str()) {
                self.device_url = url.to_string();
            }
        }

        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            ws.send(Message::Ping(Vec::new())).await
                .map_err(|e| DeviceError::WebSocketError(e.to_string()))?;
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}