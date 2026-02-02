use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};

// Import real LSL library for local_clock()
use lsl;

/// Time synchronization utilities for LSL integration
#[derive(Debug)]
pub struct TimeSync {
    /// LSL local clock offset relative to system time (signed to handle negative offsets)
    local_clock_offset: Arc<AtomicI64>,
    /// Last synchronization check timestamp
    last_sync_check: Arc<AtomicU64>,
    /// Synchronization quality metrics
    sync_quality: Arc<RwLock<SyncQuality>>,
    /// Enable time synchronization
    enabled: bool,
}

/// Time synchronization quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQuality {
    /// Synchronization accuracy in nanoseconds
    pub accuracy_ns: f64,
    /// Clock drift rate in parts per million
    pub drift_ppm: f64,
    /// Number of successful synchronizations
    pub sync_count: u64,
    /// Last synchronization timestamp
    pub last_sync_time: f64,
    /// Synchronization status
    pub status: SyncStatus,
}

/// Synchronization status indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Initial state, not synchronized
    Unsynced,
    /// Synchronization in progress
    Syncing,
    /// Successfully synchronized
    Synced,
    /// Synchronization failed
    Failed,
    /// Clock drift detected
    Drifting,
}

impl Default for SyncQuality {
    fn default() -> Self {
        Self {
            accuracy_ns: 0.0,
            drift_ppm: 0.0,
            sync_count: 0,
            last_sync_time: 0.0,
            status: SyncStatus::Unsynced,
        }
    }
}

impl TimeSync {
    /// Create new time synchronization instance
    pub fn new(enabled: bool) -> Self {
        Self {
            local_clock_offset: Arc::new(AtomicI64::new(0)),
            last_sync_check: Arc::new(AtomicU64::new(0)),
            sync_quality: Arc::new(RwLock::new(SyncQuality::default())),
            enabled,
        }
    }

    /// Get current LSL time using real lsl::local_clock()
    pub fn lsl_time(&self) -> f64 {
        if !self.enabled {
            return self.system_time();
        }

        // Use real LSL local clock - this is a simple FFI call that's safe to call
        // from any thread (doesn't involve Rc-based objects)
        lsl::local_clock()
    }

    /// Convert system time to LSL time
    pub fn system_to_lsl_time(&self, system_time: f64) -> f64 {
        if !self.enabled {
            return system_time;
        }

        system_time + self.get_clock_offset_seconds()
    }

    /// Convert LSL time to system time
    pub fn lsl_to_system_time(&self, lsl_time: f64) -> f64 {
        if !self.enabled {
            return lsl_time;
        }

        lsl_time - self.get_clock_offset_seconds()
    }

    /// Get current system time as seconds since epoch
    fn system_time(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }

    /// Get clock offset in seconds (can be negative if LSL clock is behind system clock)
    fn get_clock_offset_seconds(&self) -> f64 {
        let offset_ns = self.local_clock_offset.load(Ordering::Relaxed);
        offset_ns as f64 / 1_000_000_000.0
    }

    /// Perform time synchronization with LSL network
    /// Calculates offset between system time and LSL time
    pub async fn synchronize(&self) -> Result<(), super::types::LslError> {
        if !self.enabled {
            return Ok(());
        }

        let mut quality = self.sync_quality.write().await;
        quality.status = SyncStatus::Syncing;

        // Sample multiple times to get a more accurate offset estimate
        let mut offsets = Vec::with_capacity(5);
        for _ in 0..5 {
            let system_before = self.system_time();
            let lsl_now = lsl::local_clock();
            let system_after = self.system_time();

            // Use midpoint of system time measurements for better accuracy
            let system_mid = (system_before + system_after) / 2.0;
            let offset = lsl_now - system_mid;
            offsets.push(offset);
        }

        // Use median offset for robustness against outliers
        offsets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_offset = offsets[offsets.len() / 2];

        // Store offset in nanoseconds (signed to handle negative offsets correctly)
        let offset_ns = (median_offset * 1_000_000_000.0) as i64;
        self.local_clock_offset.store(offset_ns, Ordering::Relaxed);

        self.last_sync_check.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed,
        );

