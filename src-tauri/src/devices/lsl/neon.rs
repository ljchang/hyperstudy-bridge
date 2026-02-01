//! Pupil Labs Neon LSL Integration
//!
//! This module provides automatic discovery and data consumption from Pupil Labs Neon
//! eye trackers streaming via Lab Streaming Layer (LSL).
//!
//! # Neon LSL Streams
//!
//! When "Stream over LSL" is enabled in the Neon Companion App, two streams are created:
//! - `{DeviceName}_Neon Gaze` - Float32 gaze data at 200Hz
//! - `{DeviceName}_Neon Events` - String event markers at irregular intervals
//!
//! # Usage
//!
//! ```ignore
//! let manager = NeonLslManager::new(time_sync, resolver, inlet_manager);
//!
//! // Discover Neon devices
//! let devices = manager.discover_neon_devices().await?;
//!
//! // Connect to gaze stream
//! let gaze_rx = manager.connect_gaze_stream("MyNeon").await?;
//!
//! // Receive gaze data
//! while let Some(gaze) = gaze_rx.recv().await {
//!     println!("Gaze: ({}, {})", gaze.gaze_x, gaze.gaze_y);
//! }
//! ```

use super::inlet::{InletConfig, InletManager};
use super::resolver::{StreamFilter, StreamResolver};
use super::types::{DiscoveredNeonDevice, LslError, NeonEventData, NeonGazeData, SampleData};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Default buffer size for gaze data channel (200Hz * 5 seconds = 1000 samples)
const GAZE_CHANNEL_BUFFER_SIZE: usize = 1000;

/// Default buffer size for event data channel
const EVENT_CHANNEL_BUFFER_SIZE: usize = 100;

/// Neon LSL Manager
///
/// Manages discovery and data streaming from Pupil Labs Neon devices via LSL.
/// Uses the existing LSL infrastructure (resolver, inlet manager, time sync)
/// with Neon-specific discovery patterns and data parsing.
pub struct NeonLslManager {
    /// Stream resolver for discovery (shared to maintain stream cache)
    resolver: Arc<StreamResolver>,
    /// Inlet manager for consuming streams
    inlet_manager: Arc<InletManager>,
    /// Discovered Neon devices (device_name -> DiscoveredNeonDevice)
    discovered_devices: Arc<RwLock<HashMap<String, DiscoveredNeonDevice>>>,
    /// Active gaze stream tasks (device_name -> task handle)
    gaze_tasks: Arc<RwLock<HashMap<String, GazeStreamTask>>>,
    /// Active event stream tasks (device_name -> task handle)
    event_tasks: Arc<RwLock<HashMap<String, EventStreamTask>>>,
}

/// Handle for an active gaze stream task
struct GazeStreamTask {
    /// Sender kept to check channel health via `is_closed()` if needed.
    /// The task uses a clone of this sender.
    #[allow(dead_code)]
    sender: mpsc::Sender<NeonGazeData>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Task join handle
    _handle: tokio::task::JoinHandle<()>,
}

/// Handle for an active event stream task
struct EventStreamTask {
    /// Sender kept to check channel health via `is_closed()` if needed.
    /// The task uses a clone of this sender.
    #[allow(dead_code)]
    sender: mpsc::Sender<NeonEventData>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Task join handle
    _handle: tokio::task::JoinHandle<()>,
}

