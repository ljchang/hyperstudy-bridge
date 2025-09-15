use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::measure_latency;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

// NDT Protocol Constants
const DEFAULT_PORT: u16 = 5000;
const NDT_HEADER_SIZE: usize = 8;
const MAX_CHANNELS: usize = 16;
const BUFFER_SIZE: usize = 65536; // 64KB buffer for high-frequency data
#[allow(dead_code)]
const RECONNECT_DELAY_MS: u64 = 1000;
#[allow(dead_code)]
const CONNECTION_TIMEOUT_MS: u64 = 5000;

// NDT Protocol Commands
const NDT_START_ACQUISITION: u32 = 0x01;
const NDT_STOP_ACQUISITION: u32 = 0x02;
const NDT_SET_MARKER: u32 = 0x03;
const NDT_GET_CHANNELS: u32 = 0x04;
const NDT_SET_SAMPLING_RATE: u32 = 0x05;
const NDT_DATA_PACKET: u32 = 0x10;
#[allow(dead_code)]
const NDT_STATUS_RESPONSE: u32 = 0x20;
#[allow(dead_code)]
const NDT_ERROR_RESPONSE: u32 = 0x30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub id: u8,
    pub name: String,
    pub enabled: bool,
    pub scale: f32,
    pub offset: f32,
    pub sampling_rate: u32,
    pub units: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiopacConfig {
    pub server_address: String,
    pub port: u16,
    pub channels: Vec<ChannelConfig>,
    pub master_sampling_rate: u32,
    pub buffer_size: usize,
    pub enable_event_markers: bool,
}

#[derive(Debug, Clone)]
struct NdtPacket {
    command: u32,
    length: u32,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChannelData {
    channel_id: u8,
    timestamp: u64,
    value: f32,
    raw_value: u16,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EventMarker {
    timestamp: u64,
    marker_id: String,
    metadata: HashMap<String, String>,
}

/// Type alias for performance callback
type PerformanceCallback = Box<dyn Fn(&str, Duration, u64, u64) + Send + Sync>;

pub struct BiopacDevice {
    socket: Option<TcpStream>,
    config: BiopacConfig,
    status: DeviceStatus,
    device_config: DeviceConfig,
    acquiring: bool,
    buffer: Vec<u8>,
    data_buffer: Vec<ChannelData>,
    event_buffer: Vec<EventMarker>,
    sequence_number: u32,
    last_heartbeat: SystemTime,
    reconnect_attempts: u32,
    performance_callback: Option<PerformanceCallback>,
}

impl std::fmt::Debug for BiopacDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BiopacDevice")
            .field("status", &self.status)
            .field("config", &self.config)
            .field("device_config", &self.device_config)
            .field("acquiring", &self.acquiring)
            .field("sequence_number", &self.sequence_number)
            .field("last_heartbeat", &self.last_heartbeat)
            .field("reconnect_attempts", &self.reconnect_attempts)
            .field(
                "has_performance_callback",
                &self.performance_callback.is_some(),
            )
            .finish()
    }
}

