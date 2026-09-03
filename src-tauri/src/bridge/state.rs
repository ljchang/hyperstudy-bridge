use crate::devices::lsl::{
    FrenzLslManager, FrenzProcessManager, InletManager, NeonLslManager, OutletManager,
    StreamResolver, TimeSync,
};
use crate::devices::{BoxedDevice, DeviceInfo, DeviceStatus, DeviceType};
use crate::performance::PerformanceMonitor;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
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
    /// FRENZ LSL Manager for Earable FRENZ brainband via LSL
    pub frenz_manager: Arc<FrenzLslManager>,
    /// FRENZ Python bridge process manager (PyApp lifecycle)
    pub frenz_process: Arc<FrenzProcessManager>,
    /// EyeLink manager for SR Research EyeLink 1000 Plus eye tracking
    pub eyelink_manager: Arc<crate::devices::eyelink::EyeLinkManager>,
    /// Broadcast channel for device status change events
    /// WebSocket connections can subscribe to receive status updates
    device_status_tx: broadcast::Sender<DeviceStatusEvent>,
    /// Last successful `connect` payload per device id, so a device that
    /// dropped out (or was never re-added after a bridge restart) can be
    /// re-connected on demand when a client sends to it.
    pub last_connect_configs: Arc<DashMap<String, serde_json::Value>>,
    /// Neon phone device_id -> recording id this bridge started. Ownership
    /// otherwise lives only inside the PupilDevice instance and is lost on a
    /// disconnect/reconnect or bridge restart, after which the bridge's own
    /// recording reads as "busy" / "not owned" until someone passes force.
    pub owned_recordings: Arc<DashMap<String, String>>,
    /// Per device id, the ticket of the most recently *started* connect. A
    /// connect may only register its device if it still holds the latest
    /// ticket, so a slow connect from an older connection can never overwrite
    /// what a newer connect registered — regardless of whether the server has
    /// observed the old socket's close yet.
    pub connect_tickets: Arc<DashMap<String, u64>>,
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
    /// Device metadata at the time of the change (Neon device_id/IP, recording
    /// state…) so every client — not just the one that issued the command —
    /// can see *which* hardware is behind the status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

impl DeviceStatusEvent {
    pub fn disconnected(device_id: String, device_type: DeviceType, reason: &str) -> Self {
        Self::with_status(
            device_id,
            device_type,
            DeviceStatus::Disconnected,
            reason,
            None,
        )
    }

    pub fn connected(device_id: String, device_type: DeviceType, reason: &str) -> Self {
        Self::with_status(
            device_id,
            device_type,
            DeviceStatus::Connected,
            reason,
            None,
        )
    }

    pub fn connected_with_info(
        device_id: String,
        device_type: DeviceType,
        reason: &str,
        info: serde_json::Value,
    ) -> Self {
        Self::with_status(
            device_id,
            device_type,
            DeviceStatus::Connected,
            reason,
            Some(info),
        )
    }

    fn with_status(
        device_id: String,
        device_type: DeviceType,
        status: DeviceStatus,
        reason: &str,
        info: Option<serde_json::Value>,
    ) -> Self {
        Self {
            device_id,
            device_type,
            status,
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            info,
        }
    }
}

