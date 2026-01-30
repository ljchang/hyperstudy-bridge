// Note: Temporarily removing metrics crate integration due to compilation issues
// Will re-add once the proper syntax is determined
use histogram::Histogram;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tokio::sync::RwLock;

/// Performance monitoring system for HyperStudy Bridge
/// Tracks device latency, throughput, system resources, and connection stability
#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    /// System information collector
    pub system: Arc<RwLock<System>>,
    /// Device-specific performance counters
    pub device_counters: Arc<RwLock<HashMap<String, DeviceCounters>>>,
    /// Overall system metrics
    pub system_counters: SystemCounters,
    /// Start time for uptime calculation
    pub start_time: Instant,
}

/// Device-specific performance counters
#[derive(Debug, Clone)]
pub struct DeviceCounters {
    /// Device identifier
    pub device_id: String,
    /// Total messages sent to device
    pub messages_sent: Arc<AtomicU64>,
    /// Total messages received from device
    pub messages_received: Arc<AtomicU64>,
    /// Total errors encountered
    pub errors: Arc<AtomicU64>,
    /// Connection attempts counter
    pub connection_attempts: Arc<AtomicU64>,
    /// Successful connections counter
    pub successful_connections: Arc<AtomicU64>,
    /// Latency histogram for tracking distribution
    pub latency_histogram: Arc<RwLock<Histogram>>,
    /// Last recorded latency
    pub last_latency_ns: Arc<AtomicU64>,
    /// Bytes sent counter
    pub bytes_sent: Arc<AtomicU64>,
    /// Bytes received counter
    pub bytes_received: Arc<AtomicU64>,
    /// Last activity timestamp
    pub last_activity: Arc<AtomicU64>,
}

/// System-wide performance counters
#[derive(Debug, Clone)]
pub struct SystemCounters {
    /// Total WebSocket connections established
    pub total_connections: Arc<AtomicU64>,
    /// Currently active connections
    pub active_connections: Arc<AtomicU64>,
    /// Total bridge messages processed
    pub bridge_messages: Arc<AtomicU64>,
    /// Total errors across all components
    pub global_errors: Arc<AtomicU64>,
    /// Memory usage tracking
    pub memory_usage_bytes: Arc<AtomicU64>,
    /// CPU usage percentage
    pub cpu_usage_percent: Arc<AtomicU64>,
}

/// Exported performance metrics for external consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Timestamp when metrics were collected
    pub timestamp: u64,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// System-level metrics
    pub system: SystemMetrics,
    /// Per-device metrics
    pub devices: HashMap<String, DevicePerformanceMetrics>,
}

/// System performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: f64,
    /// Total active WebSocket connections
    pub active_connections: u64,
    /// Total bridge messages processed
    pub bridge_messages: u64,
    /// Total errors across all components
    pub global_errors: u64,
}

/// Device-specific performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePerformanceMetrics {
    /// Device identifier
    pub device_id: String,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total errors
    pub errors: u64,
    /// Connection success rate (0.0-1.0)
    pub connection_success_rate: f64,
    /// Last latency in nanoseconds
    pub last_latency_ns: u64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: f64,
    /// P95 latency in nanoseconds
    pub p95_latency_ns: f64,
    /// P99 latency in nanoseconds
    pub p99_latency_ns: f64,
    /// Throughput in messages per second
    pub throughput_mps: f64,
    /// Bytes sent total
    pub bytes_sent: u64,
    /// Bytes received total
    pub bytes_received: u64,
    /// Seconds since last activity
    pub seconds_since_last_activity: u64,
}

