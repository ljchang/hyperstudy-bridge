use super::sync::TimeSync;
use super::types::{LslError, Sample, StreamInfo, StreamStatus};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// LSL stream outlet for publishing data
#[derive(Debug)]
pub struct StreamOutlet {
    /// Stream information
    info: StreamInfo,
    /// Outlet status
    status: Arc<RwLock<StreamStatus>>,
    /// Time synchronization utility
    time_sync: Arc<TimeSync>,
    /// Data buffer for outgoing samples
    buffer: Arc<RwLock<VecDeque<Sample>>>,
    /// Buffer size limit
    buffer_limit: usize,
    /// Active status flag
    active: Arc<AtomicBool>,
    /// Sample counter
    sample_count: Arc<AtomicU64>,
    /// Data sender for async processing
    #[allow(dead_code)]
    data_sender: Option<mpsc::UnboundedSender<Sample>>,
    /// Performance metrics
    bytes_sent: Arc<AtomicU64>,
    last_send_time: Arc<RwLock<Option<Instant>>>,
}

/// Outlet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutletConfig {
    /// Buffer size for samples
    pub buffer_size: usize,
    /// Maximum transmission rate (samples per second)
    pub max_rate: f64,
    /// Enable automatic time stamping
    pub auto_timestamp: bool,
    /// Compression level (0-9, 0=none)
    pub compression: u32,
    /// Enable data integrity checks
    pub enable_crc: bool,
}

impl Default for OutletConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            max_rate: 0.0, // Unlimited
            auto_timestamp: true,
            compression: 0,
            enable_crc: false,
        }
    }
}

/// Outlet manager for handling multiple outlets
#[derive(Debug)]
pub struct OutletManager {
    /// Active outlets
    outlets: Arc<RwLock<std::collections::HashMap<String, Arc<StreamOutlet>>>>,
    /// Default configuration
    default_config: OutletConfig,
    /// Time synchronization
    time_sync: Arc<TimeSync>,
}

