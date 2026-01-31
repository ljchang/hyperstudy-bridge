use super::types::{ChannelFormat, LslError, StreamInfo, StreamType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

/// Maximum number of discovered streams to cache (prevents unbounded HashMap growth)
const MAX_DISCOVERED_STREAMS: usize = 100;

/// Channel capacity for discovery events (prevents unbounded memory growth)
const DISCOVERY_CHANNEL_CAPACITY: usize = 100;

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

/// Discovery events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    StreamFound(DiscoveredStream),
    StreamLost(String), // Stream UID
    StreamUpdated(DiscoveredStream),
    DiscoveryCompleted,
    DiscoveryError(String),
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

        info!("Starting LSL stream discovery");

        // Start discovery task
        // Note: This is a simplified implementation
        // In a real implementation, we would use proper async patterns
        info!("Discovery task started (placeholder)");

        Ok(receiver)
    }

    /// Stop continuous discovery
    pub async fn stop_discovery(&self) {
        let mut is_discovering = self.is_discovering.write().await;
        *is_discovering = false;
        info!("Stopping LSL stream discovery");
    }

    /// Perform one-time stream discovery
    pub async fn discover_streams(&self) -> Result<Vec<DiscoveredStream>, LslError> {
        info!(
            "Performing one-time stream discovery (timeout: {:?})",
            self.timeout_duration
        );

        let discovery_start = Instant::now();

        // Simulate stream discovery
        // In a real implementation, this would call lsl::resolve_streams()
        let discovered = timeout(self.timeout_duration, self.simulate_discovery())
            .await
            .map_err(|_| LslError::DiscoveryTimeout)?;

        let discovery_time = discovery_start.elapsed();
        info!(
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
                debug!("Evicted old stream from cache: {}", uid);
            }

            warn!(
                "Stream cache exceeded limit, evicted {} oldest entries",
                to_remove
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
            debug!("Removed stale stream: {}", uid);
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
                                warn!("Discovery event channel full, dropping event (receiver too slow)");
                                // Continue - don't block on slow receivers
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                error!("Discovery event receiver dropped, stopping discovery");
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Discovery error: {}", e);
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

    /// Simulate stream discovery (placeholder implementation)
    async fn simulate_discovery(&self) -> Vec<DiscoveredStream> {
        // Simulate discovery delay
        sleep(Duration::from_millis(500)).await;

        let now = std::time::SystemTime::now();
        let mut discovered = Vec::new();

        // Create some mock streams for testing
        // In a real implementation, this would call lsl::resolve_streams()

        // Mock TTL stream
        if self.should_include_mock_stream("ttl") {
            discovered.push(DiscoveredStream {
                info: StreamInfo::ttl_markers("mock_ttl_device"),
                discovered_at: now,
                last_seen: now,
                available: true,
                uid: "mock_ttl_stream_001".to_string(),
                session_id: "session_001".to_string(),
                data_loss: 0.0,
                time_stamps: (),
            });
        }

        // Mock fNIRS stream
        if self.should_include_mock_stream("fnirs") {
            discovered.push(DiscoveredStream {
                info: StreamInfo::kernel_fnirs("mock_kernel_device", 16),
                discovered_at: now,
                last_seen: now,
                available: true,
                uid: "mock_fnirs_stream_001".to_string(),
                session_id: "session_002".to_string(),
                data_loss: 0.1,
                time_stamps: (),
            });
        }

        // Mock gaze stream
        if self.should_include_mock_stream("gaze") {
            discovered.push(DiscoveredStream {
                info: StreamInfo::pupil_gaze("mock_pupil_device"),
                discovered_at: now,
                last_seen: now,
                available: true,
                uid: "mock_gaze_stream_001".to_string(),
                session_id: "session_003".to_string(),
                data_loss: 0.05,
                time_stamps: (),
            });
        }

        debug!("Discovered {} mock streams", discovered.len());
        discovered
    }

    /// Check if mock stream should be included based on filters
    fn should_include_mock_stream(&self, stream_type: &str) -> bool {
        if self.filters.is_empty() {
            return true;
        }

        let mock_type = match stream_type {
            "ttl" => StreamType::Markers,
            "fnirs" => StreamType::FNIRS,
            "gaze" => StreamType::Gaze,
            _ => StreamType::Generic,
        };

        self.filters
            .iter()
            .any(|filter| filter.stream_type.is_none() || filter.stream_type == Some(mock_type))
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
        let resolver = StreamResolver::new(5.0);
        let streams = resolver.discover_streams().await.unwrap();

        // Should find mock streams
        assert!(!streams.is_empty());

        // Check cache is populated
        let cached_streams = resolver.get_discovered_streams().await;
        assert_eq!(streams.len(), cached_streams.len());
    }

    #[tokio::test]
    async fn test_stream_filtering() {
        let resolver = StreamResolver::new(5.0);
        let _ = resolver.discover_streams().await.unwrap();

        let markers = resolver.find_by_type(StreamType::Markers).await;
        assert!(!markers.is_empty());

        let fnirs = resolver.find_by_type(StreamType::FNIRS).await;
        // May or may not be empty depending on mock implementation
    }
}