impl BiopacDevice {
    pub fn new(server_address: String) -> Self {
        let config = BiopacConfig {
            server_address,
            port: DEFAULT_PORT,
            channels: vec![ChannelConfig {
                id: 0,
                name: "Channel 0".to_string(),
                enabled: true,
                scale: 1.0,
                offset: 0.0,
                sampling_rate: 1000,
                units: "mV".to_string(),
            }],
            master_sampling_rate: 1000,
            buffer_size: BUFFER_SIZE,
            enable_event_markers: true,
        };

        Self {
            socket: None,
            config,
            status: DeviceStatus::Disconnected,
            device_config: DeviceConfig::default(),
            acquiring: false,
            buffer: Vec::with_capacity(BUFFER_SIZE),
            data_buffer: Vec::with_capacity(1000),
            event_buffer: Vec::new(),
            sequence_number: 0,
            last_heartbeat: SystemTime::now(),
            reconnect_attempts: 0,
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

    /// Configure channels for data acquisition
    pub fn configure_channels(&mut self, channels: Vec<ChannelConfig>) -> Result<(), DeviceError> {
        if channels.len() > MAX_CHANNELS {
            return Err(DeviceError::ConfigurationError(format!(
                "Too many channels configured: {} > {}",
                channels.len(),
                MAX_CHANNELS
            )));
        }

        self.config.channels = channels;
        info!(
            "Configured {} channels for Biopac device",
            self.config.channels.len()
        );
        Ok(())
    }

    /// Start data acquisition using NDT protocol
    pub async fn start_acquisition(&mut self) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        let packet = self.create_ndt_packet(NDT_START_ACQUISITION, &[]);
        let device_id = self.get_info().id;

        let (result, latency) =
            measure_latency(async { self.send_ndt_packet(&packet).await }).await;

        // Record performance metrics
        if let Some(ref callback) = self.performance_callback {
            callback(&device_id, latency, packet.data.len() as u64, 0);
        }

        result?;
        self.acquiring = true;
        self.sequence_number = 0;
        info!(
            "Started Biopac data acquisition with latency: {:?}",
            latency
        );
        Ok(())
    }

    /// Stop data acquisition using NDT protocol
    pub async fn stop_acquisition(&mut self) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        let packet = self.create_ndt_packet(NDT_STOP_ACQUISITION, &[]);
        let device_id = self.get_info().id;

        let (result, latency) =
            measure_latency(async { self.send_ndt_packet(&packet).await }).await;

        // Record performance metrics
        if let Some(ref callback) = self.performance_callback {
            callback(&device_id, latency, packet.data.len() as u64, 0);
        }

        result?;
        self.acquiring = false;
        self.data_buffer.clear();
        info!(
            "Stopped Biopac data acquisition with latency: {:?}",
            latency
        );
        Ok(())
    }

    /// Set event marker with timestamp and metadata
    pub async fn set_marker(
        &mut self,
        marker_id: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        if !self.config.enable_event_markers {
            return Err(DeviceError::ConfigurationError(
                "Event markers are disabled".to_string(),
            ));
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| DeviceError::Unknown(e.to_string()))?
            .as_micros() as u64;

        // Create marker data payload
        let mut marker_data = marker_id.as_bytes().to_vec();
        marker_data.push(0); // null terminator

        let packet = self.create_ndt_packet(NDT_SET_MARKER, &marker_data);
        let device_id = self.get_info().id;

        let (result, latency) =
            measure_latency(async { self.send_ndt_packet(&packet).await }).await;

        // Record performance metrics
        if let Some(ref callback) = self.performance_callback {
            callback(&device_id, latency, packet.data.len() as u64, 0);
        }

        result?;

        // Store marker in local buffer for analysis
        let event_marker = EventMarker {
            timestamp,
            marker_id: marker_id.to_string(),
            metadata: metadata.unwrap_or_default(),
        };
        self.event_buffer.push(event_marker);

        debug!(
            "Set marker '{}' at timestamp {} with latency: {:?}",
            marker_id, timestamp, latency
        );
        Ok(())
    }

    /// Set sampling rate for all channels
    pub async fn set_sampling_rate(&mut self, rate: u32) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        let rate_bytes = rate.to_le_bytes();
        let packet = self.create_ndt_packet(NDT_SET_SAMPLING_RATE, &rate_bytes);

        let device_id = self.get_info().id;
        let (result, latency) =
            measure_latency(async { self.send_ndt_packet(&packet).await }).await;

        // Record performance metrics
        if let Some(ref callback) = self.performance_callback {
            callback(&device_id, latency, packet.data.len() as u64, 0);
        }

        result?;