impl StreamOutlet {
    /// Create a new stream outlet
    pub async fn new(
        info: StreamInfo,
        config: OutletConfig,
        time_sync: Arc<TimeSync>,
    ) -> Result<Self, LslError> {
        info!(
            "Creating LSL outlet: {} (type: {}, channels: {})",
            info.name, info.stream_type, info.channel_count
        );

        // In a real implementation, this would create an LSL outlet
        // using lsl::StreamOutlet::new()

        let outlet = Self {
            info,
            status: Arc::new(RwLock::new(StreamStatus::default())),
            time_sync,
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.buffer_size))),
            buffer_limit: config.buffer_size,
            active: Arc::new(AtomicBool::new(false)),
            sample_count: Arc::new(AtomicU64::new(0)),
            data_sender: None,
            bytes_sent: Arc::new(AtomicU64::new(0)),
            last_send_time: Arc::new(RwLock::new(None)),
        };

        debug!("LSL outlet created successfully");
        Ok(outlet)
    }

    /// Start the outlet and begin publishing
    pub async fn start(&self) -> Result<(), LslError> {
        if self.active.load(Ordering::Relaxed) {
            warn!("Outlet already active");
            return Ok(());
        }

        info!("Starting LSL outlet: {}", self.info.name);

        self.active.store(true, Ordering::Relaxed);

        let mut status = self.status.write().await;
        status.active = true;

        info!("LSL outlet started successfully");
        Ok(())
    }

    /// Stop the outlet
    pub async fn stop(&self) -> Result<(), LslError> {
        if !self.active.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!("Stopping LSL outlet: {}", self.info.name);

        self.active.store(false, Ordering::Relaxed);

        let mut status = self.status.write().await;
        status.active = false;

        info!("LSL outlet stopped");
        Ok(())
    }

    /// Send a sample through the outlet
    pub async fn send_sample(&self, mut sample: Sample) -> Result<(), LslError> {
        if !self.active.load(Ordering::Relaxed) {
            return Err(LslError::LslLibraryError("Outlet not active".to_string()));
        }

        // Auto-timestamp if enabled
        if sample.timestamp == 0.0 {
            sample.timestamp = self.time_sync.create_timestamp();
        }

        // Validate channel count
        if sample.data.channel_count() != self.info.channel_count as usize {
            return Err(LslError::DataFormatMismatch {
                expected: format!("{} channels", self.info.channel_count),
                actual: format!("{} channels", sample.data.channel_count()),
            });
        }

        // Add to buffer
        {
            let mut buffer = self.buffer.write().await;

            // Check buffer overflow
            if buffer.len() >= self.buffer_limit {
                buffer.pop_front(); // Remove oldest sample
                warn!("Outlet buffer overflow, dropping oldest sample");

                let mut status = self.status.write().await;
                status.data_loss = (status.data_loss + 0.1).min(100.0);
            }

            buffer.push_back(sample.clone());
        }

        // Update metrics
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        let bytes = sample.data.to_bytes().len() as u64;
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);

        // Update status
        {
            let mut status = self.status.write().await;
            status.sample_count += 1;
            status.last_timestamp = sample.timestamp;
        }

        // Update last send time
        {
            let mut last_send = self.last_send_time.write().await;
            *last_send = Some(Instant::now());
        }

        let timestamp = sample.timestamp;
        let channel_count = sample.data.channel_count();

        // In a real implementation, this would call outlet.push_sample()
        self.simulate_send(sample).await?;

        debug!(
            "Sample sent: timestamp={:.3}, channels={}",
            timestamp, channel_count
        );

        Ok(())
    }

    /// Send multiple samples as a chunk
    pub async fn send_chunk(&self, samples: Vec<Sample>) -> Result<(), LslError> {
        for sample in samples {
            self.send_sample(sample).await?;
        }
        Ok(())
    }

    /// Get stream information
    pub fn get_info(&self) -> &StreamInfo {
        &self.info
    }

    /// Get current status
    pub async fn get_status(&self) -> StreamStatus {
        self.status.read().await.clone()
    }

    /// Check if outlet is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Get sample count
    pub fn get_sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::Relaxed)
    }

    /// Get bytes sent
    pub fn get_bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Get buffer usage
    pub async fn get_buffer_usage(&self) -> (usize, usize) {
        let buffer = self.buffer.read().await;
        (buffer.len(), self.buffer_limit)
    }

    /// Clear buffer
    pub async fn clear_buffer(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
    }

    /// Get outlet statistics
    pub async fn get_stats(&self) -> serde_json::Value {
        let status = self.status.read().await;
        let (buffer_used, buffer_size) = self.get_buffer_usage().await;
        let last_send = self.last_send_time.read().await;

        let seconds_since_last_send = last_send.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);

        serde_json::json!({
            "name": self.info.name,
            "type": self.info.stream_type,
            "active": self.is_active(),
            "sample_count": self.get_sample_count(),
            "bytes_sent": self.get_bytes_sent(),
            "buffer_usage": format!("{}/{}", buffer_used, buffer_size),
            "buffer_usage_percent": (buffer_used as f64 / buffer_size as f64) * 100.0,
            "data_loss_percent": status.data_loss,
            "last_timestamp": status.last_timestamp,
            "seconds_since_last_send": seconds_since_last_send
        })
    }

    /// Simulate sending data (placeholder implementation)
    async fn simulate_send(&self, sample: Sample) -> Result<(), LslError> {
        // In a real implementation, this would call:
        // self.lsl_outlet.push_sample(&sample_data, sample.timestamp)?;

        // Simulate network delay
        tokio::time::sleep(Duration::from_micros(10)).await;

        // Simulate occasional errors
        if sample.timestamp < 0.0 {
            return Err(LslError::InvalidSampleData(
                "Negative timestamp".to_string(),
            ));
        }

        Ok(())
    }
}

impl OutletManager {
    /// Create a new outlet manager
    pub fn new(time_sync: Arc<TimeSync>) -> Self {
        Self {
            outlets: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config: OutletConfig::default(),
            time_sync,
        }
    }

    /// Create outlet with custom configuration
    pub fn with_config(time_sync: Arc<TimeSync>, config: OutletConfig) -> Self {
        Self {
            outlets: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config: config,
            time_sync,
        }
    }

    /// Create a new outlet
    pub async fn create_outlet(
        &self,
        info: StreamInfo,
        config: Option<OutletConfig>,
    ) -> Result<String, LslError> {
        let outlet_config = config.unwrap_or_else(|| self.default_config.clone());
        let outlet =
            Arc::new(StreamOutlet::new(info.clone(), outlet_config, self.time_sync.clone()).await?);

        let outlet_id = format!("{}_{}", info.stream_type, info.source_id);

        let mut outlets = self.outlets.write().await;
        outlets.insert(outlet_id.clone(), outlet);

        info!("Created outlet: {}", outlet_id);
        Ok(outlet_id)
    }

    /// Get outlet by ID
    pub async fn get_outlet(&self, outlet_id: &str) -> Option<Arc<StreamOutlet>> {
        let outlets = self.outlets.read().await;
        outlets.get(outlet_id).cloned()
    }

    /// Start outlet
    pub async fn start_outlet(&self, outlet_id: &str) -> Result<(), LslError> {
        let outlet = self
            .get_outlet(outlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(outlet_id.to_string()))?;

        outlet.start().await
    }

    /// Stop outlet
    pub async fn stop_outlet(&self, outlet_id: &str) -> Result<(), LslError> {
        let outlet = self
            .get_outlet(outlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(outlet_id.to_string()))?;

        outlet.stop().await
    }

