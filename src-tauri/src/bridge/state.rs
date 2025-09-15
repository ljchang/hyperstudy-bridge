use crate::devices::{BoxedDevice, DeviceInfo, DeviceStatus};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Arc<RwLock<BoxedDevice>>>>>,
    pub connections: Arc<DashMap<String, ConnectionInfo>>,
    pub metrics: Arc<RwLock<Metrics>>,
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

impl AppState {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            start_time: Instant::now(),
            message_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn add_device(&self, id: String, device: BoxedDevice) {
        let mut devices = self.devices.write().await;
        devices.insert(id, Arc::new(RwLock::new(device)));
    }

    pub async fn remove_device(&self, id: &str) -> Option<Arc<RwLock<BoxedDevice>>> {
        let mut devices = self.devices.write().await;
        devices.remove(id)
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
            .unwrap()
            .as_secs();

        let info = ConnectionInfo {
            id: id.clone(),
            client_id,
            connected_at: now,
            last_activity: now,
            message_count: 0,
        };

        self.connections.insert(id, info);
    }

    pub fn remove_connection(&self, id: &str) {
        self.connections.remove(id);
    }

    pub fn update_connection_activity(&self, id: &str) {
        if let Some(mut entry) = self.connections.get_mut(id) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
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
            .unwrap()
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
        metrics.device_metrics.iter()
            .find(|m| m.device_id == device_id)
            .cloned()
    }
}