/// Extract `(phone device_id, active recording id, owned)` from a Pupil
/// device's `get_info().metadata`. Pure so it can be tested without a device.
pub fn pupil_recording_state_from_info(
    info: &serde_json::Value,
) -> Option<(String, Option<String>, bool)> {
    let phone = info.get("device_id")?.as_str()?.to_string();
    let recording = info
        .get("recording_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let owned = info
        .get("recording_owned")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Some((phone, recording, owned))
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("AppState");
        s.field("devices", &self.devices)
            .field("connections", &self.connections)
            .field("metrics", &self.metrics)
            .field("start_time", &self.start_time)
            .field("message_count", &self.message_count)
            .field("last_error", &self.last_error)
            .field("neon_manager", &self.neon_manager)
            .field("frenz_manager", &self.frenz_manager)
            .field("frenz_process", &self.frenz_process)
            .field("eyelink_manager", &"EyeLinkManager");
        s.field(
            "device_status_subscribers",
            &self.device_status_tx.receiver_count(),
        )
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
        // Create shared LSL infrastructure for Neon and FRENZ managers
        let time_sync = Arc::new(TimeSync::new(true));
        let resolver = Arc::new(StreamResolver::new(5.0));
        let inlet_manager = Arc::new(InletManager::new(time_sync.clone()));
        let outlet_manager = Arc::new(OutletManager::new(time_sync));
        let neon_manager = Arc::new(NeonLslManager::new(resolver.clone(), inlet_manager.clone()));
        let frenz_manager = Arc::new(FrenzLslManager::new(
            resolver,
            inlet_manager,
            outlet_manager,
        ));
        let frenz_process = Arc::new(FrenzProcessManager::new());

        let eyelink_manager = Arc::new(crate::devices::eyelink::EyeLinkManager::new());

        // Create broadcast channel for device status events
        let (device_status_tx, _) = broadcast::channel(Self::STATUS_BROADCAST_CAPACITY);

        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(DashMap::new()),
            last_connect_configs: Arc::new(DashMap::new()),
            owned_recordings: Arc::new(DashMap::new()),
            connect_tickets: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            performance_monitor: Arc::new(PerformanceMonitor::new()),
            start_time: Instant::now(),
            message_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(RwLock::new(None)),
            neon_manager,
            frenz_manager,
            frenz_process,
            eyelink_manager,
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

    /// Register `device` only if nothing is registered under `id` yet. Returns
    /// false (and drops `device`) when another connect won the race — the
    /// auto-reconnect path must never replace a device an explicit connect
    /// just registered.
    pub async fn add_device_if_absent(&self, id: String, device: BoxedDevice) -> bool {
        let mut devices = self.devices.write().await;
        if devices.contains_key(&id) {
            return false;
        }
        devices.insert(id.clone(), Arc::new(RwLock::new(device)));
        drop(devices);
        self.performance_monitor.add_device(id).await;
        true
    }

    /// Track which Neon recording this bridge owns, from a Pupil device's
    /// metadata after a command (see `pupil_recording_state_from_info`).
    pub fn note_pupil_recording_state(&self, info: &serde_json::Value) {
        if let Some((phone, recording, owned)) = pupil_recording_state_from_info(info) {
            match recording {
                Some(id) if owned => {
                    self.owned_recordings.insert(phone, id);
                }
                Some(_) => {}
                None => {
                    self.owned_recordings.remove(&phone);
                }
            }
        }
    }

    /// The recording id this bridge started on `phone`, if still remembered.
    pub fn owned_recording(&self, phone: &str) -> Option<String> {
        self.owned_recordings.get(phone).map(|v| v.value().clone())
    }

    /// Start a connect for `id`: returns a ticket that identifies this attempt
    /// as the newest. Call before any await in the connect path.
    pub fn begin_connect(&self, id: &str) -> u64 {
        let mut entry = self.connect_tickets.entry(id.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Register `device` under `id` only if `ticket` is still the newest connect
    /// for that id and the owning connection has not closed. Decided under the
    /// registry lock. Overlapping connections are ordered by ticket, not by
    /// when the server happened to notice a socket close: a connect started
    /// later always wins over one started earlier, even if the earlier one
    /// finishes last. Returns the device back on refusal so the caller can
    /// release it.
    pub async fn add_device_if_latest(
        &self,
        id: String,
        device: BoxedDevice,
        ticket: u64,
        closed: &AtomicBool,
    ) -> Result<(), BoxedDevice> {
        let mut devices = self.devices.write().await;
        let latest = self.connect_tickets.get(&id).map(|v| *v).unwrap_or(0);
        if closed.load(std::sync::atomic::Ordering::Relaxed) || ticket != latest {
            return Err(device);
        }
        devices.insert(id.clone(), Arc::new(RwLock::new(device)));
        drop(devices);
        self.performance_monitor.add_device(id).await;
        Ok(())
    }

    /// Remove the registration for `id` only if it is still `expected` — the
    /// exact device a caller disconnected — so a slow disconnect from an old
    /// connection cannot unregister the fresh device a newer one registered.
    pub async fn remove_device_if_same(
        &self,
        id: &str,
        expected: &Arc<RwLock<BoxedDevice>>,
    ) -> bool {
        let mut devices = self.devices.write().await;
        match devices.get(id) {
            Some(current) if Arc::ptr_eq(current, expected) => {
                devices.remove(id);
                drop(devices);
                self.performance_monitor.remove_device(id).await;
                true
            }
            _ => false,
        }
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
    /// Remember the payload that last connected `device_id` successfully.
    pub fn remember_connect_config(&self, device_id: &str, config: serde_json::Value) {
        self.last_connect_configs
            .insert(device_id.to_string(), config);
    }

    /// The payload that last connected `device_id`, if any.
    pub fn last_connect_config(&self, device_id: &str) -> Option<serde_json::Value> {
        self.last_connect_configs
            .get(device_id)
            .map(|v| v.value().clone())
    }

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
            self.record_device_error(
                &device_id,
                "Device physically disconnected (USB unplug detected)",
            )
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

#[cfg(test)]
mod device_identity_tests {
    use super::*;
    use crate::devices::mock::MockDevice;
    use serde_json::json;

    fn mock(name: &str) -> BoxedDevice {
        Box::new(MockDevice::new(name.to_string(), name.to_string()))
    }

    #[tokio::test]
    async fn a_later_connect_wins_even_if_an_earlier_one_finishes_last() {
        let state = AppState::new();
        let open = AtomicBool::new(false);
        // Old connection starts connecting first, new connection second.
        let old_ticket = state.begin_connect("pupil");
        let new_ticket = state.begin_connect("pupil");
        // The newer connect finishes first and registers phone B.
        assert!(state
            .add_device_if_latest("pupil".into(), mock("phone-b"), new_ticket, &open)
            .await
            .is_ok());
        // The older connect finishes last: refused, device handed back.
        let refused = state
            .add_device_if_latest("pupil".into(), mock("phone-a"), old_ticket, &open)
            .await;
        assert!(refused.is_err());
        let registered = state
            .get_device("pupil")
            .await
            .expect("phone-b stays registered");
        assert_eq!(registered.read().await.get_info().name, "phone-b");
    }

    #[tokio::test]
    async fn a_closed_connection_cannot_register_even_with_the_latest_ticket() {
        let state = AppState::new();
        let closed = AtomicBool::new(true);
        let ticket = state.begin_connect("kernel");
        assert!(state
            .add_device_if_latest("kernel".into(), mock("k"), ticket, &closed)
            .await
            .is_err());
        assert!(state.get_device("kernel").await.is_none());
    }

    #[tokio::test]
    async fn disconnect_removes_only_the_exact_device_it_disconnected() {
        let state = AppState::new();
        let open = AtomicBool::new(false);
        let t1 = state.begin_connect("kernel");
        state
            .add_device_if_latest("kernel".into(), mock("first"), t1, &open)
            .await
            .unwrap();
        let first = state.get_device("kernel").await.unwrap();
        // A newer connection replaces it.
        let t2 = state.begin_connect("kernel");
        state
            .add_device_if_latest("kernel".into(), mock("second"), t2, &open)
            .await
            .unwrap();
        // The old connection's slow disconnect must not remove the replacement.
        assert!(!state.remove_device_if_same("kernel", &first).await);
        let second = state
            .get_device("kernel")
            .await
            .expect("second stays registered");
        assert_eq!(second.read().await.get_info().name, "second");
        assert!(state.remove_device_if_same("kernel", &second).await);
        assert!(state.get_device("kernel").await.is_none());
    }

    #[test]
    fn recording_state_is_read_from_pupil_metadata() {
        let info = json!({ "device_id": "a41fe4fe2bccf6c3", "recording_id": "rec-1", "recording_owned": true });
        assert_eq!(
            pupil_recording_state_from_info(&info),
            Some(("a41fe4fe2bccf6c3".into(), Some("rec-1".into()), true))
        );
        let none = json!({ "device_id": "a41fe4fe2bccf6c3", "recording_id": null, "recording_owned": false });
        assert_eq!(
            pupil_recording_state_from_info(&none),
            Some(("a41fe4fe2bccf6c3".into(), None, false))
        );
        assert_eq!(
            pupil_recording_state_from_info(&json!({ "recording_id": "x" })),
            None
        );
    }

    #[test]
    fn connected_event_carries_info_and_disconnected_does_not() {
        let with = DeviceStatusEvent::connected_with_info(
            "pupil".into(),
            DeviceType::Pupil,
            "connected",
            json!({ "device_id": "abc" }),
        );
        let value = serde_json::to_value(&with).unwrap();
        assert_eq!(value["info"]["device_id"], "abc");
        assert_eq!(value["status"], "Connected");

        let without = DeviceStatusEvent::disconnected("kernel".into(), DeviceType::Kernel, "bye");
        let value = serde_json::to_value(&without).unwrap();
        assert!(value.get("info").is_none());
    }
}