        self.config.master_sampling_rate = rate;
        info!(
            "Set sampling rate to {} Hz with latency: {:?}",
            rate, latency
        );
        Ok(())
    }

    /// Get channel information from device
    pub async fn get_channels(&mut self) -> Result<Vec<ChannelConfig>, DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        let packet = self.create_ndt_packet(NDT_GET_CHANNELS, &[]);
        let device_id = self.get_info().id;

        let (result, latency) = measure_latency(async {
            self.send_ndt_packet(&packet).await?;
            self.receive_ndt_response().await
        })
        .await;

        // Record performance metrics
        if let Some(ref callback) = self.performance_callback {
            callback(&device_id, latency, packet.data.len() as u64, 0);
        }

        let response_packet = result?;

        // Parse channel configuration from response
        let channels = self.parse_channel_config(&response_packet.data)?;
        self.config.channels = channels.clone();

        info!(
            "Retrieved {} channels from device with latency: {:?}",
            channels.len(),
            latency
        );
        Ok(channels)
    }

    /// Read and parse physiological data from device
    pub async fn read_data(&mut self) -> Result<Vec<ChannelData>, DeviceError> {
        if !self.acquiring || self.status != DeviceStatus::Connected {
            return Ok(Vec::new());
        }

        let device_id = self.get_info().id;
        let (result, latency) = measure_latency(async { self.receive_ndt_response().await }).await;

        match result {
            Ok(packet) => {
                if packet.command == NDT_DATA_PACKET {
                    let channel_data = self.parse_physiological_data(&packet.data)?;

                    // Record performance metrics
                    if let Some(ref callback) = self.performance_callback {
                        callback(&device_id, latency, 0, packet.data.len() as u64);
                    }

                    // Store in local buffer for analysis
                    self.data_buffer.extend(channel_data.clone());

                    // Maintain buffer size limit
                    if self.data_buffer.len() > self.config.buffer_size {
                        let excess = self.data_buffer.len() - self.config.buffer_size;
                        self.data_buffer.drain(0..excess);
                    }

                    debug!(
                        "Received {} channel data points with latency: {:?}",
                        channel_data.len(),
                        latency
                    );
                    Ok(channel_data)
                } else {
                    debug!("Received non-data packet: command={:02x}", packet.command);
                    Ok(Vec::new())
                }
            }
            Err(e) => {
                if let Some(ref callback) = self.performance_callback {
                    callback(&device_id, latency, 0, 0);
                }
                Err(e)
            }
        }
    }

    /// Get recent event markers
    pub fn get_event_markers(&self, since: Option<u64>) -> Vec<EventMarker> {
        match since {
            Some(timestamp) => self
                .event_buffer
                .iter()
                .filter(|marker| marker.timestamp >= timestamp)
                .cloned()
                .collect(),
            None => self.event_buffer.clone(),
        }
    }

    /// Clear data buffers
    pub fn clear_buffers(&mut self) {
        self.data_buffer.clear();
        self.event_buffer.clear();
        debug!("Cleared Biopac data buffers");
    }

    /// Get buffered data for analysis
    pub fn get_buffered_data(&self) -> &[ChannelData] {
        &self.data_buffer
    }
}

