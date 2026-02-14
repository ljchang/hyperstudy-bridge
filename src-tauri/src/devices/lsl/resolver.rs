use super::types::{ChannelFormat, LslError, StreamInfo, StreamType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

// Import real LSL library
use lsl;

/// Maximum number of discovered streams to cache (prevents unbounded HashMap growth)
const MAX_DISCOVERED_STREAMS: usize = 100;

/// Stream discovery and resolution service
#[derive(Debug)]
pub struct StreamResolver {
    /// Discovered streams cache
    discovered_streams: RwLock<HashMap<String, DiscoveredStream>>,
    /// Stream discovery filters
    filters: Vec<StreamFilter>,
    /// Discovery timeout
    timeout_duration: Duration,
    /// Active discovery flag
    is_discovering: RwLock<bool>,
    /// Discovery result sender (bounded to prevent memory exhaustion)
    #[allow(dead_code)]
    discovery_sender: Option<mpsc::Sender<DiscoveryEvent>>,
}

/// Information about a discovered LSL stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredStream {
    pub info: StreamInfo,
    pub discovered_at: std::time::SystemTime,
    pub last_seen: std::time::SystemTime,
    pub available: bool,
    pub uid: String,
    pub session_id: String,
    pub data_loss: f64,
    /// Timestamps are not stored to prevent unbounded memory growth.
    /// In production, timestamps should be processed immediately rather than accumulated.
    /// This field uses a unit type `()` instead of `Vec<f64>` to eliminate the memory leak vector.
    #[serde(skip)]
    pub time_stamps: (),
}

/// Stream discovery filter criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamFilter {
    /// Filter by stream name (regex pattern)
    pub name_pattern: Option<String>,
    /// Filter by stream type
    pub stream_type: Option<StreamType>,
    /// Filter by source hostname
    pub hostname: Option<String>,
    /// Filter by source ID
    pub source_id: Option<String>,
    /// Minimum channel count
    pub min_channels: Option<u32>,
    /// Maximum channel count
    pub max_channels: Option<u32>,
    /// Required channel format
    pub channel_format: Option<ChannelFormat>,
}

impl StreamFilter {
    /// Create a filter for Pupil Labs Neon gaze streams
    ///
    /// Matches streams with names ending in "_Neon Gaze"
    pub fn neon_gaze() -> Self {
        Self {
            name_pattern: Some("_Neon Gaze".to_string()),
            stream_type: Some(StreamType::Gaze),
            channel_format: Some(ChannelFormat::Float32),
            ..Default::default()
        }
    }

    /// Create a filter for Pupil Labs Neon event marker streams
    ///
    /// Matches streams with names ending in "_Neon Events"
    pub fn neon_events() -> Self {
        Self {
            name_pattern: Some("_Neon Events".to_string()),
            stream_type: Some(StreamType::Markers),
            channel_format: Some(ChannelFormat::String),
            ..Default::default()
        }
    }

    /// Create a filter that matches any Neon stream (gaze or events)
    pub fn neon_any() -> Self {
        // Use a pattern that matches both "_Neon Gaze" and "_Neon Events"
        Self {
            name_pattern: Some("_Neon ".to_string()),
            ..Default::default()
        }
    }

    /// Check if a stream name indicates a Neon gaze stream
    pub fn is_neon_gaze_stream(stream_name: &str) -> bool {
        stream_name.ends_with("_Neon Gaze")
    }

    /// Check if a stream name indicates a Neon events stream
    pub fn is_neon_events_stream(stream_name: &str) -> bool {
        stream_name.ends_with("_Neon Events")
    }

    /// Extract device name from Neon stream name
    ///
    /// For example, "MyNeon_Neon Gaze" -> "MyNeon"
    pub fn extract_neon_device_name(stream_name: &str) -> Option<String> {
        if let Some(pos) = stream_name.find("_Neon Gaze") {
            Some(stream_name[..pos].to_string())
        } else if let Some(pos) = stream_name.find("_Neon Events") {
            Some(stream_name[..pos].to_string())
        } else {
            None
        }
    }

    // ====================================================================
    // FRENZ Brainband stream filters
    // ====================================================================

