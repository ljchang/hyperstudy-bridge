use super::resolver::DiscoveredStream;
use super::sync::TimeSync;
use super::types::{ChannelFormat, LslError, Sample, SampleData, StreamInfo, StreamStatus};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, info, warn};

/// LSL stream inlet for consuming data
#[derive(Debug)]
pub struct StreamInlet {
    /// Stream information
    info: StreamInfo,
    /// Inlet status
    status: Arc<RwLock<StreamStatus>>,
    /// Time synchronization utility
    time_sync: Arc<TimeSync>,
    /// Data buffer for incoming samples
    buffer: Arc<RwLock<VecDeque<Sample>>>,
    /// Buffer size limit
    buffer_limit: usize,
    /// Active status flag
    active: Arc<AtomicBool>,
    /// Sample counter
    sample_count: Arc<AtomicU64>,
    /// Data receiver for async processing
    #[allow(dead_code)]
    data_receiver: Option<mpsc::UnboundedReceiver<Sample>>,
    /// Performance metrics
    bytes_received: Arc<AtomicU64>,
    last_receive_time: Arc<RwLock<Option<Instant>>>,
    /// Stream UID for identification
    stream_uid: String,
    /// Time correction offset
    time_correction: Arc<RwLock<f64>>,
}

/// Inlet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InletConfig {
    /// Buffer size for samples
    pub buffer_size: usize,
    /// Maximum recovery time for reconnection (seconds)
    pub max_recovery_time: f64,
    /// Post-processing flags
    pub post_processing: PostProcessingFlags,
    /// Automatic time correction
    pub auto_time_correction: bool,
    /// Data processing interval
    pub processing_interval_ms: u64,
}

/// Post-processing options for inlet data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingFlags {
    /// Remove clock synchronization jitter
    pub dejitter_timestamps: bool,
    /// Automatically recover lost samples
    pub recover_samples: bool,
    /// Thread-safe sample retrieval
    pub thread_safe: bool,
    /// Enable monotonic timestamp ordering
    pub monotonic_timestamps: bool,
}

impl Default for InletConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            max_recovery_time: 5.0,
            post_processing: PostProcessingFlags {
                dejitter_timestamps: true,
                recover_samples: false,
                thread_safe: true,
                monotonic_timestamps: true,
            },
            auto_time_correction: true,
            processing_interval_ms: 100,
        }
    }
}

