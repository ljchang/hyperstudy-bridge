use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[allow(dead_code)]
const DISCOVERY_PORT: u16 = 8080;
#[allow(dead_code)]
const WS_PORT: u16 = 8081;
const DEFAULT_WS_ENDPOINT: &str = "/api/ws";

/// JSON message structure for Pupil Labs Real-Time API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PupilMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub payload: serde_json::Value,
    pub timestamp: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Gaze data structure from Pupil Labs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GazeData {
    pub timestamp: f64,
    pub gaze_position_2d: (f64, f64), // Normalized coordinates [0-1]
    pub gaze_position_3d: Option<(f64, f64, f64)>,
    pub confidence: f64,
    pub eye_id: u8, // 0 = left, 1 = right, 2 = binocular
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pupil_diameter: Option<f64>,
}

/// Pupil data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PupilData {
    pub timestamp: f64,
    pub eye_id: u8,
    pub confidence: f64,
    pub diameter: f64,
    pub ellipse: PupilEllipse,
}

/// Pupil ellipse parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PupilEllipse {
    pub center: (f64, f64),
    pub axes: (f64, f64),
    pub angle: f64,
}

/// Recording control commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingCommand {
    #[serde(rename = "start")]
    Start { template: Option<String> },
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "cancel")]
    Cancel,
}

/// Event annotation structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAnnotation {
    pub timestamp: f64,
    pub label: String,
    pub duration: Option<f64>,
    pub extra_data: Option<HashMap<String, serde_json::Value>>,
}

/// Device information from Pupil Labs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PupilDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub serial_number: String,
    pub firmware_version: String,
    pub battery_level: Option<f32>,
    pub memory_usage: Option<f32>,
}

/// Streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub gaze: bool,
    pub pupil: bool,
    pub video: bool,
    pub imu: bool,
    pub frame_rate: Option<f64>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            gaze: true,
            pupil: false,
            video: false,
            imu: false,
            frame_rate: None,
        }
    }
}

#[derive(Debug)]
pub struct PupilDevice {
    ws_client: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    device_url: String,
    device_ip: String,
    status: DeviceStatus,
    config: DeviceConfig,
    streaming_config: StreamingConfig,
    recording: bool,
    device_info: Option<PupilDeviceInfo>,
    last_gaze_data: Option<GazeData>,
    last_pupil_data: Option<PupilData>,
    connection_retry_count: u32,
    max_retries: u32,
}

impl PupilDevice {
    pub fn new(device_ip: String) -> Self {
        let device_url = format!("ws://{}:8080{}", device_ip, DEFAULT_WS_ENDPOINT);
        Self {
            ws_client: None,
            device_url: device_url.clone(),
            device_ip,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            streaming_config: StreamingConfig::default(),
            recording: false,
            device_info: None,
            last_gaze_data: None,
            last_pupil_data: None,
            connection_retry_count: 0,
            max_retries: 3,
        }
    }

    /// Create a new device instance with custom WebSocket URL
    pub fn new_with_url(device_url: String) -> Self {
        let device_ip = device_url
            .replace("ws://", "")
            .replace("wss://", "")
            .split(':')
            .next()
            .unwrap_or("localhost")
            .to_string();

        Self {
            ws_client: None,
            device_url,
            device_ip,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            streaming_config: StreamingConfig::default(),
            recording: false,
            device_info: None,
            last_gaze_data: None,
            last_pupil_data: None,
            connection_retry_count: 0,
            max_retries: 3,
        }
    }

    /// Discover Pupil Labs devices on the local network
    pub async fn discover_devices() -> Result<Vec<String>, DeviceError> {
        // TODO: Implement mDNS discovery for Pupil Labs devices
        // For now, return common local IP ranges for manual testing
        let common_ips = vec![
            "192.168.1.100".to_string(),
            "192.168.1.101".to_string(),
            "192.168.0.100".to_string(),
            "192.168.0.101".to_string(),
            "10.0.0.100".to_string(),
            "127.0.0.1".to_string(),
        ];

        info!("Device discovery not yet implemented, returning common IPs");
        Ok(common_ips)
    }

