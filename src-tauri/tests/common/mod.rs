use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use hyperstudy_bridge::bridge::state::Metrics;
use hyperstudy_bridge::bridge::{AppState, BridgeCommand, BridgeResponse, BridgeServer};
use hyperstudy_bridge::devices::{
    Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType,
};
use hyperstudy_bridge::performance::PerformanceMonitor;
use rand::Rng;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Mock device implementation for testing
#[derive(Debug, Clone)]
pub struct TestMockDevice {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub status: Arc<RwLock<DeviceStatus>>,
    pub config: Arc<RwLock<DeviceConfig>>,
    pub sent_data: Arc<RwLock<Vec<Vec<u8>>>>,
    pub received_data: Arc<RwLock<Vec<Vec<u8>>>>,
    pub connection_delay: Duration,
    pub operation_delay: Duration,
    pub should_fail: Arc<AtomicBool>,
    pub error_rate: f64, // 0.0 to 1.0
    pub latency_ms: u64,
    pub last_operation: Arc<Mutex<Option<Instant>>>,
}

impl TestMockDevice {
    pub fn new(id: String, name: String, device_type: DeviceType) -> Self {
        Self {
            id,
            name,
            device_type,
            status: Arc::new(RwLock::new(DeviceStatus::Disconnected)),
            config: Arc::new(RwLock::new(DeviceConfig::default())),
            sent_data: Arc::new(RwLock::new(Vec::new())),
            received_data: Arc::new(RwLock::new(Vec::new())),
            connection_delay: Duration::from_millis(10),
            operation_delay: Duration::from_millis(1),
            should_fail: Arc::new(AtomicBool::new(false)),
            error_rate: 0.0,
            latency_ms: 1,
            last_operation: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self.operation_delay = Duration::from_millis(latency_ms);
        self
    }

    pub fn with_error_rate(mut self, error_rate: f64) -> Self {
        self.error_rate = error_rate;
        self
    }

    pub fn with_connection_delay(mut self, delay: Duration) -> Self {
        self.connection_delay = delay;
        self
    }

    pub fn set_should_fail(&self, should_fail: bool) {
        self.should_fail.store(should_fail, Ordering::Relaxed);
    }

    pub async fn get_sent_data(&self) -> Vec<Vec<u8>> {
        self.sent_data.read().await.clone()
    }

    pub async fn get_received_data(&self) -> Vec<Vec<u8>> {
        self.received_data.read().await.clone()
    }

    pub async fn add_received_data(&self, data: Vec<u8>) {
        self.received_data.write().await.push(data);
    }

    fn should_simulate_error(&self) -> bool {
        if self.should_fail.load(Ordering::Relaxed) {
            return true;
        }

        if self.error_rate > 0.0 {
            let mut rng = rand::thread_rng();
            rng.gen::<f64>() < self.error_rate
        } else {
            false
        }
    }
}

#[async_trait]
impl Device for TestMockDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::ConnectionFailed(
                "Simulated connection failure".to_string(),
            ));
        }

        tokio::time::sleep(self.connection_delay).await;
        *self.status.write().await = DeviceStatus::Connected;
        *self.last_operation.lock().await = Some(Instant::now());
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::CommunicationError(
                "Simulated disconnect failure".to_string(),
            ));
        }

        tokio::time::sleep(Duration::from_millis(5)).await;
        *self.status.write().await = DeviceStatus::Disconnected;
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::CommunicationError(
                "Simulated send failure".to_string(),
            ));
        }

        let status = *self.status.read().await;
        if status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        tokio::time::sleep(self.operation_delay).await;
        self.sent_data.write().await.push(data.to_vec());
        *self.last_operation.lock().await = Some(Instant::now());
        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::CommunicationError(
                "Simulated receive failure".to_string(),
            ));
        }

        let status = *self.status.read().await;
        if status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        tokio::time::sleep(self.operation_delay).await;

        let mut received_data = self.received_data.write().await;
        if !received_data.is_empty() {
            Ok(received_data.remove(0))
        } else {
            Ok(b"mock_response".to_vec())
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            device_type: self.device_type,
            status: *self
                .status
                .try_read()
                .unwrap_or_else(|_| DeviceStatus::Error.into()),
            metadata: json!({
                "latency_ms": self.latency_ms,
                "error_rate": self.error_rate,
                "test_device": true
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        *self
            .status
            .try_read()
            .unwrap_or_else(|_| DeviceStatus::Error.into())
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::ConfigurationError(
                "Simulated configuration failure".to_string(),
            ));
        }

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *self.config.write().await = config;
            });
        });
        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.should_simulate_error() {
            return Err(DeviceError::Timeout);
        }

        let status = *self.status.read().await;
        if status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        *self.last_operation.lock().await = Some(Instant::now());
        Ok(())
    }
}