/// Inlet manager for handling multiple inlets
#[derive(Debug)]
pub struct InletManager {
    /// Active inlets
    inlets: Arc<RwLock<std::collections::HashMap<String, Arc<StreamInlet>>>>,
    /// Default configuration
    default_config: InletConfig,
    /// Time synchronization
    time_sync: Arc<TimeSync>,
    /// Data processing task handles
    processing_tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl StreamInlet {
    /// Create a new stream inlet from discovered stream
    pub async fn new(
        discovered_stream: DiscoveredStream,
        config: InletConfig,
        time_sync: Arc<TimeSync>,
    ) -> Result<Self, LslError> {
        info!(
            "Creating LSL inlet: {} (UID: {})",
            discovered_stream.info.name, discovered_stream.uid
        );

        // In a real implementation, this would create an LSL inlet
        // using lsl::StreamInlet::new()

        let inlet = Self {
            info: discovered_stream.info,
            status: Arc::new(RwLock::new(StreamStatus::default())),
            time_sync,
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.buffer_size))),
            buffer_limit: config.buffer_size,
            active: Arc::new(AtomicBool::new(false)),
            sample_count: Arc::new(AtomicU64::new(0)),
            data_receiver: None,
            bytes_received: Arc::new(AtomicU64::new(0)),
            last_receive_time: Arc::new(RwLock::new(None)),
            stream_uid: discovered_stream.uid,
            time_correction: Arc::new(RwLock::new(0.0)),
        };

        debug!("LSL inlet created successfully");
        Ok(inlet)
    }

    /// Open the inlet connection
    pub async fn open(&self, timeout: Duration) -> Result<(), LslError> {
        if self.active.load(Ordering::Relaxed) {
            warn!("Inlet already open");
            return Ok(());
        }

        info!(
            "Opening LSL inlet: {} (timeout: {:?})",
            self.info.name, timeout
        );

        // In a real implementation, this would call:
        // self.lsl_inlet.open_stream(timeout)?;

        // Simulate connection delay
        tokio::time::sleep(Duration::from_millis(100)).await;

        self.active.store(true, Ordering::Relaxed);

        let mut status = self.status.write().await;
        status.active = true;

        // Start time correction if enabled
        if let Ok(correction) = self.calculate_time_correction().await {
            let mut time_correction = self.time_correction.write().await;
            *time_correction = correction;
        }

        info!("LSL inlet opened successfully");
        Ok(())
    }

    /// Close the inlet connection
    pub async fn close(&self) -> Result<(), LslError> {
        if !self.active.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!("Closing LSL inlet: {}", self.info.name);

        self.active.store(false, Ordering::Relaxed);

        let mut status = self.status.write().await;
        status.active = false;

        info!("LSL inlet closed");
        Ok(())
    }

    /// Pull a single sample from the stream
    pub async fn pull_sample(&self, timeout: Duration) -> Result<Option<Sample>, LslError> {
        if !self.active.load(Ordering::Relaxed) {
            return Err(LslError::LslLibraryError("Inlet not open".to_string()));
        }

        // Try to get from buffer first
        {
            let mut buffer = self.buffer.write().await;
            if let Some(sample) = buffer.pop_front() {
                return Ok(Some(sample));
            }
        }

        // Simulate pulling from LSL
        let sample = self.simulate_pull_sample(timeout).await?;

        if let Some(sample) = &sample {
            // Update metrics
            self.sample_count.fetch_add(1, Ordering::Relaxed);
            let bytes = sample.data.to_bytes().len() as u64;
            self.bytes_received.fetch_add(bytes, Ordering::Relaxed);

            // Update status
            {
                let mut status = self.status.write().await;
                status.sample_count += 1;
                status.last_timestamp = sample.timestamp;
            }

            // Update last receive time
            {
                let mut last_receive = self.last_receive_time.write().await;
                *last_receive = Some(Instant::now());
            }
        }

        Ok(sample)
    }

    /// Pull multiple samples as a chunk
    pub async fn pull_chunk(
        &self,
        max_samples: usize,
        timeout: Duration,
    ) -> Result<Vec<Sample>, LslError> {
        let mut samples = Vec::with_capacity(max_samples);
        let start_time = Instant::now();

        while samples.len() < max_samples && start_time.elapsed() < timeout {
            let remaining_timeout = timeout - start_time.elapsed();
            match self.pull_sample(remaining_timeout).await? {
                Some(sample) => samples.push(sample),
                None => break, // No more samples available
            }
        }

        Ok(samples)
    }

    /// Start continuous data collection
    pub async fn start_collection(&self) -> Result<mpsc::Receiver<Sample>, LslError> {
        // Use bounded channel to prevent memory exhaustion from sample backlog
        let (_sender, receiver) = mpsc::channel(10000);

        if !self.active.load(Ordering::Relaxed) {
            return Err(LslError::LslLibraryError("Inlet not open".to_string()));
        }

        // Start collection task
        // Note: This is a simplified implementation
        // In a real implementation, we would use proper async patterns
        info!("Collection task started (placeholder)");

        Ok(receiver)
    }

    /// Get stream information
    pub fn get_info(&self) -> &StreamInfo {
        &self.info
    }

    /// Get stream UID
    pub fn get_uid(&self) -> &str {
        &self.stream_uid
    }

    /// Get current status
    pub async fn get_status(&self) -> StreamStatus {
        self.status.read().await.clone()
    }

    /// Check if inlet is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Get sample count
    pub fn get_sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::Relaxed)
    }

    /// Get bytes received
    pub fn get_bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get buffer usage
    pub async fn get_buffer_usage(&self) -> (usize, usize) {
        let buffer = self.buffer.read().await;
        (buffer.len(), self.buffer_limit)
    }

    /// Flush buffer
    pub async fn flush_buffer(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
    }

    /// Get time correction offset
    pub async fn get_time_correction(&self) -> f64 {
        *self.time_correction.read().await
    }

    /// Get inlet statistics
    pub async fn get_stats(&self) -> serde_json::Value {
        let status = self.status.read().await;
        let (buffer_used, buffer_size) = self.get_buffer_usage().await;
        let last_receive = self.last_receive_time.read().await;
        let time_correction = *self.time_correction.read().await;

        let seconds_since_last_receive = last_receive
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        serde_json::json!({
            "name": self.info.name,
            "uid": self.stream_uid,
            "type": self.info.stream_type,
            "active": self.is_active(),
            "sample_count": self.get_sample_count(),
            "bytes_received": self.get_bytes_received(),
            "buffer_usage": format!("{}/{}", buffer_used, buffer_size),
            "buffer_usage_percent": (buffer_used as f64 / buffer_size as f64) * 100.0,
            "data_loss_percent": status.data_loss,
            "last_timestamp": status.last_timestamp,
            "time_correction": time_correction,
            "seconds_since_last_receive": seconds_since_last_receive
        })
    }

    /// Calculate time correction offset
    async fn calculate_time_correction(&self) -> Result<f64, LslError> {
        // In a real implementation, this would call:
        // self.lsl_inlet.time_correction()?;

        // Simulate time correction calculation
        let correction = self
            .time_sync
            .calculate_time_correction(self.time_sync.lsl_time())
            .await;
        Ok(correction)
    }

    /// Continuous collection loop
    #[allow(dead_code)]
    async fn collection_loop(&self, sender: mpsc::UnboundedSender<Sample>) {
        let mut interval = interval(Duration::from_millis(10)); // 100Hz polling

        while self.active.load(Ordering::Relaxed) {
            interval.tick().await;

            match self.pull_sample(Duration::from_millis(1)).await {
                Ok(Some(sample)) => {
                    if sender.send(sample).is_err() {
                        debug!("Collection receiver dropped, stopping collection");
                        break;
                    }
                }
                Ok(None) => {
                    // No sample available, continue
                }
                Err(e) => {
                    warn!("Error pulling sample: {}", e);
                    // Continue collecting despite errors
                }
            }
        }
    }

    /// Simulate pulling a sample (placeholder implementation)
    async fn simulate_pull_sample(&self, _timeout: Duration) -> Result<Option<Sample>, LslError> {
        // In a real implementation, this would call:
        // let (sample_data, timestamp) = self.lsl_inlet.pull_sample(timeout)?;

        // Simulate occasional data
        if rand::random::<f32>() < 0.1 {
            // 10% chance of having data
            let sample_data = match self.info.channel_format {
                ChannelFormat::Float32 => SampleData::Float32(vec![
                    rand::random::<f32>();
                    self.info.channel_count as usize
                ]),
                ChannelFormat::String => SampleData::String(vec!["MOCK_MARKER".to_string()]),
                ChannelFormat::Int32 => SampleData::Int32(vec![
                    rand::random::<i32>();
                    self.info.channel_count as usize
                ]),
                _ => SampleData::Float32(vec![0.0; self.info.channel_count as usize]),
            };

            let timestamp = self.time_sync.lsl_time();
            let corrected_timestamp = timestamp + *self.time_correction.read().await;

            Ok(Some(Sample {
                data: sample_data,
                timestamp: corrected_timestamp,
            }))
        } else {
            Ok(None)
        }
    }
}