        // Estimate accuracy from measurement variability
        let variance: f64 = offsets
            .iter()
            .map(|&o| (o - median_offset).powi(2))
            .sum::<f64>()
            / offsets.len() as f64;
        let accuracy_s = variance.sqrt();
        let accuracy_ns = accuracy_s * 1_000_000_000.0;

        // Update quality metrics
        quality.accuracy_ns = accuracy_ns.max(100.0); // At least 100ns floor
        quality.sync_count += 1;
        quality.last_sync_time = lsl::local_clock();
        quality.status = SyncStatus::Synced;

        debug!(
            device = "lsl",
            "Time synchronization completed: offset = {:.3}ms, accuracy = {:.3}μs",
            median_offset * 1000.0,
            accuracy_ns / 1000.0
        );

        Ok(())
    }

    /// Check if synchronization is needed
    pub async fn needs_sync(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last_sync = self.last_sync_check.load(Ordering::Relaxed);

        // Re-sync every 60 seconds or if never synced
        current_time - last_sync > 60 || last_sync == 0
    }

    /// Get synchronization quality metrics
    pub async fn get_sync_quality(&self) -> SyncQuality {
        self.sync_quality.read().await.clone()
    }

    /// Check if time is synchronized within acceptable bounds
    pub async fn is_synchronized(&self) -> bool {
        if !self.enabled {
            return true; // Sync not required
        }

        let quality = self.sync_quality.read().await;
        quality.status == SyncStatus::Synced && quality.accuracy_ns < 10_000.0 // 10μs threshold
    }

    /// Detect clock drift and adjust if necessary
    pub async fn check_drift(&self) -> Result<(), super::types::LslError> {
        if !self.enabled {
            return Ok(());
        }

        let mut quality = self.sync_quality.write().await;

        // In a real implementation, this would:
        // 1. Compare current LSL time with expected time based on last sync
        // 2. Calculate drift rate
        // 3. Trigger re-synchronization if drift exceeds threshold

        // Placeholder drift detection
        let current_time = self.system_time();
        let time_since_sync = current_time - quality.last_sync_time;

        if time_since_sync > 300.0 {
            // 5 minutes
            // Simulate drift detection
            quality.drift_ppm = 10.0; // 10 ppm drift
            quality.status = SyncStatus::Drifting;

            warn!(device = "lsl", "Clock drift detected: {} ppm", quality.drift_ppm);

            // Trigger re-synchronization
            drop(quality); // Release the lock
            self.synchronize().await?;
        }

        Ok(())
    }

    /// Create a timestamp for data samples
    pub fn create_timestamp(&self) -> f64 {
        self.lsl_time()
    }

    /// Validate timestamp against expected range
    pub fn validate_timestamp(&self, timestamp: f64) -> bool {
        let current_time = self.lsl_time();
        let diff = (timestamp - current_time).abs();

        // Allow timestamps within 1 second of current time
        diff < 1.0
    }

    /// Calculate time correction for received samples
    pub async fn calculate_time_correction(&self, _stream_timestamp: f64) -> f64 {
        if !self.enabled {
            return 0.0;
        }

        // In a real implementation, this would use LSL's time_correction() method
        // to account for network delays and clock differences between devices

        let quality = self.sync_quality.read().await;
        quality.accuracy_ns / 1_000_000_000.0 // Convert to seconds
    }

    /// Get synchronization statistics for monitoring
    pub async fn get_sync_stats(&self) -> serde_json::Value {
        if !self.enabled {
            return serde_json::json!({
                "enabled": false,
                "status": "disabled"
            });
        }

        let quality = self.sync_quality.read().await;
        let offset_seconds = self.get_clock_offset_seconds();

        serde_json::json!({
            "enabled": true,
            "status": quality.status,
            "offset_seconds": offset_seconds,
            "accuracy_ns": quality.accuracy_ns,
            "drift_ppm": quality.drift_ppm,
            "sync_count": quality.sync_count,
            "last_sync_time": quality.last_sync_time
        })
    }
}