    /// Check if a stream name matches a known FRENZ stream suffix.
    ///
    /// FRENZ streams are named `{DEVICE_ID}_{suffix}` by the Python LSL bridge.
    /// Naming is case-inconsistent (e.g., `_EEG_raw` vs `_poas` vs `_POSTURE`),
    /// so we do case-insensitive matching.
    pub fn is_frenz_stream(stream_name: &str) -> bool {
        let lower = stream_name.to_lowercase();
        super::types::FRENZ_STREAM_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(&suffix.to_lowercase()))
    }

    /// Extract the device name prefix from a FRENZ stream name.
    ///
    /// For example, "FRENZ_ABC123_EEG_raw" -> "FRENZ_ABC123"
    pub fn extract_frenz_device_name(stream_name: &str) -> Option<String> {
        let lower = stream_name.to_lowercase();
        for suffix in super::types::FRENZ_STREAM_SUFFIXES {
            let suffix_lower = suffix.to_lowercase();
            if lower.ends_with(&suffix_lower) {
                let prefix_len = stream_name.len() - suffix.len();
                return Some(stream_name[..prefix_len].to_string());
            }
        }
        None
    }

    /// Extract the stream suffix from a FRENZ stream name.
    ///
    /// For example, "FRENZ_ABC123_EEG_raw" -> "_EEG_raw"
    /// Returns the canonical suffix (from the constant array) to normalize case.
    pub fn extract_frenz_stream_suffix(stream_name: &str) -> Option<&'static str> {
        let lower = stream_name.to_lowercase();
        for suffix in super::types::FRENZ_STREAM_SUFFIXES {
            let suffix_lower = suffix.to_lowercase();
            if lower.ends_with(&suffix_lower) {
                return Some(suffix);
            }
        }
        None
    }
}

/// Discovery events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    StreamFound(DiscoveredStream),
    StreamLost(String), // Stream UID
    StreamUpdated(DiscoveredStream),
    DiscoveryCompleted,
    DiscoveryError(String),
}

/// Internal struct to hold data extracted from lsl::StreamInfo inside spawn_blocking.
/// This struct contains only Send types (String, primitives) so it can cross thread boundaries.
#[derive(Debug)]
struct ExtractedStreamData {
    name: String,
    stream_type: String,
    channel_count: i32,
    nominal_srate: f64,
    channel_format: lsl::ChannelFormat,
    source_id: String,
    hostname: String,
    uid: String,
    session_id: String,
}

impl StreamResolver {
    /// Create a new stream resolver
    pub fn new(timeout_seconds: f64) -> Self {
        Self {
            discovered_streams: RwLock::new(HashMap::new()),
            filters: Vec::new(),
            timeout_duration: Duration::from_secs_f64(timeout_seconds),
            is_discovering: RwLock::new(false),
            discovery_sender: None,
        }
    }

    /// Create resolver with filters
    pub fn with_filters(timeout_seconds: f64, filters: Vec<StreamFilter>) -> Self {
        Self {
            discovered_streams: RwLock::new(HashMap::new()),
            filters,
            timeout_duration: Duration::from_secs_f64(timeout_seconds),
            is_discovering: RwLock::new(false),
            discovery_sender: None,
        }
    }

    /// Add a discovery filter
    pub fn add_filter(&mut self, filter: StreamFilter) {
        self.filters.push(filter);
    }

    /// Clear all filters
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    /// Start continuous stream discovery
    pub async fn start_discovery(&self) -> Result<mpsc::Receiver<DiscoveryEvent>, LslError> {
        // Use bounded channel to prevent memory exhaustion
        let (_sender, receiver) = mpsc::channel(100);

        let mut is_discovering = self.is_discovering.write().await;
        if *is_discovering {
            return Err(LslError::LslLibraryError(
                "Discovery already running".to_string(),
            ));
        }

        *is_discovering = true;
        drop(is_discovering);

        // Store sender for internal use
        // Note: In a real implementation, this would be handled differently
        // as we can't modify self here due to the async context

        info!(device = "lsl", "Starting LSL stream discovery");

        // Start discovery task
        // Note: This is a simplified implementation
        // In a real implementation, we would use proper async patterns
        info!(device = "lsl", "Discovery task started (placeholder)");

        Ok(receiver)
    }

