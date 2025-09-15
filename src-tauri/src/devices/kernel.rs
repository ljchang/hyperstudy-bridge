use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::measure_latency;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

const DEFAULT_PORT: u16 = 6767;
const HEARTBEAT_COMMAND: &[u8] = b"PING\n";
const HEARTBEAT_RESPONSE: &[u8] = b"PONG\n";
const HEARTBEAT_INTERVAL_MS: u64 = 10000; // 10 seconds
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
const INITIAL_RECONNECT_DELAY_MS: u64 = 1000;
const MAX_RECONNECT_DELAY_MS: u64 = 30000;
const CONNECTION_TIMEOUT_MS: u64 = 5000;
const READ_BUFFER_SIZE: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelConfig {
    pub ip_address: String,
    pub port: u16,
    pub connection_timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub max_reconnect_attempts: u32,
    pub initial_reconnect_delay_ms: u64,
    pub max_reconnect_delay_ms: u64,
    pub buffer_size: usize,
    pub enable_heartbeat: bool,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            ip_address: "127.0.0.1".to_string(),
            port: DEFAULT_PORT,
            connection_timeout_ms: CONNECTION_TIMEOUT_MS,
            heartbeat_interval_ms: HEARTBEAT_INTERVAL_MS,
            max_reconnect_attempts: MAX_RECONNECT_ATTEMPTS,
            initial_reconnect_delay_ms: INITIAL_RECONNECT_DELAY_MS,
            max_reconnect_delay_ms: MAX_RECONNECT_DELAY_MS,
            buffer_size: READ_BUFFER_SIZE,
            enable_heartbeat: true,
        }
    }
}

pub struct KernelDevice {
    socket: Option<TcpStream>,
    status: DeviceStatus,
    config: KernelConfig,
    device_config: DeviceConfig,
    buffer: Vec<u8>,
    reconnect_attempts: u32,
    last_heartbeat: Option<Instant>,
    last_successful_connection: Option<Instant>,
    /// Performance callback for recording metrics
    performance_callback: Option<Box<dyn Fn(&str, Duration, u64, u64) + Send + Sync>>,
}

impl std::fmt::Debug for KernelDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelDevice")
            .field("ip_address", &self.config.ip_address)
            .field("port", &self.config.port)
            .field("status", &self.status)
            .field("config", &self.config)
            .field("device_config", &self.device_config)
            .field("reconnect_attempts", &self.reconnect_attempts)
            .field("last_heartbeat", &self.last_heartbeat)
            .field("last_successful_connection", &self.last_successful_connection)
            .field("has_performance_callback", &self.performance_callback.is_some())
            .finish()
    }
}