    /// Get current timestamp in Unix epoch seconds
    fn get_timestamp() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }

    /// Generate a unique message ID
    fn generate_message_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Create a PupilMessage for sending commands
    fn create_message(msg_type: &str, payload: serde_json::Value) -> PupilMessage {
        PupilMessage {
            msg_type: msg_type.to_string(),
            payload,
            timestamp: Self::get_timestamp(),
            id: Some(Self::generate_message_id()),
        }
    }

    /// Start gaze data streaming
    pub async fn start_gaze_streaming(&mut self) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message(
            "start_streaming",
            serde_json::json!({
                "data_type": "gaze",
                "format": "json"
            }),
        );

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        self.streaming_config.gaze = true;
        info!("Started gaze data streaming");
        Ok(())
    }

    /// Stop gaze data streaming
    pub async fn stop_gaze_streaming(&mut self) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message(
            "stop_streaming",
            serde_json::json!({
                "data_type": "gaze"
            }),
        );

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        self.streaming_config.gaze = false;
        info!("Stopped gaze data streaming");
        Ok(())
    }

    /// Start recording with optional template
    pub async fn start_recording(&mut self, template: Option<String>) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message(
            "recording.start",
            serde_json::json!({
                "template": template
            }),
        );

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        self.recording = true;
        info!("Started recording with template: {:?}", template);
        Ok(())
    }

    /// Stop recording
    pub async fn stop_recording(&mut self) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message("recording.stop", serde_json::json!({}));

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        self.recording = false;
        info!("Stopped recording");
        Ok(())
    }

    /// Send event annotation
    pub async fn send_event(&mut self, event: EventAnnotation) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message(
            "event",
            serde_json::to_value(&event).map_err(|e| DeviceError::InvalidData(e.to_string()))?,
        );

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        debug!("Sent event annotation: {}", event.label);
        Ok(())
    }

    /// Request device information
    pub async fn request_device_info(&mut self) -> Result<(), DeviceError> {
        if self.ws_client.is_none() {
            return Err(DeviceError::NotConnected);
        }

        let message = Self::create_message("device.info", serde_json::json!({}));

        let json_str =
            serde_json::to_string(&message).map_err(|e| DeviceError::InvalidData(e.to_string()))?;

        self.send(json_str.as_bytes()).await?;
        debug!("Requested device information");
        Ok(())
    }

    /// Process incoming message and update internal state
    fn process_message(&mut self, message_text: &str) -> Result<(), DeviceError> {
        match serde_json::from_str::<PupilMessage>(message_text) {
            Ok(msg) => {
                debug!("Received message type: {}", msg.msg_type);

                match msg.msg_type.as_str() {
                    "gaze" => {
                        if let Ok(gaze_data) = serde_json::from_value::<GazeData>(msg.payload) {
                            self.last_gaze_data = Some(gaze_data);
                            debug!("Updated gaze data");
                        }
                    }
                    "pupil" => {
                        if let Ok(pupil_data) = serde_json::from_value::<PupilData>(msg.payload) {
                            self.last_pupil_data = Some(pupil_data);
                            debug!("Updated pupil data");
                        }
                    }
                    "device.info" => {
                        if let Ok(device_info) =
                            serde_json::from_value::<PupilDeviceInfo>(msg.payload)
                        {
                            self.device_info = Some(device_info);
                            info!("Updated device information");
                        }
                    }
                    "recording.started" => {
                        self.recording = true;
                        info!("Recording started confirmation received");
                    }
                    "recording.stopped" => {
                        self.recording = false;
                        info!("Recording stopped confirmation received");
                    }
                    "error" => {
                        warn!("Received error from device: {:?}", msg.payload);
                    }
                    _ => {
                        debug!("Unknown message type: {}", msg.msg_type);
                    }
                }
                Ok(())
            }
            Err(e) => {
                debug!("Failed to parse message as PupilMessage: {}", e);
                // Message might be raw data or different format, not necessarily an error
                Ok(())
            }
        }
    }

    /// Get the latest gaze data
    pub fn get_latest_gaze_data(&self) -> Option<&GazeData> {
        self.last_gaze_data.as_ref()
    }

    /// Get the latest pupil data
    pub fn get_latest_pupil_data(&self) -> Option<&PupilData> {
        self.last_pupil_data.as_ref()
    }

    /// Get device information
    pub fn get_device_info(&self) -> Option<&PupilDeviceInfo> {
        self.device_info.as_ref()
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Get current streaming configuration
    pub fn get_streaming_config(&self) -> &StreamingConfig {
        &self.streaming_config
    }
}