#[async_trait]
impl Device for BiopacDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!(
            "Connecting to Biopac at {}:{}",
            self.config.server_address, self.config.port
        );
        self.status = DeviceStatus::Connecting;

        let addr = format!("{}:{}", self.config.server_address, self.config.port);

        match TcpStream::connect(&addr).await {
            Ok(socket) => {
                self.socket = Some(socket);
                self.status = DeviceStatus::Connected;
                info!("Successfully connected to Biopac");
                Ok(())
            }
            Err(e) => {
                self.status = DeviceStatus::Error;
                error!("Failed to connect to Biopac: {}", e);
                Err(DeviceError::ConnectionFailed(e.to_string()))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting from Biopac");

        if self.acquiring {
            let _ = self.stop_acquisition().await;
        }

        if let Some(mut socket) = self.socket.take() {
            socket
                .shutdown()
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
        }

        self.status = DeviceStatus::Disconnected;
        self.buffer.clear();
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if let Some(ref mut socket) = self.socket {
            socket
                .write_all(data)
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

            socket
                .write_all(b"\n")
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

            socket
                .flush()
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if let Some(ref mut socket) = self.socket {
            self.buffer.clear();
            self.buffer.resize(8192, 0);

            match socket.read(&mut self.buffer).await {
                Ok(0) => {
                    self.status = DeviceStatus::Disconnected;
                    self.socket = None;
                    Err(DeviceError::ConnectionFailed(
                        "Connection closed by remote".to_string(),
                    ))
                }
                Ok(n) => {
                    self.buffer.truncate(n);
                    Ok(self.buffer.clone())
                }
                Err(e) => Err(DeviceError::CommunicationError(e.to_string())),
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: format!(
                "biopac_{}_{}",
                self.config.server_address.replace('.', "_"),
                self.config.port
            ),
            name: format!(
                "Biopac MP150/160 ({}:{})",
                self.config.server_address, self.config.port
            ),
            device_type: DeviceType::Biopac,
            status: self.status,
            metadata: serde_json::json!({
                "server_address": self.config.server_address,
                "port": self.config.port,
                "channels": self.config.channels.len(),
                "acquiring": self.acquiring,
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.device_config = config;

        if let Some(custom) = self.device_config.custom_settings.as_object() {
            // Network configuration
            if let Some(addr) = custom.get("server_address").and_then(|v| v.as_str()) {
                self.config.server_address = addr.to_string();
            }

            if let Some(port) = custom.get("port").and_then(|v| v.as_u64()) {
                self.config.port = port as u16;
            }

            // Sampling rate configuration
            if let Some(rate) = custom.get("master_sampling_rate").and_then(|v| v.as_u64()) {
                self.config.master_sampling_rate = rate as u32;
            }

            // Buffer size configuration
            if let Some(size) = custom.get("buffer_size").and_then(|v| v.as_u64()) {
                self.config.buffer_size = size as usize;
                // Resize buffers if needed
                self.data_buffer.reserve(self.config.buffer_size);
            }

            // Event marker configuration
            if let Some(enable) = custom.get("enable_event_markers").and_then(|v| v.as_bool()) {
                self.config.enable_event_markers = enable;
            }

            // Channel configuration
            if let Some(channels) = custom.get("channels").and_then(|v| v.as_array()) {
                let mut new_channels = Vec::new();
                for ch in channels {
                    if let Some(ch_obj) = ch.as_object() {
                        let channel = ChannelConfig {
                            id: ch_obj.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                            name: ch_obj
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            enabled: ch_obj
                                .get("enabled")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true),
                            scale: ch_obj.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0)
                                as f32,
                            offset: ch_obj.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0)
                                as f32,
                            sampling_rate: ch_obj
                                .get("sampling_rate")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1000) as u32,
                            units: ch_obj
                                .get("units")
                                .and_then(|v| v.as_str())
                                .unwrap_or("mV")
                                .to_string(),
                        };
                        new_channels.push(channel);
                    }
                }

                if new_channels.len() <= MAX_CHANNELS {
                    self.config.channels = new_channels;
                } else {
                    return Err(DeviceError::ConfigurationError(format!(
                        "Too many channels: {} > {}",
                        new_channels.len(),
                        MAX_CHANNELS
                    )));
                }
            }
        }

        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.socket.is_none() {
            return Err(DeviceError::NotConnected);
        }

        // Check if heartbeat is overdue
        let elapsed = self
            .last_heartbeat
            .elapsed()
            .map_err(|e| DeviceError::Unknown(e.to_string()))?;

        if elapsed > Duration::from_millis(self.device_config.timeout_ms * 2) {
            warn!("Biopac heartbeat overdue by {:?}", elapsed);

            // Attempt to send a simple status request to check connection
            let status_packet = self.create_ndt_packet(NDT_GET_CHANNELS, &[]);

            match timeout(
                Duration::from_millis(self.device_config.timeout_ms),
                self.send_ndt_packet(&status_packet),
            )
            .await
            {
                Ok(Ok(())) => {
                    self.last_heartbeat = SystemTime::now();
                    debug!("Biopac heartbeat successful");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("Biopac heartbeat failed: {}", e);
                    Err(e)
                }
                Err(_) => {
                    error!("Biopac heartbeat timed out");
                    Err(DeviceError::Timeout)
                }
            }
        } else {
            debug!("Biopac heartbeat OK (last: {:?} ago)", elapsed);
            Ok(())
        }
    }
}

impl BiopacDevice {
    // Private helper methods

    fn create_ndt_packet(&self, command: u32, data: &[u8]) -> NdtPacket {
        NdtPacket {
            command,
            length: data.len() as u32,
            data: data.to_vec(),
        }
    }

    async fn send_ndt_packet(&mut self, packet: &NdtPacket) -> Result<(), DeviceError> {
        if let Some(ref mut socket) = self.socket {
            // Send NDT header: [command: 4 bytes][length: 4 bytes]
            let mut header = Vec::with_capacity(NDT_HEADER_SIZE);
            header.extend_from_slice(&packet.command.to_le_bytes());
            header.extend_from_slice(&packet.length.to_le_bytes());

            socket
                .write_all(&header)
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

            // Send data payload if present
            if !packet.data.is_empty() {
                socket
                    .write_all(&packet.data)
                    .await
                    .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
            }

            socket
                .flush()
                .await
                .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;

            self.sequence_number = self.sequence_number.wrapping_add(1);
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive_ndt_response(&mut self) -> Result<NdtPacket, DeviceError> {
        if let Some(ref mut socket) = self.socket {
            // Read NDT header
            let mut header = [0u8; NDT_HEADER_SIZE];

            match timeout(
                Duration::from_millis(self.device_config.timeout_ms),
                socket.read_exact(&mut header),
            )
            .await
            {
                Ok(Ok(_)) => {
                    let command = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
                    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

                    // Validate packet length
                    if length > BUFFER_SIZE as u32 {
                        return Err(DeviceError::InvalidData(format!(
                            "Packet too large: {} bytes",
                            length
                        )));
                    }

                    // Read data payload
                    let mut data = vec![0u8; length as usize];
                    if length > 0 {
                        socket
                            .read_exact(&mut data)
                            .await
                            .map_err(|e| DeviceError::CommunicationError(e.to_string()))?;
                    }

                    Ok(NdtPacket {
                        command,
                        length,
                        data,
                    })
                }
                Ok(Err(e)) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        self.status = DeviceStatus::Disconnected;
                        self.socket = None;
                        Err(DeviceError::ConnectionFailed(
                            "Connection closed by remote".to_string(),
                        ))
                    } else {
                        Err(DeviceError::CommunicationError(e.to_string()))
                    }
                }
                Err(_) => Err(DeviceError::Timeout),
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn parse_channel_config(&self, data: &[u8]) -> Result<Vec<ChannelConfig>, DeviceError> {
        // Parse channel configuration from binary data
        // This is a simplified implementation - real NDT would have specific format
        if data.len() < 4 {
            return Err(DeviceError::InvalidData(
                "Channel config data too short".to_string(),
            ));
        }

        let num_channels = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut channels = Vec::with_capacity(num_channels);

        for i in 0..num_channels.min(MAX_CHANNELS) {
            channels.push(ChannelConfig {
                id: i as u8,
                name: format!("Channel {}", i),
                enabled: true,
                scale: 1.0,
                offset: 0.0,
                sampling_rate: self.config.master_sampling_rate,
                units: "mV".to_string(),
            });
        }

        Ok(channels)
    }

    fn parse_physiological_data(&self, data: &[u8]) -> Result<Vec<ChannelData>, DeviceError> {
        // Parse physiological data from NDT data packet
        // Format: [timestamp: 8 bytes][num_samples: 4 bytes][channel_data...]
        if data.len() < 12 {
            return Err(DeviceError::InvalidData(
                "Data packet too short".to_string(),
            ));
        }

        let timestamp = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);

        let num_samples = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let mut channel_data = Vec::with_capacity(num_samples);

        let mut offset = 12;
        for _i in 0..num_samples {
            if offset + 4 > data.len() {
                break;
            }

            let channel_id = data[offset];
            let raw_value = u16::from_le_bytes([data[offset + 1], data[offset + 2]]);

            // Apply scaling and offset from channel configuration
            let scaled_value =
                if let Some(config) = self.config.channels.iter().find(|c| c.id == channel_id) {
                    (raw_value as f32) * config.scale + config.offset
                } else {
                    raw_value as f32
                };

            channel_data.push(ChannelData {
                channel_id,
                timestamp,
                value: scaled_value,
                raw_value,
            });

            offset += 4; // channel_id (1) + raw_value (2) + padding (1)
        }

        Ok(channel_data)
    }

    #[allow(dead_code)]
    async fn attempt_reconnect(&mut self) -> Result<(), DeviceError> {
        if !self.device_config.auto_reconnect {
            return Err(DeviceError::ConnectionFailed(
                "Auto-reconnect disabled".to_string(),
            ));
        }

        self.reconnect_attempts += 1;
        warn!(
            "Attempting to reconnect to Biopac (attempt {})",
            self.reconnect_attempts
        );

        sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;

        match self.connect().await {
            Ok(()) => {
                self.reconnect_attempts = 0;
                info!("Successfully reconnected to Biopac");
                Ok(())
            }
            Err(e) => {
                error!(
                    "Reconnection attempt {} failed: {}",
                    self.reconnect_attempts, e
                );
                Err(e)
            }
        }
    }
}