/// WebSocket test client for integration testing
pub struct TestWebSocketClient {
    pub ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    pub sent_messages: Arc<RwLock<Vec<String>>>,
    pub received_messages: Arc<RwLock<Vec<BridgeResponse>>>,
    pub is_connected: Arc<AtomicBool>,
}

impl TestWebSocketClient {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (ws_stream, _) = connect_async(url).await?;

        Ok(Self {
            ws_stream,
            sent_messages: Arc::new(RwLock::new(Vec::new())),
            received_messages: Arc::new(RwLock::new(Vec::new())),
            is_connected: Arc::new(AtomicBool::new(true)),
        })
    }

    pub async fn send_command(
        &mut self,
        command: BridgeCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json_str = serde_json::to_string(&command)?;
        self.sent_messages.write().await.push(json_str.clone());
        self.ws_stream.send(Message::Text(json_str)).await?;
        Ok(())
    }

    pub async fn receive_response(
        &mut self,
    ) -> Result<Option<BridgeResponse>, Box<dyn std::error::Error>> {
        if let Some(msg) = self.ws_stream.next().await {
            match msg? {
                Message::Text(text) => {
                    let response: BridgeResponse = serde_json::from_str(&text)?;
                    self.received_messages.write().await.push(response.clone());
                    Ok(Some(response))
                }
                Message::Close(_) => {
                    self.is_connected.store(false, Ordering::Relaxed);
                    Ok(None)
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub async fn wait_for_response(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<BridgeResponse>, Box<dyn std::error::Error>> {
        tokio::time::timeout(timeout, self.receive_response()).await?
    }

    pub async fn get_sent_messages(&self) -> Vec<String> {
        self.sent_messages.read().await.clone()
    }

    pub async fn get_received_messages(&self) -> Vec<BridgeResponse> {
        self.received_messages.read().await.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    pub async fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ws_stream.send(Message::Close(None)).await?;
        self.is_connected.store(false, Ordering::Relaxed);
        Ok(())
    }
}

/// Test data generator for various scenarios
pub struct TestDataGenerator {
    rng: rand::rngs::ThreadRng,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        Self {
            rng: rand::thread_rng(),
        }
    }

    pub fn generate_device_id(&mut self) -> String {
        format!("test_device_{}", Uuid::new_v4())
    }

    pub fn generate_ttl_command(&mut self) -> BridgeCommand {
        BridgeCommand::Command {
            device: "ttl".to_string(),
            action: hyperstudy_bridge::bridge::message::CommandAction::Send,
            payload: Some(json!({"command": "PULSE"})),
            id: Some(self.generate_request_id()),
        }
    }

    pub fn generate_connect_command(&mut self, device_type: &str) -> BridgeCommand {
        let payload = match device_type {
            "ttl" => Some(json!({"port": "/dev/ttyUSB0"})),
            "kernel" => Some(json!({"ip": "127.0.0.1"})),
            "pupil" => Some(json!({"url": "localhost:8081"})),
            "biopac" => Some(json!({"address": "localhost"})),
            "mock" => Some(json!({})),
            _ => None,
        };

        BridgeCommand::Command {
            device: device_type.to_string(),
            action: hyperstudy_bridge::bridge::message::CommandAction::Connect,
            payload,
            id: Some(self.generate_request_id()),
        }
    }

    pub fn generate_disconnect_command(&mut self, device_type: &str) -> BridgeCommand {
        BridgeCommand::Command {
            device: device_type.to_string(),
            action: hyperstudy_bridge::bridge::message::CommandAction::Disconnect,
            payload: None,
            id: Some(self.generate_request_id()),
        }
    }

    pub fn generate_status_query(&mut self, device_type: &str) -> BridgeCommand {
        BridgeCommand::Command {
            device: device_type.to_string(),
            action: hyperstudy_bridge::bridge::message::CommandAction::Status,
            payload: None,
            id: Some(self.generate_request_id()),
        }
    }

    pub fn generate_random_data(&mut self, size: usize) -> Vec<u8> {
        (0..size).map(|_| self.rng.gen()).collect()
    }

    pub fn generate_request_id(&mut self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn generate_large_message(&mut self, size_kb: usize) -> Value {
        let data = "x".repeat(size_kb * 1024);
        json!({"large_data": data})
    }
}

/// Performance measurement utilities
pub struct PerformanceMeasurement {
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub operation_name: String,
}

impl PerformanceMeasurement {
    pub fn start(operation_name: String) -> Self {
        Self {
            start_time: Instant::now(),
            end_time: None,
            operation_name,
        }
    }

    pub fn stop(&mut self) -> Duration {
        self.end_time = Some(Instant::now());
        self.duration()
    }

    pub fn duration(&self) -> Duration {
        if let Some(end) = self.end_time {
            end.duration_since(self.start_time)
        } else {
            Instant::now().duration_since(self.start_time)
        }
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration().as_secs_f64() * 1000.0
    }

    pub fn duration_ns(&self) -> u128 {
        self.duration().as_nanos()
    }

    pub fn is_within_threshold(&self, threshold_ms: f64) -> bool {
        self.duration_ms() <= threshold_ms
    }
}

/// Memory leak detection utilities
pub struct MemoryTracker {
    initial_memory: u64,
    peak_memory: u64,
    measurements: Vec<(Instant, u64)>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        let initial = Self::get_memory_usage();
        Self {
            initial_memory: initial,
            peak_memory: initial,
            measurements: vec![(Instant::now(), initial)],
        }
    }

    pub fn measure(&mut self) {
        let current = Self::get_memory_usage();
        self.peak_memory = self.peak_memory.max(current);
        self.measurements.push((Instant::now(), current));
    }

    pub fn get_memory_usage() -> u64 {
        // Simple memory usage estimation - in a real implementation,
        // you might use more sophisticated memory profiling
        use sysinfo::System;
        let mut system = System::new_all();
        system.refresh_memory();
        system.used_memory()
    }

    pub fn memory_increase(&self) -> u64 {
        self.peak_memory.saturating_sub(self.initial_memory)
    }

    pub fn has_memory_leak(&self, threshold_mb: u64) -> bool {
        let increase_mb = self.memory_increase() / 1024 / 1024;
        increase_mb > threshold_mb
    }

    pub fn get_measurements(&self) -> &[(Instant, u64)] {
        &self.measurements
    }
}

/// Test fixture setup and teardown utilities
pub struct TestFixture {
    pub app_state: Arc<AppState>,
    pub performance_monitor: Arc<PerformanceMonitor>,
    pub temp_devices: Vec<String>,
    pub temp_files: Vec<String>,
}

impl TestFixture {
    pub async fn new() -> Self {
        let app_state = Arc::new(AppState::new());
        let performance_monitor = app_state.performance_monitor.clone();

        Self {
            app_state,
            performance_monitor,
            temp_devices: Vec::new(),
            temp_files: Vec::new(),
        }
    }

    pub async fn add_mock_device(&mut self, device_type: DeviceType) -> String {
        let device_id = format!("test_{:?}_{}", device_type, Uuid::new_v4());
        let device = TestMockDevice::new(
            device_id.clone(),
            format!("Test {:?} Device", device_type),
            device_type,
        );

        self.app_state
            .add_device(device_id.clone(), Box::new(device))
            .await;
        self.temp_devices.push(device_id.clone());
        device_id
    }

    pub async fn add_high_latency_device(
        &mut self,
        device_type: DeviceType,
        latency_ms: u64,
    ) -> String {
        let device_id = format!("test_{:?}_slow_{}", device_type, Uuid::new_v4());
        let device = TestMockDevice::new(
            device_id.clone(),
            format!("Slow Test {:?} Device", device_type),
            device_type,
        )
        .with_latency(latency_ms);

        self.app_state
            .add_device(device_id.clone(), Box::new(device))
            .await;
        self.temp_devices.push(device_id.clone());
        device_id
    }

    pub async fn add_unreliable_device(
        &mut self,
        device_type: DeviceType,
        error_rate: f64,
    ) -> String {
        let device_id = format!("test_{:?}_unreliable_{}", device_type, Uuid::new_v4());
        let device = TestMockDevice::new(
            device_id.clone(),
            format!("Unreliable Test {:?} Device", device_type),
            device_type,
        )
        .with_error_rate(error_rate);

        self.app_state
            .add_device(device_id.clone(), Box::new(device))
            .await;
        self.temp_devices.push(device_id.clone());
        device_id
    }

    pub async fn cleanup(&mut self) {
        // Remove all test devices
        for device_id in &self.temp_devices {
            self.app_state.remove_device(device_id).await;
        }
        self.temp_devices.clear();

        // Clean up temporary files
        for file_path in &self.temp_files {
            let _ = tokio::fs::remove_file(file_path).await;
        }
        self.temp_files.clear();
    }

    pub async fn wait_for_device_status(
        &self,
        device_id: &str,
        expected_status: DeviceStatus,
        timeout: Duration,
    ) -> bool {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if let Some(status) = self.app_state.get_device_status(device_id).await {
                if status == expected_status {
                    return true;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    pub async fn get_device_count(&self) -> usize {
        self.app_state.devices.read().await.len()
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        // Cleanup in async context if possible
        tokio::task::block_in_place(|| {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.block_on(async {
                    self.cleanup().await;
                });
            }
        });
    }
}

/// Utility functions for common test operations
pub mod test_utils {
    use super::*;

    /// Wait for a condition to be true with timeout
    pub async fn wait_for_condition<F>(mut condition: F, timeout: Duration) -> bool
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    /// Wait for an async condition to be true with timeout
    pub async fn wait_for_async_condition<F, Fut>(mut condition: F, timeout: Duration) -> bool
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    /// Create multiple mock devices of different types
    pub async fn create_multi_device_setup(
        fixture: &mut TestFixture,
    ) -> HashMap<DeviceType, String> {
        let mut devices = HashMap::new();

        devices.insert(
            DeviceType::TTL,
            fixture.add_mock_device(DeviceType::TTL).await,
        );
        devices.insert(
            DeviceType::Kernel,
            fixture.add_mock_device(DeviceType::Kernel).await,
        );
        devices.insert(
            DeviceType::Pupil,
            fixture.add_mock_device(DeviceType::Pupil).await,
        );
        devices.insert(
            DeviceType::Biopac,
            fixture.add_mock_device(DeviceType::Biopac).await,
        );
        devices.insert(
            DeviceType::Mock,
            fixture.add_mock_device(DeviceType::Mock).await,
        );

        devices
    }

    /// Measure throughput of operations
    pub async fn measure_throughput<F, Fut>(operation: F, duration: Duration) -> (u64, f64)
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let start = Instant::now();
        let mut count = 0u64;

        while start.elapsed() < duration {
            operation().await;
            count += 1;
        }

        let actual_duration = start.elapsed().as_secs_f64();
        let throughput = count as f64 / actual_duration;

        (count, throughput)
    }

    /// Assert that TTL latency is under 1ms
    pub fn assert_ttl_latency_compliance(latency: Duration) {
        assert!(
            latency.as_millis() < 1,
            "TTL latency {} ms exceeds 1ms requirement",
            latency.as_millis()
        );
    }

    /// Assert that throughput meets requirements
    pub fn assert_throughput_compliance(throughput: f64, minimum: f64) {
        assert!(
            throughput >= minimum,
            "Throughput {} msg/sec is below minimum requirement of {} msg/sec",
            throughput,
            minimum
        );
    }

    /// Generate load test scenario
    pub async fn run_load_test<F, Fut>(
        operation: F,
        concurrent_operations: usize,
        operations_per_worker: usize,
    ) -> Vec<Duration>
    where
        F: Fn(usize) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Duration> + Send + 'static,
    {
        let mut handles = Vec::new();

        for worker_id in 0..concurrent_operations {
            let op = operation.clone();
            let handle = tokio::spawn(async move {
                let mut latencies = Vec::new();
                for _ in 0..operations_per_worker {
                    let latency = op(worker_id).await;
                    latencies.push(latency);
                }
                latencies
            });
            handles.push(handle);
        }

        let mut all_latencies = Vec::new();
        for handle in handles {
            let worker_latencies = handle.await.unwrap();
            all_latencies.extend(worker_latencies);
        }

        all_latencies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_device_basic_operations() {
        let mut device = TestMockDevice::new(
            "test_device".to_string(),
            "Test Device".to_string(),
            DeviceType::Mock,
        );

        // Test connection
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
        device.connect().await.unwrap();
        assert_eq!(device.get_status(), DeviceStatus::Connected);

        // Test send/receive
        let test_data = b"test_message";
        device.send(test_data).await.unwrap();
        let sent_data = device.get_sent_data().await;
        assert_eq!(sent_data.len(), 1);
        assert_eq!(sent_data[0], test_data);

        // Test disconnect
        device.disconnect().await.unwrap();
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_mock_device_error_simulation() {
        let mut device = TestMockDevice::new(
            "test_device".to_string(),
            "Test Device".to_string(),
            DeviceType::Mock,
        )
        .with_error_rate(1.0); // 100% error rate

        // Should fail to connect
        assert!(device.connect().await.is_err());
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_performance_measurement() {
        let mut measurement = PerformanceMeasurement::start("test_operation".to_string());

        tokio::time::sleep(Duration::from_millis(50)).await;
        let duration = measurement.stop();

        assert!(duration.as_millis() >= 50);
        assert!(measurement.is_within_threshold(100.0));
        assert!(!measurement.is_within_threshold(10.0));
    }

    #[tokio::test]
    async fn test_test_fixture() {
        let mut fixture = TestFixture::new().await;

        let device_id = fixture.add_mock_device(DeviceType::TTL).await;
        assert_eq!(fixture.get_device_count().await, 1);

        let status = fixture.app_state.get_device_status(&device_id).await;
        assert!(status.is_some());

        fixture.cleanup().await;
        assert_eq!(fixture.get_device_count().await, 0);
    }
}