#[async_trait]
impl Device for PupilDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting to Pupil Labs Neon at {}", self.device_url);
        self.status = DeviceStatus::Connecting;
        self.connection_retry_count = 0;

        let url = if !self.device_url.starts_with("ws://") && !self.device_url.starts_with("wss://")
        {
            format!("ws://{}", self.device_url)
        } else {
            self.device_url.clone()
        };

        // Apply connection timeout from config
        let connect_timeout = tokio::time::Duration::from_millis(self.config.timeout_ms);

        // Use loop-based retry instead of recursion to prevent stack overflow
        loop {
            match tokio::time::timeout(connect_timeout, connect_async(&url)).await {
                Ok(Ok((ws_stream, response))) => {
                    self.ws_client = Some(ws_stream);
                    self.status = DeviceStatus::Connected;
                    self.connection_retry_count = 0;

                    info!(
                        "Successfully connected to Pupil Labs Neon. Status: {}",
                        response.status()
                    );

                    // Request device information after successful connection
                    if let Err(e) = self.request_device_info().await {
                        warn!("Failed to request device info: {}", e);
                    }

                    return Ok(());
                }
                Ok(Err(e)) => {
                    self.status = DeviceStatus::Error;
                    self.connection_retry_count += 1;
                    error!("Failed to connect to Pupil Labs Neon: {}", e);

                    // Auto-retry if enabled and under retry limit
                    if self.config.auto_reconnect && self.connection_retry_count < self.max_retries
                    {
                        warn!(
                            "Retrying connection ({}/{})",
                            self.connection_retry_count, self.max_retries
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            self.config.reconnect_interval_ms,
                        ))
                        .await;
                        continue; // Retry via loop instead of recursion
                    }

                    return Err(DeviceError::WebSocketError(e.to_string()));
                }
                Err(_) => {
                    self.status = DeviceStatus::Error;
                    self.connection_retry_count += 1;
                    error!("Connection timeout to Pupil Labs Neon");

                    if self.config.auto_reconnect && self.connection_retry_count < self.max_retries
                    {
                        warn!(
                            "Retrying connection after timeout ({}/{})",
                            self.connection_retry_count, self.max_retries
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            self.config.reconnect_interval_ms,
                        ))
                        .await;
                        continue; // Retry via loop instead of recursion
                    }

                    return Err(DeviceError::Timeout);
                }
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting from Pupil Labs Neon");

        // Stop any active streaming before disconnecting
        if self.streaming_config.gaze {
            let _ = self.stop_gaze_streaming().await;
        }

        // Stop recording if active
        if self.recording {
            let _ = self.stop_recording().await;
        }

        if let Some(mut ws) = self.ws_client.take() {
            if let Err(e) = ws.close(None).await {
                warn!("Error closing WebSocket connection: {}", e);
            }
        }

        self.status = DeviceStatus::Disconnected;
        self.streaming_config = StreamingConfig::default();
        self.recording = false;
        self.device_info = None;
        self.last_gaze_data = None;
        self.last_pupil_data = None;
        self.connection_retry_count = 0;

        info!("Successfully disconnected from Pupil Labs Neon");
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            let message = String::from_utf8_lossy(data);

            // Apply send timeout
            let send_timeout = tokio::time::Duration::from_millis(self.config.timeout_ms);

            match tokio::time::timeout(
                send_timeout,
                ws.send(Message::Text(message.to_string().into())),
            )
            .await
            {
                Ok(Ok(())) => {
                    debug!("Sent message to Pupil: {}", message);
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("WebSocket send error: {}", e);
                    self.status = DeviceStatus::Error;
                    Err(DeviceError::WebSocketError(e.to_string()))
                }
                Err(_) => {
                    error!("Send timeout to Pupil Labs Neon");
                    self.status = DeviceStatus::Error;
                    Err(DeviceError::Timeout)
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            // Apply receive timeout
            let receive_timeout = tokio::time::Duration::from_millis(self.config.timeout_ms);

            match tokio::time::timeout(receive_timeout, ws.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    debug!("Received text from Pupil: {}", text);

                    // Process the message to update internal state
                    if let Err(e) = self.process_message(&text) {
                        warn!("Failed to process received message: {}", e);
                    }

                    Ok(text.as_bytes().to_vec())
                }
                Ok(Some(Ok(Message::Binary(data)))) => {
                    debug!("Received {} bytes from Pupil", data.len());
                    Ok(data.to_vec())
                }
                Ok(Some(Ok(Message::Close(frame)))) => {
                    info!("WebSocket closed by remote: {:?}", frame);
                    self.status = DeviceStatus::Disconnected;
                    self.ws_client = None;
                    self.recording = false;
                    self.streaming_config = StreamingConfig::default();
                    Err(DeviceError::ConnectionFailed(
                        "WebSocket closed by remote".to_string(),
                    ))
                }
                Ok(Some(Ok(Message::Ping(data)))) => {
                    // Respond to ping with pong
                    if let Err(e) = ws.send(Message::Pong(data)).await {
                        warn!("Failed to send pong response: {}", e);
                    }
                    Ok(Vec::new())
                }
                Ok(Some(Ok(Message::Pong(_)))) => {
                    debug!("Received pong from Pupil");
                    Ok(Vec::new())
                }
                Ok(Some(Ok(Message::Frame(_)))) => {
                    // Raw frame, not typically used
                    Ok(Vec::new())
                }
                Ok(Some(Err(e))) => {
                    error!("WebSocket receive error: {}", e);
                    self.status = DeviceStatus::Error;
                    Err(DeviceError::WebSocketError(e.to_string()))
                }
                Ok(None) => {
                    info!("WebSocket stream ended");
                    self.status = DeviceStatus::Disconnected;
                    self.ws_client = None;
                    self.recording = false;
                    self.streaming_config = StreamingConfig::default();
                    Err(DeviceError::ConnectionFailed(
                        "WebSocket stream ended".to_string(),
                    ))
                }
                Err(_) => {
                    // Timeout occurred, this is not necessarily an error for receive operations
                    debug!("Receive timeout (no data available)");
                    Ok(Vec::new())
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        let mut metadata = serde_json::json!({
            "device_url": self.device_url,
            "device_ip": self.device_ip,
            "streaming_config": self.streaming_config,
            "recording": self.recording,
            "connection_retry_count": self.connection_retry_count,
        });

        // Add device info if available
        if let Some(ref device_info) = self.device_info {
            metadata["device_info"] = serde_json::to_value(device_info).unwrap_or_default();
        }

        // Add latest gaze data timestamp if available
        if let Some(ref gaze_data) = self.last_gaze_data {
            if let Some(timestamp_num) = serde_json::Number::from_f64(gaze_data.timestamp) {
                metadata["last_gaze_timestamp"] = serde_json::Value::Number(timestamp_num);
            }
            if let Some(confidence_num) = serde_json::Number::from_f64(gaze_data.confidence) {
                metadata["gaze_confidence"] = serde_json::Value::Number(confidence_num);
            }
        }

        DeviceInfo {
            id: format!("pupil_{}", self.device_ip.replace('.', "_")),
            name: format!("Pupil Labs Neon ({})", self.device_ip),
            device_type: DeviceType::Pupil,
            status: self.status,
            metadata,
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        info!("Configuring Pupil device with new settings");
        self.config = config;

        if let Some(custom) = self.config.custom_settings.as_object() {
            // Update device URL if provided
            if let Some(url) = custom.get("device_url").and_then(|v| v.as_str()) {
                self.device_url = url.to_string();
                self.device_ip = url
                    .replace("ws://", "")
                    .replace("wss://", "")
                    .split(':')
                    .next()
                    .unwrap_or("localhost")
                    .to_string();
            }

            // Update device IP if provided
            if let Some(ip) = custom.get("device_ip").and_then(|v| v.as_str()) {
                self.device_ip = ip.to_string();
                self.device_url = format!("ws://{}:8080{}", ip, DEFAULT_WS_ENDPOINT);
            }

            // Update max retries if provided
            if let Some(retries) = custom.get("max_retries").and_then(|v| v.as_u64()) {
                self.max_retries = retries as u32;
            }

            // Update streaming configuration if provided
            if let Some(streaming_config) = custom.get("streaming_config") {
                if let Ok(stream_config) =
                    serde_json::from_value::<StreamingConfig>(streaming_config.clone())
                {
                    self.streaming_config = stream_config;
                    debug!(
                        "Updated streaming configuration: {:?}",
                        self.streaming_config
                    );
                }
            }
        }

        info!("Pupil device configuration updated successfully");
        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if let Some(ref mut ws) = self.ws_client {
            let ping_timeout = tokio::time::Duration::from_millis(self.config.timeout_ms);

            match tokio::time::timeout(ping_timeout, ws.send(Message::Ping(Vec::new().into())))
                .await
            {
                Ok(Ok(())) => {
                    debug!("Heartbeat ping sent successfully");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("Heartbeat ping failed: {}", e);
                    self.status = DeviceStatus::Error;
                    Err(DeviceError::WebSocketError(e.to_string()))
                }
                Err(_) => {
                    error!("Heartbeat ping timeout");
                    self.status = DeviceStatus::Error;
                    Err(DeviceError::Timeout)
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pupil_device_creation() {
        let device = PupilDevice::new("192.168.1.100".to_string());

        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
        assert!(!device.is_recording());
        assert_eq!(device.get_streaming_config().gaze, true);
        assert_eq!(device.get_streaming_config().pupil, false);
    }

    #[test]
    fn test_pupil_device_creation_with_url() {
        let device = PupilDevice::new_with_url("ws://192.168.1.100:8080/api/ws".to_string());

        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
        let info = device.get_info();
        assert_eq!(info.device_type, DeviceType::Pupil);
        assert!(info.name.contains("192.168.1.100"));
    }

    #[test]
    fn test_device_configuration() {
        let mut device = PupilDevice::new("192.168.1.100".to_string());

        let mut config = DeviceConfig::default();
        config.timeout_ms = 10000;
        config.custom_settings = serde_json::json!({
            "device_ip": "192.168.1.101",
            "max_retries": 5,
            "streaming_config": {
                "gaze": true,
                "pupil": true,
                "video": false,
                "imu": false
            }
        });

        let result = device.configure(config);
        assert!(result.is_ok());
        assert_eq!(device.get_streaming_config().pupil, true);
    }

    #[test]
    fn test_gaze_data_structure() {
        let gaze_data = GazeData {
            timestamp: 1234567890.0,
            gaze_position_2d: (0.5, 0.3),
            gaze_position_3d: Some((0.1, 0.2, 0.8)),
            confidence: 0.95,
            eye_id: 2,
            pupil_diameter: Some(3.2),
        };

        assert_eq!(gaze_data.timestamp, 1234567890.0);
        assert_eq!(gaze_data.gaze_position_2d, (0.5, 0.3));
        assert_eq!(gaze_data.confidence, 0.95);
        assert_eq!(gaze_data.eye_id, 2);
    }

    #[test]
    fn test_event_annotation_structure() {
        let event = EventAnnotation {
            timestamp: 1234567890.0,
            label: "stimulus_onset".to_string(),
            duration: Some(2.5),
            extra_data: Some(HashMap::from([
                (
                    "condition".to_string(),
                    serde_json::Value::String("experimental".to_string()),
                ),
                (
                    "trial_id".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(42)),
                ),
            ])),
        };

        assert_eq!(event.label, "stimulus_onset");
        assert_eq!(event.duration, Some(2.5));
        assert!(event.extra_data.is_some());
    }

    #[test]
    fn test_pupil_message_creation() {
        let message =
            PupilDevice::create_message("test_command", serde_json::json!({"param": "value"}));

        assert_eq!(message.msg_type, "test_command");
        assert!(message.id.is_some());
        assert!(message.timestamp > 0.0);
    }

    #[tokio::test]
    async fn test_device_discovery() {
        let devices = PupilDevice::discover_devices().await;
        assert!(devices.is_ok());
        let device_list = devices.unwrap();
        assert!(!device_list.is_empty());
        assert!(device_list.contains(&"127.0.0.1".to_string()));
    }
}