    /// Stop continuous discovery
    pub async fn stop_discovery(&self) {
        let mut is_discovering = self.is_discovering.write().await;
        *is_discovering = false;
        info!(device = "lsl", "Stopping LSL stream discovery");
    }

    /// Perform one-time stream discovery using real LSL library
    pub async fn discover_streams(&self) -> Result<Vec<DiscoveredStream>, LslError> {
        info!(
            device = "lsl",
            "Performing one-time stream discovery (timeout: {:?})", self.timeout_duration
        );

        let discovery_start = Instant::now();
        let timeout_secs = self.timeout_duration.as_secs_f64();

        // Use spawn_blocking because lsl::resolve_streams is a blocking FFI call.
        // CRITICAL: Extract ALL data from lsl::StreamInfo INSIDE the closure because
        // lsl::StreamInfo contains Rc internally (not Send) and cannot cross thread boundaries.
        let extracted_streams = tokio::task::spawn_blocking(move || {
            let lsl_streams = lsl::resolve_streams(timeout_secs)?;

            // Extract all data inside this closure - return only Send types
            let extracted: Vec<ExtractedStreamData> = lsl_streams
                .into_iter()
                .map(|info| ExtractedStreamData {
                    name: info.stream_name(),
                    stream_type: info.stream_type(),
                    channel_count: info.channel_count(),
                    nominal_srate: info.nominal_srate(),
                    channel_format: info.channel_format(),
                    source_id: info.source_id(),
                    hostname: info.hostname(),
                    uid: info.uid(),
                    session_id: info.session_id(),
                })
                .collect();

            Ok::<_, lsl::Error>(extracted)
        })
        .await
        .map_err(|e| LslError::LslLibraryError(format!("Task join error: {}", e)))?
        .map_err(|e| LslError::LslLibraryError(format!("LSL resolve error: {:?}", e)))?;

        let now = std::time::SystemTime::now();
        let mut discovered = Vec::new();

        for extracted in extracted_streams {
            // Map LSL channel format to our ChannelFormat
            let channel_format = match extracted.channel_format {
                lsl::ChannelFormat::Float32 => ChannelFormat::Float32,
                lsl::ChannelFormat::Double64 => ChannelFormat::Float64,
                lsl::ChannelFormat::String => ChannelFormat::String,
                lsl::ChannelFormat::Int8 => ChannelFormat::Int8,
                lsl::ChannelFormat::Int16 => ChannelFormat::Int16,
                lsl::ChannelFormat::Int32 => ChannelFormat::Int32,
                lsl::ChannelFormat::Int64 => ChannelFormat::Int64,
                _ => ChannelFormat::Float32, // Default fallback
            };

            // Map stream type string to our StreamType enum
            let stream_type = match extracted.stream_type.to_lowercase().as_str() {
                "markers" | "marker" => StreamType::Markers,
                "fnirs" | "nirs" => StreamType::FNIRS,
                "gaze" | "eyetracking" => StreamType::Gaze,
                // EEG, EMG, ECG, and other biosignals map to Biosignals
                "eeg" | "emg" | "ecg" | "biosignals" | "physio" | "physiological" => {
                    StreamType::Biosignals
                }
                // Video, Audio, and others map to Generic
                _ => StreamType::Generic,
            };

            // Apply filters if any
            let stream_info = StreamInfo {
                name: extracted.name,
                stream_type,
                channel_count: extracted.channel_count as u32,
                channel_format,
                nominal_srate: extracted.nominal_srate,
                source_id: extracted.source_id,
                hostname: extracted.hostname,
                metadata: std::collections::HashMap::new(),
            };

            if !self.filters.is_empty() {
                let matches = self
                    .filters
                    .iter()
                    .any(|f| self.matches_filter(&stream_info, f));
                if !matches {
                    continue;
                }
            }

            discovered.push(DiscoveredStream {
                info: stream_info,
                discovered_at: now,
                last_seen: now,
                available: true,
                uid: extracted.uid,
                session_id: extracted.session_id,
                data_loss: 0.0,
                time_stamps: (),
            });
        }

        let discovery_time = discovery_start.elapsed();
        info!(
            device = "lsl",
            "Discovery completed in {:?}, found {} streams",
            discovery_time,
            discovered.len()
        );

        // Update cache with size limit enforcement
        let mut cache = self.discovered_streams.write().await;
        for stream in &discovered {
            cache.insert(stream.uid.clone(), stream.clone());
        }

        // Enforce maximum cache size to prevent unbounded memory growth
        if cache.len() > MAX_DISCOVERED_STREAMS {
            // Remove oldest entries by discovered_at timestamp
            let mut entries: Vec<_> = cache
                .iter()
                .map(|(uid, stream)| (uid.clone(), stream.discovered_at))
                .collect();
            entries.sort_by(|a, b| a.1.cmp(&b.1));

            let to_remove = cache.len() - MAX_DISCOVERED_STREAMS;
            for (uid, _) in entries.into_iter().take(to_remove) {
                cache.remove(&uid);
                debug!(device = "lsl", "Evicted old stream from cache: {}", uid);
            }

            warn!(
                device = "lsl",
                "Stream cache exceeded limit, evicted {} oldest entries", to_remove
            );
        }

        Ok(discovered)
    }

