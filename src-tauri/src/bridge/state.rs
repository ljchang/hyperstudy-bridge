use crate::devices::lsl::{InletManager, NeonLslManager, StreamResolver, TimeSync};
use crate::devices::{BoxedDevice, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::PerformanceMonitor;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Arc<RwLock<BoxedDevice>>>>>,
    pub connections: Arc<DashMap<String, ConnectionInfo>>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub performance_monitor: Arc<PerformanceMonitor>,
    pub start_time: Instant,
    pub message_count: Arc<AtomicU64>,
    pub last_error: Arc<RwLock<Option<String>>>,
    /// Neon LSL Manager for Pupil Labs Neon eye tracking via LSL
    pub neon_manager: Arc<NeonLslManager>,
    /// Broadcast channel for device status change events
    /// WebSocket connections can subscribe to receive status updates
    device_status_tx: broadcast::Sender<DeviceStatusEvent>,
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

/// Event broadcast when a device's status changes
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatusEvent {
    pub device_id: String,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub reason: String,
    pub timestamp: u64,
}

impl DeviceStatusEvent {
    pub fn disconnected(device_id: String, device_type: DeviceType, reason: &str) -> Self {
        Self {
            device_id,
            device_type,
            status: DeviceStatus::Disconnected,
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("devices", &self.devices)
            .field("connections", &self.connections)
            .field("metrics", &self.metrics)
            .field("start_time", &self.start_time)
            .field("message_count", &self.message_count)
            .field("last_error", &self.last_error)
            .field("device_status_subscribers", &self.device_status_tx.receiver_count())
            .finish()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Capacity for device status broadcast channel
    const STATUS_BROADCAST_CAPACITY: usize = 16;

    pub fn new() -> Self {
        // Create shared LSL infrastructure for Neon manager
        let time_sync = Arc::new(TimeSync::new(true));
        let resolver = Arc::new(StreamResolver::new(5.0));
        let inlet_manager = Arc::new(InletManager::new(time_sync));
        let neon_manager = Arc::new(NeonLslManager::new(resolver, inlet_manager));

        // Create broadcast channel for device status events
        let (device_status_tx, _) = broadcast::channel(Self::STATUS_BROADCAST_CAPACITY);

        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            performance_monitor: Arc::new(PerformanceMonitor::new()),
            start_time: Instant::now(),
            message_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(RwLock::new(None)),
            neon_manager,
            device_status_tx,
        }
    }

    /// Subscribe to device status change events.
    ///
    /// Returns a receiver that will receive `DeviceStatusEvent` notifications
    /// when devices connect, disconnect, or change status.
    pub fn subscribe_device_status(&self) -> broadcast::Receiver<DeviceStatusEvent> {
        self.device_status_tx.subscribe()
    }

    /// Broadcast a device status change event to all subscribers.
    ///
    /// This is used to notify WebSocket clients when device status changes.
    pub fn broadcast_device_status(&self, event: DeviceStatusEvent) {
        // It's OK if there are no subscribers - send returns error but we ignore it
        let _ = self.device_status_tx.send(event);
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

    /// Handle USB disconnect event for TTL device.
    ///
    /// This is called by the USB monitor when it detects that a TTL device
    /// has been physically unplugged. It updates the device status to Disconnected
    /// and removes the device from the active devices map.
    ///
    /// Returns true if a TTL device was found and updated, false otherwise.
    pub async fn handle_ttl_usb_disconnect(&self) -> bool {
        let devices = self.devices.read().await;

        // Find all TTL devices
        let ttl_device_ids: Vec<String> = {
            let mut ids = Vec::new();
            for (id, device_lock) in devices.iter() {
                let device = device_lock.read().await;
                if device.get_info().device_type == DeviceType::TTL {
                    ids.push(id.clone());
                }
            }
            ids
        };
        drop(devices);

        if ttl_device_ids.is_empty() {
            return false;
        }

        let mut any_updated = false;

        // Disconnect each TTL device
        for device_id in ttl_device_ids {
            // Re-verify device exists and is still TTL type (avoid race condition)
            let should_disconnect = if let Some(device_lock) = self.get_device(&device_id).await {
                let device = device_lock.read().await;
                device.get_info().device_type == DeviceType::TTL
            } else {
                // Device was already removed by another task
                false
            };

            if !should_disconnect {
                continue;
            }

            info!(
                device = "ttl",
                "USB disconnect detected, marking device {} as disconnected", device_id
            );

            // Try to call disconnect on the device (best effort - the hardware is gone)
            if let Some(device_lock) = self.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                // The disconnect call may fail since the device is gone, but we try anyway
                let _ = device.disconnect().await;
            }

            // Remove the device from the registry
            self.remove_device(&device_id).await;

            // Record the error
            self.record_device_error(&device_id, "Device physically disconnected (USB unplug detected)")
                .await;

            // Broadcast status change to WebSocket clients
            self.broadcast_device_status(DeviceStatusEvent::disconnected(
                device_id.clone(),
                DeviceType::TTL,
                "USB device unplugged",
            ));

            warn!(
                device = "ttl",
                "TTL device {} removed due to USB disconnect", device_id
            );

            any_updated = true;
        }

        any_updated
    }
}
