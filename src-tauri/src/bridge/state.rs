use crate::devices::{BoxedDevice, DeviceInfo, DeviceStatus};
use crate::performance::PerformanceMonitor;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Arc<RwLock<BoxedDevice>>>>>,
    pub connections: Arc<DashMap<String, ConnectionInfo>>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub performance_monitor: Arc<PerformanceMonitor>,
    pub start_time: Instant,
    pub message_count: Arc<AtomicU64>,
    pub last_error: Arc<RwLock<Option<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub client_id: String,
    pub connected_at: u64,
    pub last_activity: u64,
    pub message_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub total_messages: u64,
    pub total_errors: u64,
    pub total_connections: u64,
    pub uptime_seconds: u64,
    pub device_metrics: Vec<DeviceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub device_id: String,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub errors: u64,
    pub last_latency_ms: f64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            performance_monitor: Arc::new(PerformanceMonitor::new()),
            start_time: Instant::now(),
            message_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn add_device(&self, id: String, device: BoxedDevice) {
        let mut devices = self.devices.write().await;
        devices.insert(id.clone(), Arc::new(RwLock::new(device)));

        // Add device to performance monitoring
        self.performance_monitor.add_device(id).await;
    }

    pub async fn remove_device(&self, id: &str) -> Option<Arc<RwLock<BoxedDevice>>> {
        let mut devices = self.devices.write().await;
        let result = devices.remove(id);

        // Remove device from performance monitoring
        self.performance_monitor.remove_device(id).await;

        result
    }

    pub async fn get_device(&self, id: &str) -> Option<Arc<RwLock<BoxedDevice>>> {
        let devices = self.devices.read().await;
        devices.get(id).cloned()
    }

    pub async fn list_devices(&self) -> Vec<DeviceInfo> {
        let mut device_infos = Vec::new();
        let devices = self.devices.read().await;

        for (_, device_lock) in devices.iter() {
            let device = device_lock.read().await;
            device_infos.push(device.get_info());
        }

        device_infos
    }

    pub async fn get_device_status(&self, id: &str) -> Option<DeviceStatus> {
        if let Some(device_lock) = self.get_device(id).await {
            let device = device_lock.read().await;
            Some(device.get_status())
        } else {
            None
        }
    }

    pub fn add_connection(&self, id: String, client_id: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = ConnectionInfo {
            id: id.clone(),
            client_id,
            connected_at: now,
            last_activity: now,
            message_count: 0,
        };

        self.connections.insert(id, info);

        // Record WebSocket connection in performance monitoring
        self.performance_monitor.record_websocket_connection(true);
    }

    pub fn remove_connection(&self, id: &str) {
        self.connections.remove(id);

        // Record WebSocket disconnection in performance monitoring
        self.performance_monitor.record_websocket_connection(false);
    }

    pub fn update_connection_activity(&self, id: &str) {
        if let Some(mut entry) = self.connections.get_mut(id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            entry.last_activity = now;
            entry.message_count += 1;
        }
    }

    pub async fn update_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut Metrics),
    {
        let mut metrics = self.metrics.write().await;
        updater(&mut metrics);
    }

    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }

    pub async fn cleanup_stale_connections(&self, max_idle_seconds: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut to_remove = Vec::new();

        for entry in self.connections.iter() {
            if now - entry.value().last_activity > max_idle_seconds {
                to_remove.push(entry.key().clone());
            }
        }

        for id in to_remove {
            self.remove_connection(&id);
        }
    }

    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub async fn get_message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    pub fn increment_message_count(&self) {
        self.message_count.fetch_add(1, Ordering::Relaxed);

        // Record bridge message in performance monitoring
        self.performance_monitor.record_bridge_message();
    }

    pub async fn set_last_error(&self, error: Option<String>) {
        let mut last_error = self.last_error.write().await;
        *last_error = error;
    }

    pub async fn get_last_error(&self) -> Option<String> {
        self.last_error.read().await.clone()
    }

    pub async fn get_device_metrics(&self, device_id: &str) -> Option<DeviceMetrics> {
        let metrics = self.metrics.read().await;
        metrics
            .device_metrics
            .iter()
            .find(|m| m.device_id == device_id)
            .cloned()
    }

    /// Record device operation with performance tracking
    pub async fn record_device_operation(
        &self,
        device_id: &str,
        latency: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        self.performance_monitor
            .record_device_operation(device_id, latency, bytes_sent, bytes_received)
            .await;
    }

    /// Record device error with performance tracking
    pub async fn record_device_error(&self, device_id: &str, error_msg: &str) {
        self.performance_monitor
            .record_device_error(device_id, error_msg)
            .await;
        self.set_last_error(Some(error_msg.to_string())).await;
    }

    /// Record device connection attempt with performance tracking
    pub async fn record_connection_attempt(&self, device_id: &str, success: bool) {
        self.performance_monitor
            .record_connection_attempt(device_id, success)
            .await;
    }

    /// Get comprehensive performance metrics
    pub async fn get_performance_metrics(&self) -> crate::performance::PerformanceMetrics {
        self.performance_monitor.get_metrics().await
    }

    /// Get device-specific performance metrics
    pub async fn get_device_performance_metrics(
        &self,
        device_id: &str,
    ) -> Option<crate::performance::DevicePerformanceMetrics> {
        self.performance_monitor.get_device_metrics(device_id).await
    }

    /// Get performance summary for monitoring dashboard
    pub async fn get_performance_summary(&self) -> serde_json::Value {
        self.performance_monitor.get_performance_summary().await
    }

    /// Check TTL latency compliance (<1ms)
    pub async fn check_ttl_latency_compliance(&self, device_id: &str) -> Option<bool> {
        self.performance_monitor
            .check_ttl_latency_compliance(device_id)
            .await
    }
}