    /// Get all discovered streams
    pub async fn get_discovered_streams(&self) -> Vec<DiscoveredStream> {
        let cache = self.discovered_streams.read().await;
        cache.values().cloned().collect()
    }

    /// Get streams by filter criteria
    pub async fn find_streams(&self, filter: &StreamFilter) -> Vec<DiscoveredStream> {
        let cache = self.discovered_streams.read().await;
        cache
            .values()
            .filter(|stream| self.matches_filter(&stream.info, filter))
            .cloned()
            .collect()
    }

    /// Find streams by name pattern
    pub async fn find_by_name(&self, name_pattern: &str) -> Vec<DiscoveredStream> {
        let filter = StreamFilter {
            name_pattern: Some(name_pattern.to_string()),
            ..Default::default()
        };
        self.find_streams(&filter).await
    }

    /// Find streams by type
    pub async fn find_by_type(&self, stream_type: StreamType) -> Vec<DiscoveredStream> {
        let filter = StreamFilter {
            stream_type: Some(stream_type),
            ..Default::default()
        };
        self.find_streams(&filter).await
    }

    /// Get stream by UID
    pub async fn get_stream(&self, uid: &str) -> Option<DiscoveredStream> {
        let cache = self.discovered_streams.read().await;
        cache.get(uid).cloned()
    }

    /// Check if a stream is available
    pub async fn is_stream_available(&self, uid: &str) -> bool {
        let cache = self.discovered_streams.read().await;
        cache.get(uid).is_some_and(|stream| stream.available)
    }

    /// Remove stale streams from cache
    pub async fn cleanup_stale_streams(&self, max_age: Duration) {
        let mut cache = self.discovered_streams.write().await;
        let now = std::time::SystemTime::now();

        let stale_uids: Vec<String> = cache
            .iter()
            .filter_map(|(uid, stream)| {
                if now.duration_since(stream.last_seen).unwrap_or_default() > max_age {
                    Some(uid.clone())
                } else {
                    None
                }
            })
            .collect();

        for uid in stale_uids {
            cache.remove(&uid);
            debug!(device = "lsl", "Removed stale stream: {}", uid);
        }
    }

    /// Get discovery statistics
    pub async fn get_discovery_stats(&self) -> serde_json::Value {
        let cache = self.discovered_streams.read().await;
        let is_discovering = *self.is_discovering.read().await;

        let mut stats_by_type = HashMap::new();
        for stream in cache.values() {
            *stats_by_type.entry(stream.info.stream_type).or_insert(0) += 1;
        }

        serde_json::json!({
            "total_streams": cache.len(),
            "streams_by_type": stats_by_type,
            "is_discovering": is_discovering,
            "filter_count": self.filters.len(),
            "timeout_seconds": self.timeout_duration.as_secs_f64()
        })
    }