impl KernelDevice {
    pub fn new(ip_address: String) -> Self {
        Self {
            socket: None,
            status: DeviceStatus::Disconnected,
            config: KernelConfig {
                ip_address,
                ..Default::default()
            },
            device_config: DeviceConfig::default(),
            buffer: Vec::with_capacity(READ_BUFFER_SIZE),
            reconnect_attempts: 0,
            last_heartbeat: None,
            last_successful_connection: None,
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

    /// Calculate exponential backoff delay
    fn calculate_backoff_delay(&self) -> Duration {
        let base_delay = self.config.initial_reconnect_delay_ms;
        let max_delay = self.config.max_reconnect_delay_ms;

        // Exponential backoff: base * 2^attempts, capped at max_delay
        let delay_ms = (base_delay * (2_u64.pow(self.reconnect_attempts.min(10))))
            .min(max_delay);

        Duration::from_millis(delay_ms)
    }

    /// Attempt to establish TCP connection with timeout
    async fn establish_connection(&mut self) -> Result<TcpStream, DeviceError> {
        let addr = format!("{}:{}", self.config.ip_address, self.config.port);
        let connection_timeout = Duration::from_millis(self.config.connection_timeout_ms);

        debug!("Attempting to connect to Kernel Flow2 at {}", addr);

        match timeout(connection_timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(socket)) => {
                info!("Successfully established connection to Kernel Flow2 at {}", addr);
                Ok(socket)
            }
            Ok(Err(e)) => {
                error!("TCP connection failed to {}: {}", addr, e);
                Err(DeviceError::ConnectionFailed(format!("TCP connection failed: {}", e)))
            }
            Err(_) => {
                error!("Connection attempt to {} timed out after {:?}", addr, connection_timeout);
                Err(DeviceError::Timeout)
            }
        }
    }

    /// Reconnect with exponential backoff
    async fn attempt_reconnect(&mut self) -> Result<(), DeviceError> {
        if self.reconnect_attempts >= self.config.max_reconnect_attempts {
            error!("Maximum reconnection attempts ({}) reached for Kernel Flow2", self.config.max_reconnect_attempts);
            self.status = DeviceStatus::Error;
            return Err(DeviceError::ConnectionFailed("Maximum reconnection attempts reached".to_string()));
        }

        self.reconnect_attempts += 1;
        let backoff_delay = self.calculate_backoff_delay();

        warn!("Reconnection attempt {} of {} for Kernel Flow2, waiting {:?}",
              self.reconnect_attempts, self.config.max_reconnect_attempts, backoff_delay);

        sleep(backoff_delay).await;

        match self.establish_connection().await {
            Ok(socket) => {
                self.socket = Some(socket);
                self.status = DeviceStatus::Connected;
                self.last_successful_connection = Some(Instant::now());
                self.last_heartbeat = Some(Instant::now());
                self.reconnect_attempts = 0; // Reset on successful connection
                info!("Kernel Flow2 reconnection successful");
                Ok(())
            }
            Err(e) => {
                warn!("Reconnection attempt {} failed: {}", self.reconnect_attempts, e);
                Err(e)
            }
        }
    }

    /// Check if heartbeat is needed and send if necessary
    async fn check_heartbeat(&mut self) -> Result<(), DeviceError> {
        if !self.config.enable_heartbeat || self.socket.is_none() {
            return Ok(());
        }

        let should_heartbeat = match self.last_heartbeat {
            Some(last) => {
                let elapsed = last.elapsed();
                elapsed > Duration::from_millis(self.config.heartbeat_interval_ms)
            }
            None => true,
        };

        if should_heartbeat {
            debug!("Sending heartbeat to Kernel Flow2");
            match self.send_heartbeat().await {
                Ok(_) => {
                    self.last_heartbeat = Some(Instant::now());
                    debug!("Heartbeat sent successfully");
                    Ok(())
                }
                Err(e) => {
                    warn!("Heartbeat failed, connection may be lost: {}", e);
                    self.status = DeviceStatus::Error;
                    Err(e)
                }
            }
        } else {
            Ok(())
        }
    }

    /// Send heartbeat ping
    async fn send_heartbeat(&mut self) -> Result<(), DeviceError> {
        if let Some(ref mut socket) = self.socket {
            let device_id = self.get_info().id;
            let result = socket.write_all(HEARTBEAT_COMMAND).await
                .and_then(|_| socket.flush().await)
                .map_err(|e| DeviceError::CommunicationError(format!("Heartbeat failed: {}", e)));

            // Record performance metrics (simplified for borrowing)
            if let Some(ref callback) = self.performance_callback {
                callback(&device_id, Duration::from_nanos(0), HEARTBEAT_COMMAND.len() as u64, 0);
            }

            result
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    /// Detect connection health and handle disconnections
    fn is_connection_healthy(&self) -> bool {
        if self.socket.is_none() {
            return false;
        }

        // Check if too much time has passed since last successful operation
        if let Some(last_connection) = self.last_successful_connection {
            let connection_age = last_connection.elapsed();
            // Consider connection stale if no activity for extended period
            if connection_age > Duration::from_millis(self.config.heartbeat_interval_ms * 3) {
                return false;
            }
        }

        true
    }

    /// Check if an IO error indicates connection loss
    fn is_io_error_connection_lost(&self, error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
        )
    }

    /// Get connection uptime
    pub fn get_connection_uptime(&self) -> Option<Duration> {
        self.last_successful_connection.map(|t| t.elapsed())
    }

    /// Get current reconnection attempt count
    pub fn get_reconnect_attempts(&self) -> u32 {
        self.reconnect_attempts
    }

    /// Reset reconnection attempts counter
    pub fn reset_reconnect_attempts(&mut self) {
        self.reconnect_attempts = 0;
    }

    /// Check if device is currently connected
    pub fn is_connected(&self) -> bool {
        self.socket.is_some() && self.status == DeviceStatus::Connected
    }

    /// Get kernel-specific configuration
    pub fn get_kernel_config(&self) -> &KernelConfig {
        &self.config
    }

    /// Update kernel-specific configuration
    pub fn update_kernel_config(&mut self, config: KernelConfig) {
        self.config = config;
        // Resize buffer if size changed
        if self.buffer.capacity() < self.config.buffer_size {
            self.buffer.reserve(self.config.buffer_size - self.buffer.capacity());
        }
    }
}

#[async_trait]
impl Device for KernelDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting to Kernel Flow2 at {}:{}", self.config.ip_address, self.config.port);
        self.status = DeviceStatus::Connecting;
        self.reconnect_attempts = 0; // Reset reconnection attempts on new connection

        match self.establish_connection().await {
            Ok(socket) => {
                self.socket = Some(socket);
                self.status = DeviceStatus::Connected;
                self.last_successful_connection = Some(Instant::now());
                self.last_heartbeat = Some(Instant::now());
                self.reconnect_attempts = 0;
                info!("Successfully connected to Kernel Flow2");
                Ok(())
            }
            Err(e) => {
                self.status = DeviceStatus::Error;
                error!("Failed to connect to Kernel Flow2: {}", e);
                Err(e)
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting from Kernel Flow2");

        if let Some(mut socket) = self.socket.take() {
            // Try to gracefully shutdown the connection
            if let Err(e) = socket.shutdown().await {
                warn!("Error during graceful shutdown: {}", e);
            }
        }

        self.status = DeviceStatus::Disconnected;
        self.buffer.clear();
        self.last_heartbeat = None;
        self.last_successful_connection = None;
        self.reconnect_attempts = 0;

        info!("Kernel Flow2 disconnected successfully");
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        // Check connection health before sending
        if !self.is_connection_healthy() {
            warn!("Connection unhealthy, attempting reconnection before send");
            if self.device_config.auto_reconnect {
                self.attempt_reconnect().await?;
            } else {
                return Err(DeviceError::NotConnected);
            }
        }

        if let Some(ref mut socket) = self.socket {
            let device_id = self.get_info().id;

            // Measure latency for the operation
            let start = Instant::now();
            let result = socket.write_all(data).await;
            let latency = start.elapsed();

            match result {
                Ok(_) => {
                    if let Err(flush_err) = socket.flush().await {
                        if self.is_io_error_connection_lost(&flush_err) {
                            self.socket = None;
                            self.status = DeviceStatus::Error;
                            return Err(DeviceError::ConnectionFailed(format!("Connection lost during flush: {}", flush_err)));
                        } else {
                            return Err(DeviceError::CommunicationError(format!("Flush failed: {}", flush_err)));
                        }
                    }

                    // Record success
                    self.last_successful_connection = Some(Instant::now());

                    // Record performance metrics
                    if let Some(ref callback) = self.performance_callback {
                        callback(&device_id, latency, data.len() as u64, 0);
                    }

                    debug!("Kernel Flow2 data sent successfully: {} bytes with latency {:?}", data.len(), latency);
                    Ok(())
                }
                Err(send_err) => {
                    if self.is_io_error_connection_lost(&send_err) {
                        self.socket = None;
                        self.status = DeviceStatus::Error;
                        Err(DeviceError::ConnectionFailed(format!("Connection lost during send: {}", send_err)))
                    } else {
                        Err(DeviceError::CommunicationError(format!("Send failed: {}", send_err)))
                    }
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        // Check connection health before receiving
        if !self.is_connection_healthy() {
            warn!("Connection unhealthy, attempting reconnection before receive");
            if self.device_config.auto_reconnect {
                self.attempt_reconnect().await?;
            } else {
                return Err(DeviceError::NotConnected);
            }
        }

        if let Some(ref mut socket) = self.socket {
            let device_id = self.get_info().id;

            // Prepare buffer for reading
            self.buffer.clear();
            self.buffer.resize(self.config.buffer_size, 0);

            // Measure latency for the read operation
            let start = Instant::now();
            let read_result = socket.read(&mut self.buffer).await;
            let latency = start.elapsed();

            match read_result {
                Ok(0) => {
                    // Connection closed by remote
                    warn!("Kernel Flow2 connection closed by remote");
                    self.status = DeviceStatus::Error;
                    self.socket = None;
                    Err(DeviceError::ConnectionFailed("Connection closed by remote".to_string()))
                }
                Ok(n) => {
                    self.buffer.truncate(n);
                    debug!("Kernel Flow2 received {} bytes", n);

                    let data = self.buffer.clone();

                    // Record success
                    if !data.is_empty() {
                        self.last_successful_connection = Some(Instant::now());
                    }

                    // Record performance metrics
                    if let Some(ref callback) = self.performance_callback {
                        callback(&device_id, latency, 0, data.len() as u64);
                    }

                    Ok(data)
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Timeout is acceptable, just return empty data
                    Ok(Vec::new())
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Non-blocking read, no data available
                    Ok(Vec::new())
                }
                Err(e) => {
                    error!("Kernel Flow2 read error: {}", e);
                    if self.is_io_error_connection_lost(&e) {
                        self.socket = None;
                        self.status = DeviceStatus::Error;
                        Err(DeviceError::ConnectionFailed(format!("Connection lost during read: {}", e)))
                    } else {
                        Err(DeviceError::CommunicationError(format!("Read failed: {}", e)))
                    }
                }
            }
        } else {
            Err(DeviceError::NotConnected)
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: format!("kernel_{}_{}", self.config.ip_address.replace('.', "_"), self.config.port),
            name: format!("Kernel Flow2 ({}:{})", self.config.ip_address, self.config.port),
            device_type: DeviceType::Kernel,
            status: self.status,
            metadata: serde_json::json!({
                "ip_address": self.config.ip_address,
                "port": self.config.port,
                "connection_timeout_ms": self.config.connection_timeout_ms,
                "heartbeat_interval_ms": self.config.heartbeat_interval_ms,
                "max_reconnect_attempts": self.config.max_reconnect_attempts,
                "buffer_size": self.config.buffer_size,
                "enable_heartbeat": self.config.enable_heartbeat,
                "reconnect_attempts": self.reconnect_attempts,
                "last_heartbeat": self.last_heartbeat.map(|t| t.elapsed().as_secs()),
                "last_successful_connection": self.last_successful_connection.map(|t| t.elapsed().as_secs()),
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.device_config = config;

        if let Some(custom) = self.device_config.custom_settings.as_object() {
            if let Some(ip) = custom.get("ip_address").and_then(|v| v.as_str()) {
                self.config.ip_address = ip.to_string();
            }

            if let Some(port) = custom.get("port").and_then(|v| v.as_u64()) {
                self.config.port = port as u16;
            }

            if let Some(timeout) = custom.get("connection_timeout_ms").and_then(|v| v.as_u64()) {
                self.config.connection_timeout_ms = timeout;
            }

            if let Some(heartbeat_interval) = custom.get("heartbeat_interval_ms").and_then(|v| v.as_u64()) {
                self.config.heartbeat_interval_ms = heartbeat_interval;
            }

            if let Some(max_attempts) = custom.get("max_reconnect_attempts").and_then(|v| v.as_u64()) {
                self.config.max_reconnect_attempts = max_attempts as u32;
            }

            if let Some(buffer_size) = custom.get("buffer_size").and_then(|v| v.as_u64()) {
                self.config.buffer_size = buffer_size as usize;
                // Resize buffer to new size
                self.buffer.reserve(self.config.buffer_size);
            }

            if let Some(enable_heartbeat) = custom.get("enable_heartbeat").and_then(|v| v.as_bool()) {
                self.config.enable_heartbeat = enable_heartbeat;
            }
        }

        info!("Kernel Flow2 device configured: {}:{}", self.config.ip_address, self.config.port);
        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        // Check connection health and perform heartbeat if needed
        if self.socket.is_some() {
            // Perform heartbeat check
            self.check_heartbeat().await?;

            // If auto-reconnect is enabled and connection is unhealthy, try to reconnect
            if !self.is_connection_healthy() && self.device_config.auto_reconnect {
                warn!("Connection health check failed, attempting automatic reconnection");
                self.attempt_reconnect().await?;
            }

            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}