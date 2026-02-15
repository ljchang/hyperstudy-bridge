use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Default API port for Neon Companion App REST API.
const DEFAULT_API_PORT: u16 = 8080;

/// Default timeout for HTTP requests (milliseconds).
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 5000;

// ---------------------------------------------------------------------------
// Neon REST API response types (matching OpenAPI spec v2.1.0)
// ---------------------------------------------------------------------------

/// Generic API response envelope. All Neon REST endpoints wrap responses in
/// `{"message": "...", "result": ...}`.
#[derive(Debug, Clone, Deserialize)]
struct ApiResponse<T> {
    #[allow(dead_code)]
    message: String,
    result: T,
}

/// A single component from the GET /api/status heterogeneous array.
/// The status endpoint returns `{"result": [{"model": "Phone", "data": {...}}, ...]}`.
#[derive(Debug, Clone, Deserialize)]
struct StatusItem {
    model: String,
    data: serde_json::Value,
}

/// Parsed status from Neon Companion App, assembled from the heterogeneous
/// status array returned by GET /api/status.
#[derive(Debug, Clone, Serialize)]
pub struct NeonStatus {
    pub phone: PhoneInfo,
    pub hardware: Option<HardwareInfo>,
    pub sensors: Vec<SensorInfo>,
    pub recording: Option<RecordingInfo>,
}

impl NeonStatus {
    /// Parse a NeonStatus from the heterogeneous array returned by GET /api/status.
    fn from_status_items(items: Vec<StatusItem>) -> Result<Self, DeviceError> {
        let mut phone = None;
        let mut hardware = None;
        let mut sensors = Vec::new();
        let mut recording = None;

        for item in items {
            match item.model.as_str() {
                "Phone" => {
                    phone = Some(serde_json::from_value(item.data).map_err(|e| {
                        DeviceError::InvalidData(format!("Failed to parse Phone data: {}", e))
                    })?);
                }
                "Hardware" => {
                    hardware = Some(serde_json::from_value(item.data).map_err(|e| {
                        DeviceError::InvalidData(format!("Failed to parse Hardware data: {}", e))
                    })?);
                }
                "Sensor" => {
                    sensors.push(serde_json::from_value(item.data).map_err(|e| {
                        DeviceError::InvalidData(format!("Failed to parse Sensor data: {}", e))
                    })?);
                }
                "Recording" => {
                    recording = Some(serde_json::from_value(item.data).map_err(|e| {
                        DeviceError::InvalidData(format!("Failed to parse Recording data: {}", e))
                    })?);
                }
                other => {
                    debug!(model = %other, "Unknown status component, skipping");
                }
            }
        }

        let phone = phone.ok_or_else(|| {
            DeviceError::InvalidData("Missing Phone component in status response".to_string())
        })?;

        Ok(NeonStatus {
            phone,
            hardware,
            sensors,
            recording,
        })
    }
}

/// Phone/device information from Neon status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneInfo {
    pub ip: String,
    #[serde(default)]
    pub port: u16,
    pub device_id: String,
    pub device_name: String,
    pub battery_level: f32,
    pub battery_state: String,
    pub memory: u64,
    pub memory_state: String,
    #[serde(default)]
    pub time_echo_port: u16,
}

/// Hardware information (glasses/camera serials).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub glasses_serial: String,
    #[serde(default)]
    pub world_camera_serial: String,
    #[serde(default)]
    pub module_serial: String,
}

/// Sensor connection information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorInfo {
    pub sensor: String,
    pub conn_type: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub params: String,
    #[serde(default)]
    pub connected: bool,
}

/// Recording state information (from status array component).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingInfo {
    pub id: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub rec_duration_ns: u64,
    #[serde(default)]
    pub message: String,
}

/// POST /api/event request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRequest {
    pub name: String,
    /// Optional timestamp in nanoseconds since Unix epoch.
    /// If omitted, the Companion App uses its own clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

/// POST /api/event response body (inside envelope `result`).
/// Note: The Neon API does not currently return the event name in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    #[serde(default)]
    pub name: String,
    pub recording_id: String,
    pub timestamp: i64,
}

/// POST /api/recording:start response body (inside envelope `result`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStartResponse {
    pub id: String,
}

/// POST /api/recording:stop_and_save response body (inside envelope `result`).
#[derive(Debug, Clone, Deserialize)]
struct RecordingStopResponse {
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    rec_duration_ns: u64,
}