    /// Internal discovery loop for continuous discovery
    /// Uses bounded channel to prevent memory exhaustion from event backlog
    #[allow(dead_code)]
    async fn discovery_loop(&self, sender: mpsc::Sender<DiscoveryEvent>) {
        while *self.is_discovering.read().await {
            match self.discover_streams().await {
                Ok(streams) => {
                    for stream in streams {
                        // Use try_send to avoid blocking and detect backpressure
                        match sender.try_send(DiscoveryEvent::StreamFound(stream)) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                warn!(device = "lsl", "Discovery event channel full, dropping event (receiver too slow)");
                                // Continue - don't block on slow receivers
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                error!(
                                    device = "lsl",
                                    "Discovery event receiver dropped, stopping discovery"
                                );
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(device = "lsl", "Discovery error: {}", e);
                    if sender
                        .try_send(DiscoveryEvent::DiscoveryError(e.to_string()))
                        .is_err()
                    {
                        // Channel closed or full, stop discovery
                        break;
                    }
                }
            }

            // Clean up stale streams
            self.cleanup_stale_streams(Duration::from_secs(30)).await;

            // Wait before next discovery cycle
            sleep(Duration::from_secs(5)).await;
        }

        let _ = sender.try_send(DiscoveryEvent::DiscoveryCompleted);
    }

    /// Check if stream matches filter criteria
    fn matches_filter(&self, stream_info: &StreamInfo, filter: &StreamFilter) -> bool {
        // Check name pattern
        if let Some(pattern) = &filter.name_pattern {
            // Simplified pattern matching (in production, use regex)
            if !stream_info.name.contains(pattern) {
                return false;
            }
        }

        // Check stream type
        if let Some(stream_type) = &filter.stream_type {
            if stream_info.stream_type != *stream_type {
                return false;
            }
        }

        // Check hostname
        if let Some(hostname) = &filter.hostname {
            if stream_info.hostname != *hostname {
                return false;
            }
        }

        // Check source ID
        if let Some(source_id) = &filter.source_id {
            if stream_info.source_id != *source_id {
                return false;
            }
        }

        // Check channel count range
        if let Some(min_channels) = filter.min_channels {
            if stream_info.channel_count < min_channels {
                return false;
            }
        }

        if let Some(max_channels) = filter.max_channels {
            if stream_info.channel_count > max_channels {
                return false;
            }
        }

        // Check channel format
        if let Some(channel_format) = &filter.channel_format {
            if stream_info.channel_format != *channel_format {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolver_creation() {
        let resolver = StreamResolver::new(5.0);
        assert_eq!(resolver.timeout_duration, Duration::from_secs(5));
        assert!(resolver.filters.is_empty());
    }

    #[tokio::test]
    async fn test_filter_matching() {
        let resolver = StreamResolver::new(5.0);
        let stream_info = StreamInfo::ttl_markers("test_device");

        let filter = StreamFilter {
            stream_type: Some(StreamType::Markers),
            ..Default::default()
        };

        assert!(resolver.matches_filter(&stream_info, &filter));

        let filter = StreamFilter {
            stream_type: Some(StreamType::FNIRS),
            ..Default::default()
        };

        assert!(!resolver.matches_filter(&stream_info, &filter));
    }

    #[tokio::test]
    async fn test_stream_discovery() {
        // Use a short timeout since we're testing the API, not waiting for streams
        let resolver = StreamResolver::new(0.5);
        let streams = resolver.discover_streams().await.unwrap();

        // Real LSL discovery may or may not find streams depending on environment
        // Just verify discovery completes without error and caching works
        let cached_streams = resolver.get_discovered_streams().await;
        assert_eq!(streams.len(), cached_streams.len());
    }

    #[tokio::test]
    async fn test_stream_filtering() {
        let resolver = StreamResolver::new(0.5);
        let _ = resolver.discover_streams().await.unwrap();

        // Real LSL won't have mock streams - just verify filtering works without panic
        let _markers = resolver.find_by_type(StreamType::Markers).await;
        let _fnirs = resolver.find_by_type(StreamType::FNIRS).await;
    }
}