impl NeonLslManager {
    /// Create a new Neon LSL manager
    ///
    /// # Arguments
    /// * `resolver` - Stream resolver for LSL discovery (shared to maintain cache)
    /// * `inlet_manager` - Inlet manager for consuming LSL streams
    pub fn new(resolver: Arc<StreamResolver>, inlet_manager: Arc<InletManager>) -> Self {
        Self {
            resolver,
            inlet_manager,
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            gaze_tasks: Arc::new(RwLock::new(HashMap::new())),
            event_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Discover Neon devices streaming via LSL
    ///
    /// Scans the network for LSL streams matching Neon patterns and groups them
    /// by device name. Returns a list of discovered devices with their capabilities.
    ///
    /// This method uses the shared resolver to maintain stream cache, which is
    /// required for subsequent `connect_gaze_stream()` and `connect_events_stream()` calls.
    pub async fn discover_neon_devices(&self) -> Result<Vec<DiscoveredNeonDevice>, LslError> {
        info!("Discovering Neon devices via LSL...");

        // Discover all streams using the shared resolver (maintains cache for connect calls)
        let all_streams = self.resolver.discover_streams().await?;

        // Filter to only Neon streams
        let streams: Vec<_> = all_streams
            .into_iter()
            .filter(|s| {
                StreamFilter::is_neon_gaze_stream(&s.info.name)
                    || StreamFilter::is_neon_events_stream(&s.info.name)
            })
            .collect();

        // Group streams by device name
        let mut device_map: HashMap<String, DiscoveredNeonDevice> = HashMap::new();
        let now = std::time::SystemTime::now();

        for stream in streams {
            let stream_name = &stream.info.name;

            if let Some(device_name) = StreamFilter::extract_neon_device_name(stream_name) {
                let entry = device_map.entry(device_name.clone()).or_insert_with(|| {
                    DiscoveredNeonDevice {
                        device_name: device_name.clone(),
                        has_gaze_stream: false,
                        has_events_stream: false,
                        gaze_channel_count: 0,
                        gaze_stream_uid: None,
                        events_stream_uid: None,
                        discovered_at: now,
                    }
                });

                if StreamFilter::is_neon_gaze_stream(stream_name) {
                    entry.has_gaze_stream = true;
                    entry.gaze_channel_count = stream.info.channel_count;
                    entry.gaze_stream_uid = Some(stream.uid.clone());
                    debug!(
                        "Found Neon gaze stream: {} ({} channels)",
                        stream_name, stream.info.channel_count
                    );
                } else if StreamFilter::is_neon_events_stream(stream_name) {
                    entry.has_events_stream = true;
                    entry.events_stream_uid = Some(stream.uid.clone());
                    debug!("Found Neon events stream: {}", stream_name);
                }
            }
        }

        // Update our cache
        {
            let mut devices = self.discovered_devices.write().await;
            *devices = device_map.clone();
        }

        let device_list: Vec<DiscoveredNeonDevice> = device_map.into_values().collect();
        info!("Discovered {} Neon device(s)", device_list.len());

        for device in &device_list {
            info!(
                "  - {}: gaze={} ({}ch), events={}",
                device.device_name,
                device.has_gaze_stream,
                device.gaze_channel_count,
                device.has_events_stream
            );
        }

        Ok(device_list)
    }

    /// Get a previously discovered Neon device by name
    pub async fn get_device(&self, device_name: &str) -> Option<DiscoveredNeonDevice> {
        let devices = self.discovered_devices.read().await;
        devices.get(device_name).cloned()
    }

    /// Get all discovered Neon devices
    pub async fn get_all_devices(&self) -> Vec<DiscoveredNeonDevice> {
        let devices = self.discovered_devices.read().await;
        devices.values().cloned().collect()
    }

    /// Connect to a Neon gaze stream
    ///
    /// Returns a channel receiver that will emit gaze data samples.
    /// The stream continues until `disconnect_gaze_stream` is called.
    pub async fn connect_gaze_stream(
        &self,
        device_name: &str,
    ) -> Result<mpsc::Receiver<NeonGazeData>, LslError> {
        info!("Connecting to Neon gaze stream: {}", device_name);

        // Check if already connected
        {
            let tasks = self.gaze_tasks.read().await;
            if tasks.contains_key(device_name) {
                return Err(LslError::LslLibraryError(format!(
                    "Already connected to gaze stream for device: {}",
                    device_name
                )));
            }
        }

        // Get the device info
        let device = self.get_device(device_name).await.ok_or_else(|| {
            LslError::NeonDeviceNotFound(format!(
                "Device not found: {}. Run discover_neon_devices() first.",
                device_name
            ))
        })?;

        if !device.has_gaze_stream {
            return Err(LslError::NeonStreamNotAvailable(format!(
                "No gaze stream available for device: {}",
                device_name
            )));
        }

        let gaze_uid = device.gaze_stream_uid.ok_or_else(|| {
            LslError::NeonStreamNotAvailable("Gaze stream UID not found".to_string())
        })?;

        // Find the stream in the resolver cache
        let stream = self.resolver.get_stream(&gaze_uid).await.ok_or_else(|| {
            LslError::StreamNotFound(format!("Gaze stream not found: {}", gaze_uid))
        })?;

        // Create inlet for this stream
        let inlet_config = InletConfig {
            buffer_size: GAZE_CHANNEL_BUFFER_SIZE,
            max_buflen: 360,
            max_chunklen: 0,
            recover: true,
            ..Default::default()
        };

        self.inlet_manager
            .create_inlet(stream.clone(), Some(inlet_config))
            .await?;

        // Open the inlet
        self.inlet_manager
            .open_inlet(&gaze_uid, Duration::from_secs(10))
            .await?;

        // Create channel for gaze data
        let (tx, rx) = mpsc::channel(GAZE_CHANNEL_BUFFER_SIZE);
        let shutdown = Arc::new(AtomicBool::new(false));

        // Clone tx for the task (we'll store the original)
        let task_tx = tx.clone();

        // Spawn task to read gaze data
        let task_shutdown = shutdown.clone();
        let task_inlet_manager = self.inlet_manager.clone();
        let task_uid = gaze_uid.clone();
        let task_device_name = device_name.to_string();
        let channel_count = device.gaze_channel_count;

        let handle = tokio::spawn(async move {
            info!("Neon gaze stream task started for: {}", task_device_name);

            while !task_shutdown.load(Ordering::Relaxed) {
                // Pull samples with a short timeout
                match task_inlet_manager
                    .pull_sample(&task_uid, Duration::from_millis(100))
                    .await
                {
                    Ok(Some(sample)) => {
                        // Parse gaze data based on channel format
                        if let SampleData::Float32(data) = sample.data {
                            match NeonGazeData::from_lsl_sample(sample.timestamp, &data) {
                                Ok(gaze) => {
                                    if task_tx.send(gaze).await.is_err() {
                                        debug!("Gaze receiver dropped, stopping task");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse Neon gaze data: {}", e);
                                }
                            }
                        } else {
                            warn!(
                                "Unexpected data format for Neon gaze (expected Float32, ch={})",
                                channel_count
                            );
                        }
                    }
                    Ok(None) => {
                        // No data available, continue
                    }
                    Err(e) => {
                        if !task_shutdown.load(Ordering::Relaxed) {
                            warn!("Error pulling Neon gaze sample: {}", e);
                        }
                    }
                }
            }

            info!("Neon gaze stream task stopped for: {}", task_device_name);
        });

        // Store the task
        {
            let mut tasks = self.gaze_tasks.write().await;
            tasks.insert(
                device_name.to_string(),
                GazeStreamTask {
                    sender: tx,
                    shutdown,
                    _handle: handle,
                },
            );
        }

        info!("Connected to Neon gaze stream: {}", device_name);
        Ok(rx)
    }

    /// Connect to a Neon events stream
    ///
    /// Returns a channel receiver that will emit event markers.
    /// The stream continues until `disconnect_events_stream` is called.
    pub async fn connect_events_stream(
        &self,
        device_name: &str,
    ) -> Result<mpsc::Receiver<NeonEventData>, LslError> {
        info!("Connecting to Neon events stream: {}", device_name);

        // Check if already connected
        {
            let tasks = self.event_tasks.read().await;
            if tasks.contains_key(device_name) {
                return Err(LslError::LslLibraryError(format!(
                    "Already connected to events stream for device: {}",
                    device_name
                )));
            }
        }

        // Get the device info
        let device = self.get_device(device_name).await.ok_or_else(|| {
            LslError::NeonDeviceNotFound(format!(
                "Device not found: {}. Run discover_neon_devices() first.",
                device_name
            ))
        })?;

        if !device.has_events_stream {
            return Err(LslError::NeonStreamNotAvailable(format!(
                "No events stream available for device: {}",
                device_name
            )));
        }

        let events_uid = device.events_stream_uid.ok_or_else(|| {
            LslError::NeonStreamNotAvailable("Events stream UID not found".to_string())
        })?;

        // Find the stream in the resolver cache
        let stream = self
            .resolver
            .get_stream(&events_uid)
            .await
            .ok_or_else(|| {
                LslError::StreamNotFound(format!("Events stream not found: {}", events_uid))
            })?;

        // Create inlet for this stream
        let inlet_config = InletConfig {
            buffer_size: EVENT_CHANNEL_BUFFER_SIZE,
            max_buflen: 360,
            max_chunklen: 0,
            recover: true,
            ..Default::default()
        };

        self.inlet_manager
            .create_inlet(stream.clone(), Some(inlet_config))
            .await?;

        // Open the inlet
        self.inlet_manager
            .open_inlet(&events_uid, Duration::from_secs(10))
            .await?;

        // Create channel for event data
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_BUFFER_SIZE);
        let shutdown = Arc::new(AtomicBool::new(false));

        // Clone tx for the task (we'll store the original)
        let task_tx = tx.clone();

        // Spawn task to read event data
        let task_shutdown = shutdown.clone();
        let task_inlet_manager = self.inlet_manager.clone();
        let task_uid = events_uid.clone();
        let task_device_name = device_name.to_string();

        let handle = tokio::spawn(async move {
            info!("Neon events stream task started for: {}", task_device_name);

            while !task_shutdown.load(Ordering::Relaxed) {
                // Pull samples with a longer timeout (events are irregular)
                match task_inlet_manager
                    .pull_sample(&task_uid, Duration::from_millis(500))
                    .await
                {
                    Ok(Some(sample)) => {
                        // Parse event data
                        if let SampleData::String(data) = sample.data {
                            match NeonEventData::from_lsl_sample(sample.timestamp, &data) {
                                Ok(event) => {
                                    debug!("Neon event: {} @ {}", event.event_name, event.timestamp);
                                    if task_tx.send(event).await.is_err() {
                                        debug!("Events receiver dropped, stopping task");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse Neon event data: {}", e);
                                }
                            }
                        } else {
                            warn!("Unexpected data format for Neon events (expected String)");
                        }
                    }
                    Ok(None) => {
                        // No data available, continue
                    }
                    Err(e) => {
                        if !task_shutdown.load(Ordering::Relaxed) {
                            warn!("Error pulling Neon event sample: {}", e);
                        }
                    }
                }
            }

            info!("Neon events stream task stopped for: {}", task_device_name);
        });

        // Store the task
        {
            let mut tasks = self.event_tasks.write().await;
            tasks.insert(
                device_name.to_string(),
                EventStreamTask {
                    sender: tx,
                    shutdown,
                    _handle: handle,
                },
            );
        }

        info!("Connected to Neon events stream: {}", device_name);
        Ok(rx)
    }

    /// Disconnect from a Neon gaze stream
    pub async fn disconnect_gaze_stream(&self, device_name: &str) -> Result<(), LslError> {
        info!("Disconnecting Neon gaze stream: {}", device_name);

        let task = {
            let mut tasks = self.gaze_tasks.write().await;
            tasks.remove(device_name)
        };

        if let Some(task) = task {
            // Signal shutdown
            task.shutdown.store(true, Ordering::Relaxed);
            // Wait for task to complete (with timeout)
            let _ = tokio::time::timeout(Duration::from_secs(2), task._handle).await;
        }

        // Close the inlet
        if let Some(device) = self.get_device(device_name).await {
            if let Some(uid) = device.gaze_stream_uid {
                let _ = self.inlet_manager.close_inlet(&uid).await;
                let _ = self.inlet_manager.remove_inlet(&uid).await;
            }
        }

        info!("Disconnected Neon gaze stream: {}", device_name);
        Ok(())
    }

    /// Disconnect from a Neon events stream
    pub async fn disconnect_events_stream(&self, device_name: &str) -> Result<(), LslError> {
        info!("Disconnecting Neon events stream: {}", device_name);

        let task = {
            let mut tasks = self.event_tasks.write().await;
            tasks.remove(device_name)
        };

        if let Some(task) = task {
            // Signal shutdown
            task.shutdown.store(true, Ordering::Relaxed);
            // Wait for task to complete (with timeout)
            let _ = tokio::time::timeout(Duration::from_secs(2), task._handle).await;
        }

        // Close the inlet
        if let Some(device) = self.get_device(device_name).await {
            if let Some(uid) = device.events_stream_uid {
                let _ = self.inlet_manager.close_inlet(&uid).await;
                let _ = self.inlet_manager.remove_inlet(&uid).await;
            }
        }

        info!("Disconnected Neon events stream: {}", device_name);
        Ok(())
    }

    /// Disconnect all streams for a device
    pub async fn disconnect(&self, device_name: &str) -> Result<(), LslError> {
        let _ = self.disconnect_gaze_stream(device_name).await;
        let _ = self.disconnect_events_stream(device_name).await;
        Ok(())
    }

    /// Disconnect all Neon streams
    pub async fn disconnect_all(&self) -> Result<(), LslError> {
        let device_names: Vec<String> = {
            let devices = self.discovered_devices.read().await;
            devices.keys().cloned().collect()
        };

        for device_name in device_names {
            let _ = self.disconnect(&device_name).await;
        }

        Ok(())
    }

    /// Check if gaze stream is connected for a device
    pub async fn is_gaze_connected(&self, device_name: &str) -> bool {
        let tasks = self.gaze_tasks.read().await;
        tasks.contains_key(device_name)
    }

    /// Check if events stream is connected for a device
    pub async fn is_events_connected(&self, device_name: &str) -> bool {
        let tasks = self.event_tasks.read().await;
        tasks.contains_key(device_name)
    }

    /// Get statistics for Neon streams
    pub async fn get_stats(&self) -> serde_json::Value {
        let devices = self.discovered_devices.read().await;
        let gaze_tasks = self.gaze_tasks.read().await;
        let event_tasks = self.event_tasks.read().await;

        let device_stats: Vec<serde_json::Value> = devices
            .values()
            .map(|d| {
                serde_json::json!({
                    "device_name": d.device_name,
                    "has_gaze_stream": d.has_gaze_stream,
                    "has_events_stream": d.has_events_stream,
                    "gaze_channel_count": d.gaze_channel_count,
                    "gaze_connected": gaze_tasks.contains_key(&d.device_name),
                    "events_connected": event_tasks.contains_key(&d.device_name),
                })
            })
            .collect();

        serde_json::json!({
            "discovered_device_count": devices.len(),
            "active_gaze_streams": gaze_tasks.len(),
            "active_event_streams": event_tasks.len(),
            "devices": device_stats
        })
    }
}

impl std::fmt::Debug for NeonLslManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeonLslManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neon_gaze_data_basic() {
        let gaze = NeonGazeData::from_basic(1.0, 0.5, 0.5);
        assert_eq!(gaze.gaze_x, 0.5);
        assert_eq!(gaze.gaze_y, 0.5);
        assert!(gaze.pupil_diameter.is_none());
        assert!(gaze.eyeball_center.is_none());
    }

    #[test]
    fn test_neon_gaze_data_full() {
        let gaze = NeonGazeData::from_full(1.0, 0.5, 0.5, 3.5, 0.0, 0.0, 0.5);
        assert_eq!(gaze.gaze_x, 0.5);
        assert_eq!(gaze.gaze_y, 0.5);
        assert_eq!(gaze.pupil_diameter, Some(3.5));
        assert_eq!(gaze.eyeball_center, Some((0.0, 0.0, 0.5)));
    }

    #[test]
    fn test_neon_gaze_from_lsl_sample_basic() {
        let data = vec![0.3, 0.7];
        let gaze = NeonGazeData::from_lsl_sample(1.5, &data).unwrap();
        assert_eq!(gaze.gaze_x, 0.3);
        assert_eq!(gaze.gaze_y, 0.7);
        assert!(gaze.pupil_diameter.is_none());
    }

    #[test]
    fn test_neon_gaze_from_lsl_sample_full() {
        let data = vec![0.3, 0.7, 4.0, 0.1, 0.2, 0.3];
        let gaze = NeonGazeData::from_lsl_sample(1.5, &data).unwrap();
        assert_eq!(gaze.gaze_x, 0.3);
        assert_eq!(gaze.gaze_y, 0.7);
        assert_eq!(gaze.pupil_diameter, Some(4.0));
        assert_eq!(gaze.eyeball_center, Some((0.1, 0.2, 0.3)));
    }

    #[test]
    fn test_neon_gaze_from_lsl_sample_invalid() {
        let data = vec![0.3, 0.7, 4.0]; // 3 channels - invalid
        let result = NeonGazeData::from_lsl_sample(1.5, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_neon_event_data() {
        let event = NeonEventData::new(2.5, "stimulus_start".to_string());
        assert_eq!(event.timestamp, 2.5);
        assert_eq!(event.event_name, "stimulus_start");
    }

    #[test]
    fn test_neon_event_from_lsl_sample() {
        let data = vec!["response_button".to_string()];
        let event = NeonEventData::from_lsl_sample(3.0, &data).unwrap();
        assert_eq!(event.event_name, "response_button");
    }

    #[test]
    fn test_neon_event_from_lsl_sample_empty() {
        let data: Vec<String> = vec![];
        let result = NeonEventData::from_lsl_sample(3.0, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_filter_neon_gaze() {
        let filter = StreamFilter::neon_gaze();
        assert_eq!(filter.name_pattern, Some("_Neon Gaze".to_string()));
    }

    #[test]
    fn test_stream_filter_is_neon_stream() {
        assert!(StreamFilter::is_neon_gaze_stream("MyNeon_Neon Gaze"));
        assert!(!StreamFilter::is_neon_gaze_stream("MyNeon_Neon Events"));
        assert!(StreamFilter::is_neon_events_stream("MyNeon_Neon Events"));
        assert!(!StreamFilter::is_neon_events_stream("MyNeon_Neon Gaze"));
    }

    #[test]
    fn test_extract_neon_device_name() {
        assert_eq!(
            StreamFilter::extract_neon_device_name("MyNeon_Neon Gaze"),
            Some("MyNeon".to_string())
        );
        assert_eq!(
            StreamFilter::extract_neon_device_name("Lab Neon 1_Neon Events"),
            Some("Lab Neon 1".to_string())
        );
        assert_eq!(
            StreamFilter::extract_neon_device_name("SomeOtherStream"),
            None
        );
    }
}