    /// Remove outlet
    pub async fn remove_outlet(&self, outlet_id: &str) -> Result<(), LslError> {
        // Stop outlet first
        if let Some(outlet) = self.get_outlet(outlet_id).await {
            outlet.stop().await?;
        }

        let mut outlets = self.outlets.write().await;
        outlets.remove(outlet_id);

        info!("Removed outlet: {}", outlet_id);
        Ok(())
    }

    /// Send sample to outlet
    pub async fn send_sample(&self, outlet_id: &str, sample: Sample) -> Result<(), LslError> {
        let outlet = self
            .get_outlet(outlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(outlet_id.to_string()))?;

        outlet.send_sample(sample).await
    }

    /// List all outlets
    pub async fn list_outlets(&self) -> Vec<String> {
        let outlets = self.outlets.read().await;
        outlets.keys().cloned().collect()
    }

    /// Get outlet statistics for all outlets
    pub async fn get_all_stats(&self) -> serde_json::Value {
        let outlets = self.outlets.read().await;
        let mut stats = std::collections::HashMap::new();

        for (id, outlet) in outlets.iter() {
            stats.insert(id.clone(), outlet.get_stats().await);
        }

        serde_json::json!({
            "outlet_count": outlets.len(),
            "outlets": stats
        })
    }

    /// Create outlet for specific device types
    pub async fn create_device_outlet(
        &self,
        device_id: &str,
        device_type: &str,
        config: Option<OutletConfig>,
    ) -> Result<String, LslError> {
        let stream_info = match device_type {
            "ttl" => StreamInfo::ttl_markers(device_id),
            "kernel" => StreamInfo::kernel_fnirs(device_id, 16), // Default 16 channels
            "pupil" => StreamInfo::pupil_gaze(device_id),
            _ => {
                return Err(LslError::LslLibraryError(format!(
                    "Unknown device type: {}",
                    device_type
                )))
            }
        };

        self.create_outlet(stream_info, config).await
    }

    /// Start all outlets
    pub async fn start_all(&self) -> Result<(), LslError> {
        let outlets = self.outlets.read().await;
        for outlet in outlets.values() {
            outlet.start().await?;
        }
        Ok(())
    }

    /// Stop all outlets
    pub async fn stop_all(&self) -> Result<(), LslError> {
        let outlets = self.outlets.read().await;
        for outlet in outlets.values() {
            outlet.stop().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::lsl::sync::TimeSync;
    use crate::devices::lsl::types::SampleData;

    #[tokio::test]
    async fn test_outlet_creation() {
        let time_sync = Arc::new(TimeSync::new(false));
        let info = StreamInfo::ttl_markers("test_device");
        let config = OutletConfig::default();

        let outlet = StreamOutlet::new(info, config, time_sync).await;
        assert!(outlet.is_ok());

        let outlet = outlet.unwrap();
        assert_eq!(outlet.get_info().name, "test_device_TTL_Markers");
        assert!(!outlet.is_active());
    }

    #[tokio::test]
    async fn test_outlet_lifecycle() {
        let time_sync = Arc::new(TimeSync::new(false));
        let info = StreamInfo::ttl_markers("test_device");
        let config = OutletConfig::default();

        let outlet = StreamOutlet::new(info, config, time_sync).await.unwrap();

        // Start outlet
        assert!(outlet.start().await.is_ok());
        assert!(outlet.is_active());

        // Stop outlet
        assert!(outlet.stop().await.is_ok());
        assert!(!outlet.is_active());
    }

    #[tokio::test]
    async fn test_sample_sending() {
        let time_sync = Arc::new(TimeSync::new(false));
        let info = StreamInfo::ttl_markers("test_device");
        let config = OutletConfig::default();

        let outlet = StreamOutlet::new(info, config, time_sync).await.unwrap();
        outlet.start().await.unwrap();

        let sample = Sample {
            data: SampleData::ttl_marker("TEST_PULSE".to_string()),
            timestamp: 1000.0,
        };

        assert!(outlet.send_sample(sample).await.is_ok());
        assert_eq!(outlet.get_sample_count(), 1);
    }

    #[tokio::test]
    async fn test_outlet_manager() {
        let time_sync = Arc::new(TimeSync::new(false));
        let manager = OutletManager::new(time_sync);

        let outlet_id = manager
            .create_device_outlet("test_device", "ttl", None)
            .await;
        assert!(outlet_id.is_ok());

        let outlet_id = outlet_id.unwrap();
        assert!(manager.start_outlet(&outlet_id).await.is_ok());

        let outlets = manager.list_outlets().await;
        assert_eq!(outlets.len(), 1);
        assert!(outlets.contains(&outlet_id));
    }
}