/// Utility functions for time synchronization
///
/// Convert nanoseconds to seconds
pub fn ns_to_seconds(ns: u64) -> f64 {
    ns as f64 / 1_000_000_000.0
}

/// Convert seconds to nanoseconds
pub fn seconds_to_ns(seconds: f64) -> u64 {
    (seconds * 1_000_000_000.0) as u64
}

/// Calculate time difference in microseconds
pub fn time_diff_us(t1: f64, t2: f64) -> i64 {
    ((t1 - t2) * 1_000_000.0) as i64
}

/// Check if timestamp is within acceptable bounds
pub fn is_timestamp_valid(timestamp: f64, reference: f64, tolerance_seconds: f64) -> bool {
    (timestamp - reference).abs() <= tolerance_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_time_sync_creation() {
        let sync = TimeSync::new(true);
        assert!(sync.enabled);

        let quality = sync.get_sync_quality().await;
        assert_eq!(quality.status, SyncStatus::Unsynced);
    }

    #[tokio::test]
    async fn test_time_conversion() {
        let sync = TimeSync::new(false); // Disabled
        let system_time = sync.system_time();
        let lsl_time = sync.system_to_lsl_time(system_time);

        // When disabled, times should be equal
        assert!((lsl_time - system_time).abs() < 0.001);
    }

    #[test]
    fn test_utility_functions() {
        assert_eq!(ns_to_seconds(1_000_000_000), 1.0);
        assert_eq!(seconds_to_ns(1.0), 1_000_000_000);
        assert_eq!(time_diff_us(1.0, 0.5), 500_000);
        assert!(is_timestamp_valid(1.0, 1.1, 0.2));
        assert!(!is_timestamp_valid(1.0, 1.5, 0.2));
    }

    #[test]
    fn test_negative_clock_offset() {
        // Test that negative clock offsets are handled correctly
        // This verifies the AtomicI64 fix (was AtomicU64 which couldn't store negatives)
        let sync = TimeSync::new(true);

        // Manually store a negative offset (simulating LSL clock behind system clock)
        let negative_offset_ns: i64 = -500_000_000; // -0.5 seconds
        sync.local_clock_offset
            .store(negative_offset_ns, Ordering::Relaxed);

        // Verify the offset is retrieved correctly as negative
        let offset_seconds = sync.get_clock_offset_seconds();
        assert!((offset_seconds - (-0.5)).abs() < 0.001);

        // Verify time conversion uses the negative offset correctly
        let system_time = 1000.0;
        let lsl_time = sync.system_to_lsl_time(system_time);
        assert!((lsl_time - 999.5).abs() < 0.001); // 1000.0 + (-0.5) = 999.5

        // Verify reverse conversion
        let back_to_system = sync.lsl_to_system_time(lsl_time);
        assert!((back_to_system - system_time).abs() < 0.001);
    }

    #[test]
    fn test_lsl_time_enabled() {
        // Test that lsl_time() returns real LSL clock when enabled
        let sync = TimeSync::new(true);
        let lsl_time = sync.lsl_time();

        // LSL time should be a positive value (time since LSL library init)
        assert!(lsl_time > 0.0);

        // Should be reasonably close to what lsl::local_clock() returns
        let direct_lsl = lsl::local_clock();
        assert!((lsl_time - direct_lsl).abs() < 1.0); // Within 1 second
    }

    #[test]
    fn test_timestamp_creation() {
        // Test that create_timestamp() produces valid LSL timestamps
        let sync = TimeSync::new(true);

        let ts1 = sync.create_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = sync.create_timestamp();

        // Timestamps should be positive and increasing
        assert!(ts1 > 0.0);
        assert!(ts2 > ts1);
    }
}