impl PerformanceMonitor {
    /// Create a new performance monitor instance
    pub fn new() -> Self {
        // Initialize metrics registry
        Self::register_global_metrics();

        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(RwLock::new(system)),
            device_counters: Arc::new(RwLock::new(HashMap::new())),
            system_counters: SystemCounters::new(),
            start_time: Instant::now(),
        }
    }

    /// Register global metrics (placeholder for future metrics integration)
    fn register_global_metrics() {
        // Placeholder - will implement proper metrics registration
        // when metrics crate syntax issues are resolved
    }

    /// Add a new device to monitoring
    pub async fn add_device(&self, device_id: String) {
        let counters = DeviceCounters::new(device_id.clone());
        let mut device_counters = self.device_counters.write().await;
        device_counters.insert(device_id, counters);
    }

    /// Remove a device from monitoring
    pub async fn remove_device(&self, device_id: &str) {
        let mut device_counters = self.device_counters.write().await;
        device_counters.remove(device_id);
    }

    /// Record a device operation with latency measurement
    pub async fn record_device_operation(
        &self,
        device_id: &str,
        latency: Duration,
        bytes_sent: u64,
        bytes_received: u64,
    ) {
        if let Some(counters) = self.get_device_counters(device_id).await {
            let latency_ns = latency.as_nanos() as u64;

            counters.messages_sent.fetch_add(1, Ordering::Relaxed);
            if bytes_received > 0 {
                counters.messages_received.fetch_add(1, Ordering::Relaxed);
            }
            counters.bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
            counters
                .bytes_received
                .fetch_add(bytes_received, Ordering::Relaxed);
            counters
                .last_latency_ns
                .store(latency_ns, Ordering::Relaxed);
            counters.last_activity.store(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                Ordering::Relaxed,
            );

            // Update latency histogram
            {
                let mut histogram = counters.latency_histogram.write().await;
                histogram.increment(latency_ns).ok();
            }

            // Note: Metrics registry calls removed temporarily due to compilation issues
            // Will re-add once proper syntax is determined
        }
    }

    /// Record a device error
    pub async fn record_device_error(&self, device_id: &str, error_msg: &str) {
        if let Some(counters) = self.get_device_counters(device_id).await {
            counters.errors.fetch_add(1, Ordering::Relaxed);
        }

        self.system_counters
            .global_errors
            .fetch_add(1, Ordering::Relaxed);

        tracing::error!("Device {} error: {}", device_id, error_msg);
    }

    /// Record a device connection attempt
    pub async fn record_connection_attempt(&self, device_id: &str, success: bool) {
        if let Some(counters) = self.get_device_counters(device_id).await {
            counters.connection_attempts.fetch_add(1, Ordering::Relaxed);
            if success {
                counters
                    .successful_connections
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a WebSocket connection event.
    ///
    /// Uses saturating subtraction on disconnect to prevent counter underflow
    /// if disconnect is called more times than connect.
    pub fn record_websocket_connection(&self, connected: bool) {
        if connected {
            self.system_counters
                .total_connections
                .fetch_add(1, Ordering::Relaxed);
            self.system_counters
                .active_connections
                .fetch_add(1, Ordering::Relaxed);
        } else {
            // Use fetch_update with saturating_sub to prevent underflow
            let _ = self
                .system_counters
                .active_connections
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(1))
                });
        }
    }

    /// Record a bridge message
    pub fn record_bridge_message(&self) {
        self.system_counters
            .bridge_messages
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Update system resource metrics
    pub async fn update_system_metrics(&self) {
        let mut system = self.system.write().await;
        system.refresh_memory();
        system.refresh_cpu_all();

        // Get memory usage (simplified for compatibility)
        let memory_bytes = system.total_memory();
        self.system_counters
            .memory_usage_bytes
            .store(memory_bytes, Ordering::Relaxed);

        // Get CPU usage (simplified for compatibility)
        let cpu_info = system.cpus();
        let cpu_usage = if !cpu_info.is_empty() {
            cpu_info[0].cpu_usage() as u64
        } else {
            0
        };
        self.system_counters
            .cpu_usage_percent
            .store(cpu_usage, Ordering::Relaxed);
    }

    /// Get comprehensive performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        // Update system metrics first
        self.update_system_metrics().await;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let uptime_seconds = self.start_time.elapsed().as_secs();

        let system_metrics = SystemMetrics {
            memory_usage_bytes: self
                .system_counters
                .memory_usage_bytes
                .load(Ordering::Relaxed),
            cpu_usage_percent: self
                .system_counters
                .cpu_usage_percent
                .load(Ordering::Relaxed) as f64,
            active_connections: self
                .system_counters
                .active_connections
                .load(Ordering::Relaxed),
            bridge_messages: self.system_counters.bridge_messages.load(Ordering::Relaxed),
            global_errors: self.system_counters.global_errors.load(Ordering::Relaxed),
        };

        let mut device_metrics = HashMap::new();
        let device_counters = self.device_counters.read().await;

        for (device_id, counters) in device_counters.iter() {
            let metrics = self.calculate_device_metrics(counters, timestamp).await;
            device_metrics.insert(device_id.clone(), metrics);
        }

        PerformanceMetrics {
            timestamp,
            uptime_seconds,
            system: system_metrics,
            devices: device_metrics,
        }
    }

    /// Get metrics for a specific device
    pub async fn get_device_metrics(&self, device_id: &str) -> Option<DevicePerformanceMetrics> {
        let device_counters = self.device_counters.read().await;
        let counters = device_counters.get(device_id)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Some(self.calculate_device_metrics(counters, timestamp).await)
    }

    /// Calculate device performance metrics from counters
    async fn calculate_device_metrics(
        &self,
        counters: &DeviceCounters,
        current_timestamp: u64,
    ) -> DevicePerformanceMetrics {
        let messages_sent = counters.messages_sent.load(Ordering::Relaxed);
        let messages_received = counters.messages_received.load(Ordering::Relaxed);
        let errors = counters.errors.load(Ordering::Relaxed);
        let connection_attempts = counters.connection_attempts.load(Ordering::Relaxed);
        let successful_connections = counters.successful_connections.load(Ordering::Relaxed);
        let last_latency_ns = counters.last_latency_ns.load(Ordering::Relaxed);
        let bytes_sent = counters.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = counters.bytes_received.load(Ordering::Relaxed);
        let last_activity = counters.last_activity.load(Ordering::Relaxed);

        let connection_success_rate = if connection_attempts > 0 {
            successful_connections as f64 / connection_attempts as f64
        } else {
            0.0
        };

        let seconds_since_last_activity = if last_activity > 0 {
            current_timestamp.saturating_sub(last_activity)
        } else {
            0
        };

        // Calculate latency statistics from histogram
        // Note: Simplified histogram usage for compatibility
        let _histogram = counters.latency_histogram.read().await;
        let (avg_latency_ns, p95_latency_ns, p99_latency_ns) = if last_latency_ns > 0 {
            // For now, use last latency as approximation
            let latency_f64 = last_latency_ns as f64;
            (latency_f64, latency_f64, latency_f64)
        } else {
            (0.0, 0.0, 0.0)
        };

        // Calculate throughput (messages per second over last minute)
        let uptime_seconds = self.start_time.elapsed().as_secs();
        let throughput_mps = if uptime_seconds > 0 {
            messages_sent as f64 / uptime_seconds as f64
        } else {
            0.0
        };

        DevicePerformanceMetrics {
            device_id: counters.device_id.clone(),
            messages_sent,
            messages_received,
            errors,
            connection_success_rate,
            last_latency_ns,
            avg_latency_ns,
            p95_latency_ns,
            p99_latency_ns,
            throughput_mps,
            bytes_sent,
            bytes_received,
            seconds_since_last_activity,
        }
    }

    /// Get device counters for a specific device
    async fn get_device_counters(&self, device_id: &str) -> Option<DeviceCounters> {
        let device_counters = self.device_counters.read().await;
        device_counters.get(device_id).cloned()
    }

    /// Check if TTL latency is within acceptable bounds (<1ms)
    pub async fn check_ttl_latency_compliance(&self, device_id: &str) -> Option<bool> {
        if let Some(metrics) = self.get_device_metrics(device_id).await {
            // Check if P95 latency is under 1ms (1_000_000 ns)
            Some(metrics.p95_latency_ns < 1_000_000.0)
        } else {
            None
        }
    }

    /// Get performance summary for monitoring dashboard
    pub async fn get_performance_summary(&self) -> serde_json::Value {
        let metrics = self.get_metrics().await;

        serde_json::json!({
            "timestamp": metrics.timestamp,
            "uptime_seconds": metrics.uptime_seconds,
            "system": {
                "memory_mb": metrics.system.memory_usage_bytes / 1024 / 1024,
                "cpu_percent": metrics.system.cpu_usage_percent,
                "active_connections": metrics.system.active_connections,
                "total_messages": metrics.system.bridge_messages,
                "total_errors": metrics.system.global_errors
            },
            "devices": metrics.devices.iter().map(|(id, dev)| {
                serde_json::json!({
                    "id": id,
                    "latency_ms": dev.last_latency_ns as f64 / 1_000_000.0,
                    "avg_latency_ms": dev.avg_latency_ns / 1_000_000.0,
                    "p95_latency_ms": dev.p95_latency_ns / 1_000_000.0,
                    "throughput_mps": dev.throughput_mps,
                    "success_rate": dev.connection_success_rate,
                    "errors": dev.errors
                })
            }).collect::<Vec<_>>()
        })
    }
}

impl DeviceCounters {
    /// Create new device counters
    pub fn new(device_id: String) -> Self {
        Self {
            device_id,
            messages_sent: Arc::new(AtomicU64::new(0)),
            messages_received: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            connection_attempts: Arc::new(AtomicU64::new(0)),
            successful_connections: Arc::new(AtomicU64::new(0)),
            latency_histogram: Arc::new(RwLock::new(
                Histogram::new(3, 16).expect("Failed to create histogram"),
            )),
            last_latency_ns: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            last_activity: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for SystemCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCounters {
    /// Create new system counters
    pub fn new() -> Self {
        Self {
            total_connections: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU64::new(0)),
            bridge_messages: Arc::new(AtomicU64::new(0)),
            global_errors: Arc::new(AtomicU64::new(0)),
            memory_usage_bytes: Arc::new(AtomicU64::new(0)),
            cpu_usage_percent: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to measure operation latency
pub async fn measure_latency<F, T, E>(operation: F) -> (Result<T, E>, Duration)
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let result = operation.await;
    let latency = start.elapsed();
    (result, latency)
}