impl InletManager {
    /// Create a new inlet manager
    pub fn new(time_sync: Arc<TimeSync>) -> Self {
        Self {
            inlets: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config: InletConfig::default(),
            time_sync,
            processing_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create inlet manager with custom configuration
    pub fn with_config(time_sync: Arc<TimeSync>, config: InletConfig) -> Self {
        Self {
            inlets: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config: config,
            time_sync,
            processing_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new inlet from discovered stream
    pub async fn create_inlet(
        &self,
        discovered_stream: DiscoveredStream,
        config: Option<InletConfig>,
    ) -> Result<String, LslError> {
        let inlet_config = config.unwrap_or_else(|| self.default_config.clone());
        let inlet = Arc::new(
            StreamInlet::new(
                discovered_stream.clone(),
                inlet_config,
                self.time_sync.clone(),
            )
            .await?,
        );

        let inlet_id = discovered_stream.uid.clone();

        let mut inlets = self.inlets.write().await;
        inlets.insert(inlet_id.clone(), inlet);

        info!("Created inlet: {}", inlet_id);
        Ok(inlet_id)
    }

    /// Get inlet by ID
    pub async fn get_inlet(&self, inlet_id: &str) -> Option<Arc<StreamInlet>> {
        let inlets = self.inlets.read().await;
        inlets.get(inlet_id).cloned()
    }

    /// Open inlet connection
    pub async fn open_inlet(&self, inlet_id: &str, timeout: Duration) -> Result<(), LslError> {
        let inlet = self
            .get_inlet(inlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(inlet_id.to_string()))?;

        inlet.open(timeout).await
    }

    /// Close inlet connection
    pub async fn close_inlet(&self, inlet_id: &str) -> Result<(), LslError> {
        let inlet = self
            .get_inlet(inlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(inlet_id.to_string()))?;

        inlet.close().await
    }

    /// Remove inlet
    pub async fn remove_inlet(&self, inlet_id: &str) -> Result<(), LslError> {
        // Close inlet first
        if let Some(inlet) = self.get_inlet(inlet_id).await {
            inlet.close().await?;
        }

        let mut inlets = self.inlets.write().await;
        inlets.remove(inlet_id);

        info!("Removed inlet: {}", inlet_id);
        Ok(())
    }

    /// Pull sample from inlet
    pub async fn pull_sample(
        &self,
        inlet_id: &str,
        timeout: Duration,
    ) -> Result<Option<Sample>, LslError> {
        let inlet = self
            .get_inlet(inlet_id)
            .await
            .ok_or_else(|| LslError::StreamNotFound(inlet_id.to_string()))?;

        inlet.pull_sample(timeout).await
    }

    /// List all inlets
    pub async fn list_inlets(&self) -> Vec<String> {
        let inlets = self.inlets.read().await;
        inlets.keys().cloned().collect()
    }

    /// Get inlet statistics for all inlets
    pub async fn get_all_stats(&self) -> serde_json::Value {
        let inlets = self.inlets.read().await;
        let mut stats = std::collections::HashMap::new();

        for (id, inlet) in inlets.iter() {
            stats.insert(id.clone(), inlet.get_stats().await);
        }

        serde_json::json!({
            "inlet_count": inlets.len(),
            "inlets": stats
        })
    }

    /// Start all inlets
    pub async fn start_all(&self, timeout: Duration) -> Result<(), LslError> {
        let inlets = self.inlets.read().await;
        for inlet in inlets.values() {
            inlet.open(timeout).await?;
        }
        Ok(())
    }

    /// Stop all inlets
    pub async fn stop_all(&self) -> Result<(), LslError> {
        let inlets = self.inlets.read().await;
        for inlet in inlets.values() {
            inlet.close().await?;
        }
        Ok(())
    }

    /// Clean up processing tasks
    pub async fn cleanup_tasks(&self) {
        let mut tasks = self.processing_tasks.write().await;
        tasks.retain(|handle| !handle.is_finished());
    }
}

// Add rand dependency for mock implementation
use rand;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::lsl::{resolver::DiscoveredStream, sync::TimeSync, types::StreamInfo};
    use std::time::SystemTime;

    fn create_mock_discovered_stream() -> DiscoveredStream {
        DiscoveredStream {
            info: StreamInfo::ttl_markers("test_device"),
            discovered_at: SystemTime::now(),
            last_seen: SystemTime::now(),
            available: true,
            uid: "test_stream_001".to_string(),
            session_id: "session_001".to_string(),
            data_loss: 0.0,
            time_stamps: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_inlet_creation() {
        let time_sync = Arc::new(TimeSync::new(false));
        let discovered_stream = create_mock_discovered_stream();
        let config = InletConfig::default();

        let inlet = StreamInlet::new(discovered_stream, config, time_sync).await;
        assert!(inlet.is_ok());

        let inlet = inlet.unwrap();
        assert_eq!(inlet.get_info().name, "test_device_TTL_Markers");
        assert!(!inlet.is_active());
    }

    #[tokio::test]
    async fn test_inlet_lifecycle() {
        let time_sync = Arc::new(TimeSync::new(false));
        let discovered_stream = create_mock_discovered_stream();
        let config = InletConfig::default();

        let inlet = StreamInlet::new(discovered_stream, config, time_sync)
            .await
            .unwrap();

        // Open inlet
        assert!(inlet.open(Duration::from_secs(1)).await.is_ok());
        assert!(inlet.is_active());

        // Close inlet
        assert!(inlet.close().await.is_ok());
        assert!(!inlet.is_active());
    }

    #[tokio::test]
    async fn test_inlet_manager() {
        let time_sync = Arc::new(TimeSync::new(false));
        let manager = InletManager::new(time_sync);

        let discovered_stream = create_mock_discovered_stream();
        let inlet_id = manager.create_inlet(discovered_stream, None).await;
        assert!(inlet_id.is_ok());

        let inlet_id = inlet_id.unwrap();
        assert!(manager
            .open_inlet(&inlet_id, Duration::from_secs(1))
            .await
            .is_ok());

        let inlets = manager.list_inlets().await;
        assert_eq!(inlets.len(), 1);
        assert!(inlets.contains(&inlet_id));
    }
}