/// POST /api/recording:cancel response body (inside envelope `result`).
#[derive(Debug, Clone, Deserialize)]
struct RecordingCancelResponse {
    #[allow(dead_code)]
    id: String,
}

// ---------------------------------------------------------------------------
// Command routing: JSON payloads sent via Device::send()
// ---------------------------------------------------------------------------

/// Internal command structure parsed from send() byte payload.
#[derive(Debug, Clone, Deserialize)]
struct PupilCommand {
    command: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    timestamp: Option<i64>,
}

// ---------------------------------------------------------------------------
// PupilDevice — Neon Companion REST API client
// ---------------------------------------------------------------------------

/// Pupil Labs Neon device controller using the Companion App REST API.
///
/// This module handles recording control, event sending, and status monitoring
/// via HTTP requests to the Neon Companion App (port 8080). Gaze data streaming
/// is handled separately via LSL through the `neon.rs` module.
#[derive(Debug)]
pub struct PupilDevice {
    http_client: reqwest::Client,
    base_url: String,
    device_ip: String,
    status: DeviceStatus,
    config: DeviceConfig,
    recording_id: Option<String>,
    neon_status: Option<NeonStatus>,
    connection_retry_count: u32,
    max_retries: u32,
}

impl PupilDevice {
    /// Create a new PupilDevice for the given host (e.g., "neon.local:8080" or "192.168.1.100").
    ///
    /// If no port is specified, defaults to 8080.
    pub fn new(host: String) -> Self {
        let (device_ip, base_url) = Self::parse_host(&host);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS))
            .build()
            .unwrap_or_default();

        Self {
            http_client,
            base_url,
            device_ip,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            recording_id: None,
            neon_status: None,
            connection_retry_count: 0,
            max_retries: 3,
        }
    }

    /// Parse a host string into (ip, base_url).
    /// Accepts formats: "neon.local:8080", "192.168.1.100", "neon.local"
    fn parse_host(host: &str) -> (String, String) {
        // Strip any protocol prefix if accidentally included
        let cleaned = host
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_start_matches("ws://")
            .trim_start_matches("wss://");

        let (ip_part, port) = if let Some((ip, port_str)) = cleaned.rsplit_once(':') {
            let port = port_str.parse::<u16>().unwrap_or(DEFAULT_API_PORT);
            (ip.to_string(), port)
        } else {
            (cleaned.to_string(), DEFAULT_API_PORT)
        };

        let base_url = format!("http://{}:{}/api", ip_part, port);
        (ip_part, base_url)
    }

    // -----------------------------------------------------------------------
    // Public helper methods (convenience wrappers for common REST operations)
    // -----------------------------------------------------------------------

    /// Fetch current device status from `GET /api/status`.
    ///
    /// The Neon API returns a heterogeneous array of `{"model": "...", "data": {...}}`
    /// objects wrapped in a `{"message": "...", "result": [...]}` envelope.
    pub async fn get_neon_status(&mut self) -> Result<NeonStatus, DeviceError> {
        let url = format!("{}/status", self.base_url);
        let resp = self.http_client.get(&url).send().await.map_err(|e| {
            DeviceError::CommunicationError(format!("GET /api/status failed: {}", e))
        })?;

        if !resp.status().is_success() {
            return Err(DeviceError::CommunicationError(format!(
                "GET /api/status returned {}",
                resp.status()
            )));
        }

        let envelope: ApiResponse<Vec<StatusItem>> = resp.json().await.map_err(|e| {
            DeviceError::InvalidData(format!("Failed to parse status envelope: {}", e))
        })?;

        let neon_status = NeonStatus::from_status_items(envelope.result)?;
        self.neon_status = Some(neon_status.clone());
        Ok(neon_status)
    }

    /// Start a recording. Returns the recording UUID.
    pub async fn start_recording(&mut self) -> Result<String, DeviceError> {
        let url = format!("{}/recording:start", self.base_url);
        let resp = self.http_client.post(&url).send().await.map_err(|e| {
            DeviceError::CommunicationError(format!("POST recording:start failed: {}", e))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DeviceError::CommunicationError(format!(
                "recording:start returned {} — {}",
                status, body
            )));
        }

        let envelope: ApiResponse<RecordingStartResponse> = resp.json().await.map_err(|e| {
            DeviceError::InvalidData(format!("Failed to parse recording response: {}", e))
        })?;

        self.recording_id = Some(envelope.result.id.clone());
        info!(device = "pupil", recording_id = %envelope.result.id, "Recording started");
        Ok(envelope.result.id)
    }

    /// Stop the current recording and save it.
    pub async fn stop_recording(&mut self) -> Result<(), DeviceError> {
        let url = format!("{}/recording:stop_and_save", self.base_url);
        let resp = self.http_client.post(&url).send().await.map_err(|e| {
            DeviceError::CommunicationError(format!("POST recording:stop_and_save failed: {}", e))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DeviceError::CommunicationError(format!(
                "recording:stop_and_save returned {} — {}",
                status, body
            )));
        }

        // Parse envelope to validate response
        let _envelope: ApiResponse<RecordingStopResponse> = resp.json().await.map_err(|e| {
            DeviceError::InvalidData(format!("Failed to parse stop response: {}", e))
        })?;

        info!(device = "pupil", "Recording stopped and saved");
        self.recording_id = None;
        Ok(())
    }

    /// Cancel the current recording without saving.
    pub async fn cancel_recording(&mut self) -> Result<(), DeviceError> {
        let url = format!("{}/recording:cancel", self.base_url);
        let resp = self.http_client.post(&url).send().await.map_err(|e| {
            DeviceError::CommunicationError(format!("POST recording:cancel failed: {}", e))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(DeviceError::CommunicationError(format!(
                "recording:cancel returned {} — {}",
                status, body
            )));
        }

        // Parse envelope to validate response
        let _envelope: ApiResponse<RecordingCancelResponse> = resp.json().await.map_err(|e| {
            DeviceError::InvalidData(format!("Failed to parse cancel response: {}", e))
        })?;

        info!(device = "pupil", "Recording cancelled");
        self.recording_id = None;
        Ok(())
    }

    /// Send an event annotation to the Neon Companion App.
    ///
    /// Timestamps are in nanoseconds since Unix epoch. If `timestamp_ns` is None,
    /// the Companion App uses its own clock.
    pub async fn send_neon_event(
        &mut self,
        name: &str,
        timestamp_ns: Option<i64>,
    ) -> Result<EventResponse, DeviceError> {
        let url = format!("{}/event", self.base_url);
        let body = EventRequest {
            name: name.to_string(),
            timestamp: timestamp_ns,
        };

        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                DeviceError::CommunicationError(format!("POST /api/event failed: {}", e))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let resp_body = resp.text().await.unwrap_or_default();
            return Err(DeviceError::CommunicationError(format!(
                "POST /api/event returned {} — {}",
                status, resp_body
            )));
        }

        let envelope: ApiResponse<EventResponse> = resp.json().await.map_err(|e| {
            DeviceError::InvalidData(format!("Failed to parse event response: {}", e))
        })?;

        // The Neon API does not return the event name — inject it manually
        let mut event_resp = envelope.result;
        event_resp.name = name.to_string();

        debug!(device = "pupil", event = %event_resp.name, "Event sent successfully");
        Ok(event_resp)
    }

    /// Check if a recording is currently active.
    pub fn is_recording(&self) -> bool {
        self.recording_id.is_some()
    }

    /// Get the cached Neon status (from last connect/heartbeat).
    pub fn get_cached_status(&self) -> Option<&NeonStatus> {
        self.neon_status.as_ref()
    }

    /// Route a JSON command to the appropriate REST endpoint.
    async fn route_command(&mut self, data: &[u8]) -> Result<serde_json::Value, DeviceError> {
        let text = String::from_utf8_lossy(data);

        let cmd: PupilCommand = serde_json::from_str(&text)
            .map_err(|e| DeviceError::InvalidData(format!("Invalid command JSON: {}", e)))?;

        match cmd.command.as_str() {
            "recording_start" => {
                let id = self.start_recording().await?;
                Ok(serde_json::json!({ "recording_id": id }))
            }
            "recording_stop" => {
                self.stop_recording().await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "recording_cancel" => {
                self.cancel_recording().await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "event" => {
                let name = cmd.name.unwrap_or_else(|| "unnamed".to_string());
                let resp = self.send_neon_event(&name, cmd.timestamp).await?;
                Ok(serde_json::json!({
                    "name": resp.name,
                    "recording_id": resp.recording_id,
                    "timestamp": resp.timestamp,
                }))
            }
            "status" => {
                let status = self.get_neon_status().await?;
                Ok(serde_json::to_value(&status)
                    .map_err(|e| DeviceError::InvalidData(e.to_string()))?)
            }
            other => Err(DeviceError::InvalidData(format!(
                "Unknown Pupil command: {}",
                other
            ))),
        }
    }
}

#[async_trait]
impl Device for PupilDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!(
            device = "pupil",
            "Connecting to Neon Companion at {}", self.base_url
        );
        self.status = DeviceStatus::Connecting;
        self.connection_retry_count = 0;

        let connect_timeout = Duration::from_millis(self.config.timeout_ms);

        loop {
            match timeout(connect_timeout, self.get_neon_status()).await {
                Ok(Ok(status)) => {
                    self.neon_status = Some(status.clone());
                    self.status = DeviceStatus::Connected;
                    self.connection_retry_count = 0;

                    info!(
                        device = "pupil",
                        device_name = %status.phone.device_name,
                        device_id = %status.phone.device_id,
                        battery = %status.phone.battery_level,
                        "Connected to Neon Companion"
                    );

                    // Check for active recording
                    if let Some(ref rec) = status.recording {
                        self.recording_id = Some(rec.id.clone());
                        info!(
                            device = "pupil",
                            recording_id = %rec.id,
                            "Found active recording"
                        );
                    }

                    return Ok(());
                }
                Ok(Err(e)) => {
                    self.connection_retry_count += 1;
                    error!(
                        device = "pupil",
                        "Failed to connect to Neon Companion: {}", e
                    );

                    if self.config.auto_reconnect && self.connection_retry_count < self.max_retries
                    {
                        warn!(
                            device = "pupil",
                            "Retrying connection ({}/{})",
                            self.connection_retry_count,
                            self.max_retries
                        );
                        tokio::time::sleep(Duration::from_millis(
                            self.config.reconnect_interval_ms,
                        ))
                        .await;
                        continue;
                    }

                    self.status = DeviceStatus::Error;
                    return Err(DeviceError::ConnectionFailed(format!(
                        "Cannot reach Neon Companion at {}: {}",
                        self.base_url, e
                    )));
                }
                Err(_) => {
                    self.connection_retry_count += 1;
                    error!(device = "pupil", "Connection timeout to Neon Companion");

                    if self.config.auto_reconnect && self.connection_retry_count < self.max_retries
                    {
                        warn!(
                            device = "pupil",
                            "Retrying after timeout ({}/{})",
                            self.connection_retry_count,
                            self.max_retries
                        );
                        tokio::time::sleep(Duration::from_millis(
                            self.config.reconnect_interval_ms,
                        ))
                        .await;
                        continue;
                    }

                    self.status = DeviceStatus::Error;
                    return Err(DeviceError::Timeout);
                }
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!(device = "pupil", "Disconnecting from Neon Companion");

        self.status = DeviceStatus::Disconnected;
        self.neon_status = None;
        self.recording_id = None;
        self.connection_retry_count = 0;

        info!(device = "pupil", "Disconnected from Neon Companion");
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        match self.route_command(data).await {
            Ok(result) => {
                debug!(device = "pupil", result = %result, "Command executed");
                Ok(())
            }
            Err(e) => {
                error!(device = "pupil", "Command failed: {}", e);
                Err(e)
            }
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        // Poll status endpoint for updates.
        // Gaze data streaming is handled by the LSL neon.rs module, not here.
        match self.get_neon_status().await {
            Ok(status) => {
                let json = serde_json::to_vec(&status)
                    .map_err(|e| DeviceError::InvalidData(e.to_string()))?;
                Ok(json)
            }
            Err(e) => {
                warn!(device = "pupil", "Failed to poll status: {}", e);
                Ok(Vec::new())
            }
        }
    }

    fn get_info(&self) -> DeviceInfo {
        let mut metadata = serde_json::json!({
            "base_url": self.base_url,
            "device_ip": self.device_ip,
            "recording_id": self.recording_id,
        });

        if let Some(ref status) = self.neon_status {
            metadata["device_name"] = serde_json::json!(status.phone.device_name);
            metadata["device_id"] = serde_json::json!(status.phone.device_id);
            metadata["battery_level"] = serde_json::json!(status.phone.battery_level);
            metadata["battery_state"] = serde_json::json!(status.phone.battery_state);
            metadata["memory_state"] = serde_json::json!(status.phone.memory_state);
            metadata["sensor_count"] = serde_json::json!(status.sensors.len());
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
        info!(device = "pupil", "Configuring Pupil device");
        self.config = config;

        if let Some(custom) = self.config.custom_settings.as_object() {
            // Update device IP/URL if provided
            if let Some(ip) = custom.get("device_ip").and_then(|v| v.as_str()) {
                let (device_ip, base_url) = Self::parse_host(ip);
                self.device_ip = device_ip;
                self.base_url = base_url;
            }

            if let Some(url) = custom.get("device_url").and_then(|v| v.as_str()) {
                let (device_ip, base_url) = Self::parse_host(url);
                self.device_ip = device_ip;
                self.base_url = base_url;
            }

            // Update max retries if provided
            if let Some(retries) = custom.get("max_retries").and_then(|v| v.as_u64()) {
                self.max_retries = retries as u32;
            }
        }

        // Rebuild HTTP client with updated timeout
        self.http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .build()
            .unwrap_or_default();

        info!(device = "pupil", "Pupil device configuration updated");
        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        match self.get_neon_status().await {
            Ok(_) => {
                debug!(device = "pupil", "Heartbeat OK");
                Ok(())
            }
            Err(e) => {
                error!(device = "pupil", "Heartbeat failed: {}", e);
                self.status = DeviceStatus::Error;
                Err(e)
            }
        }
    }

    async fn test_connection(&mut self) -> Result<bool, DeviceError> {
        let test_url = format!("{}/status", self.base_url);
        let test_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        match test_client.get(&test_url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    async fn send_event(&mut self, event: serde_json::Value) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        let name = event
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed");
        let timestamp = event.get("timestamp").and_then(|v| v.as_i64());

        self.send_neon_event(name, timestamp).await?;
        Ok(())
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
        assert_eq!(device.base_url, "http://192.168.1.100:8080/api");
        assert_eq!(device.device_ip, "192.168.1.100");
    }

    #[test]
    fn test_pupil_device_creation_with_port() {
        let device = PupilDevice::new("neon.local:8080".to_string());

        assert_eq!(device.base_url, "http://neon.local:8080/api");
        assert_eq!(device.device_ip, "neon.local");
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[test]
    fn test_pupil_device_creation_default_port() {
        let device = PupilDevice::new("neon.local".to_string());

        assert_eq!(device.base_url, "http://neon.local:8080/api");
        assert_eq!(device.device_ip, "neon.local");
    }

    #[test]
    fn test_pupil_device_strips_protocol() {
        let device = PupilDevice::new("http://192.168.1.100:8080".to_string());
        assert_eq!(device.base_url, "http://192.168.1.100:8080/api");

        let device2 = PupilDevice::new("ws://192.168.1.100:8080".to_string());
        assert_eq!(device2.base_url, "http://192.168.1.100:8080/api");
    }

    #[test]
    fn test_device_info() {
        let device = PupilDevice::new("192.168.1.100".to_string());
        let info = device.get_info();

        assert_eq!(info.device_type, DeviceType::Pupil);
        assert!(info.name.contains("192.168.1.100"));
        assert_eq!(info.id, "pupil_192_168_1_100");
    }

    #[test]
    fn test_device_configuration() {
        let mut device = PupilDevice::new("192.168.1.100".to_string());

        let mut config = DeviceConfig::default();
        config.timeout_ms = 10000;
        config.custom_settings = serde_json::json!({
            "device_ip": "192.168.1.101:8080",
            "max_retries": 5,
        });

        let result = device.configure(config);
        assert!(result.is_ok());
        assert_eq!(device.base_url, "http://192.168.1.101:8080/api");
        assert_eq!(device.max_retries, 5);
    }

    #[test]
    fn test_neon_status_from_api_response() {
        // Simulate the actual Neon API response format:
        // {"message": "...", "result": [{"model": "Phone", "data": {...}}, ...]}
        let api_json = serde_json::json!({
            "message": "success",
            "result": [
                {
                    "model": "Phone",
                    "data": {
                        "ip": "192.168.1.100",
                        "device_id": "abc123",
                        "device_name": "Neon Test",
                        "battery_level": 0.85,
                        "battery_state": "OK",
                        "memory": 4000000000_u64,
                        "memory_state": "OK"
                    }
                },
                {
                    "model": "Hardware",
                    "data": {
                        "version": "2.0",
                        "glasses_serial": "GL-001",
                        "world_camera_serial": "WC-001"
                    }
                },
                {
                    "model": "Sensor",
                    "data": {
                        "sensor": "world",
                        "conn_type": "DIRECT",
                        "protocol": "rtsp",
                        "ip": "192.168.1.100",
                        "port": 8086,
                        "connected": true
                    }
                },
                {
                    "model": "Sensor",
                    "data": {
                        "sensor": "gaze",
                        "conn_type": "WEBSOCKET",
                        "connected": true
                    }
                },
                {
                    "model": "Recording",
                    "data": {
                        "id": "550e8400-e29b-41d4-a716-446655440000",
                        "action": "START",
                        "rec_duration_ns": 5000000000_u64
                    }
                }
            ]
        });

        // Parse the envelope
        let envelope: ApiResponse<Vec<StatusItem>> = serde_json::from_value(api_json).unwrap();
        assert_eq!(envelope.message, "success");

        // Parse status from heterogeneous array
        let status = NeonStatus::from_status_items(envelope.result).unwrap();
        assert_eq!(status.phone.device_name, "Neon Test");
        assert_eq!(status.phone.battery_level, 0.85);
        assert!(status.hardware.is_some());
        assert_eq!(status.hardware.unwrap().glasses_serial, "GL-001");
        assert_eq!(status.sensors.len(), 2);
        assert_eq!(status.sensors[0].sensor, "world");
        assert!(status.sensors[0].connected);
        assert!(status.recording.is_some());
        assert_eq!(status.recording.unwrap().action, "START");
    }

    #[test]
    fn test_neon_status_missing_phone() {
        // Status response without a Phone component should fail
        let items = vec![StatusItem {
            model: "Sensor".to_string(),
            data: serde_json::json!({"sensor": "world", "conn_type": "DIRECT"}),
        }];

        let result = NeonStatus::from_status_items(items);
        assert!(result.is_err());
    }

    #[test]
    fn test_neon_status_minimal() {
        // Minimal status with just Phone (no hardware, sensors, or recording)
        let items = vec![StatusItem {
            model: "Phone".to_string(),
            data: serde_json::json!({
                "ip": "192.168.1.100",
                "device_id": "abc123",
                "device_name": "Neon Minimal",
                "battery_level": 0.5,
                "battery_state": "OK",
                "memory": 2000000000_u64,
                "memory_state": "OK"
            }),
        }];

        let status = NeonStatus::from_status_items(items).unwrap();
        assert_eq!(status.phone.device_name, "Neon Minimal");
        assert!(status.hardware.is_none());
        assert!(status.sensors.is_empty());
        assert!(status.recording.is_none());
    }

    #[test]
    fn test_recording_start_envelope() {
        let json = serde_json::json!({
            "message": "Recording started",
            "result": {
                "id": "550e8400-e29b-41d4-a716-446655440000"
            }
        });

        let envelope: ApiResponse<RecordingStartResponse> = serde_json::from_value(json).unwrap();
        assert_eq!(envelope.result.id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_event_response_envelope() {
        // Note: The Neon API does not return the event name in the response
        let json = serde_json::json!({
            "message": "Event sent",
            "result": {
                "recording_id": "550e8400-e29b-41d4-a716-446655440000",
                "timestamp": 1700000000000000000_i64
            }
        });

        let envelope: ApiResponse<EventResponse> = serde_json::from_value(json).unwrap();
        assert_eq!(
            envelope.result.recording_id,
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(envelope.result.timestamp, 1700000000000000000);
        assert_eq!(envelope.result.name, ""); // Not returned by API
    }

    #[test]
    fn test_event_request_serialization() {
        let event = EventRequest {
            name: "stimulus_onset".to_string(),
            timestamp: Some(1700000000_000_000_000),
        };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["name"], "stimulus_onset");
        assert_eq!(json["timestamp"], 1700000000_000_000_000_i64);
    }

    #[test]
    fn test_event_request_without_timestamp() {
        let event = EventRequest {
            name: "marker".to_string(),
            timestamp: None,
        };
        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["name"], "marker");
        assert!(json.get("timestamp").is_none());
    }

    #[test]
    fn test_command_routing_parse() {
        let cmd_json = r#"{"command": "recording_start"}"#;
        let cmd: PupilCommand = serde_json::from_str(cmd_json).unwrap();
        assert_eq!(cmd.command, "recording_start");

        let cmd_json = r#"{"command": "event", "name": "stim", "timestamp": 1700000000000000000}"#;
        let cmd: PupilCommand = serde_json::from_str(cmd_json).unwrap();
        assert_eq!(cmd.command, "event");
        assert_eq!(cmd.name.unwrap(), "stim");
        assert_eq!(cmd.timestamp.unwrap(), 1700000000_000_000_000);
    }
}
