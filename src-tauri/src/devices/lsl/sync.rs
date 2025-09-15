use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Time synchronization utilities for LSL integration
#[derive(Debug)]
pub struct TimeSync {
    /// LSL local clock offset relative to system time
    local_clock_offset: Arc<AtomicU64>,
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
            local_clock_offset: Arc::new(AtomicU64::new(0)),
            last_sync_check: Arc::new(AtomicU64::new(0)),
            sync_quality: Arc::new(RwLock::new(SyncQuality::default())),
            enabled,
        }
    }

    /// Get current LSL time
    pub fn lsl_time(&self) -> f64 {
        if !self.enabled {
            return self.system_time();
        }

        // In a real implementation, this would call lsl::local_clock()
        // For now, we'll use a placeholder implementation
        self.system_time() + self.get_clock_offset_seconds()
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

    /// Get clock offset in seconds
    fn get_clock_offset_seconds(&self) -> f64 {
        let offset_ns = self.local_clock_offset.load(Ordering::Relaxed);
        offset_ns as f64 / 1_000_000_000.0
    }

    /// Perform time synchronization with LSL network
    pub async fn synchronize(&self) -> Result<(), super::types::LslError> {
        if !self.enabled {
            return Ok(());
        }

        let mut quality = self.sync_quality.write().await;
        quality.status = SyncStatus::Syncing;

        // In a real implementation, this would:
        // 1. Call lsl::local_clock() multiple times
        // 2. Compare with system time
        // 3. Calculate offset and accuracy
        // 4. Detect clock drift

        // Placeholder implementation
        let start_time = self.system_time();
        let lsl_time = start_time; // Would be lsl::local_clock()
        let offset_ns = ((lsl_time - start_time) * 1_000_000_000.0) as u64;

        self.local_clock_offset.store(offset_ns, Ordering::Relaxed);
        self.last_sync_check.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed,
        );

        // Update quality metrics
        quality.accuracy_ns = 1000.0; // Placeholder: 1μs accuracy
        quality.sync_count += 1;
        quality.last_sync_time = start_time;
        quality.status = SyncStatus::Synced;

        debug!("Time synchronization completed: offset = {}ns, accuracy = {}ns",
               offset_ns, quality.accuracy_ns);

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

        if time_since_sync > 300.0 { // 5 minutes
            // Simulate drift detection
            quality.drift_ppm = 10.0; // 10 ppm drift
            quality.status = SyncStatus::Drifting;

            warn!("Clock drift detected: {} ppm", quality.drift_ppm);

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
}