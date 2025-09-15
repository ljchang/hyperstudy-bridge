// LSL Test Utilities
// This module provides helper functions and utilities for testing LSL functionality

use super::*;
use crate::devices::lsl::tests::MockLSLDevice;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Global counter for unique test identifiers
static TEST_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate unique test identifier
pub fn generate_test_id() -> String {
    let id = TEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("test_{}", id)
}

/// Test data generator for various signal types
pub struct TestDataGenerator {
    sample_rate: f64,
    channel_count: usize,
    signal_type: SignalType,
}

#[derive(Debug, Clone)]
pub enum SignalType {
    Sine { frequency: f64, amplitude: f64 },
    Square { frequency: f64, amplitude: f64 },
    Noise { amplitude: f64 },
    Ramp { start: f64, end: f64 },
    Constant { value: f64 },
}

impl TestDataGenerator {
    pub fn new(sample_rate: f64, channel_count: usize, signal_type: SignalType) -> Self {
        Self {
            sample_rate,
            channel_count,
            signal_type,
        }
    }

    pub fn generate_sample(&self, sample_index: usize) -> Vec<f64> {
        let time = sample_index as f64 / self.sample_rate;

        (0..self.channel_count)
            .map(|channel| {
                let channel_offset = channel as f64 * 0.1; // Small offset per channel
                match &self.signal_type {
                    SignalType::Sine { frequency, amplitude } => {
                        amplitude * (2.0 * std::f64::consts::PI * frequency * time + channel_offset).sin()
                    }
                    SignalType::Square { frequency, amplitude } => {
                        let sine_val = (2.0 * std::f64::consts::PI * frequency * time + channel_offset).sin();
                        if sine_val >= 0.0 { *amplitude } else { -amplitude }
                    }
                    SignalType::Noise { amplitude } => {
                        amplitude * (rand::random::<f64>() - 0.5) * 2.0
                    }
                    SignalType::Ramp { start, end } => {
                        start + (end - start) * (time % 1.0) + channel_offset
                    }
                    SignalType::Constant { value } => {
                        value + channel_offset
                    }
                }
            })
            .collect()
    }

    pub fn generate_batch(&self, start_sample: usize, count: usize) -> Vec<Vec<f64>> {
        (start_sample..start_sample + count)
            .map(|i| self.generate_sample(i))
            .collect()
    }
}

/// Mock LSL network for testing multi-device scenarios
pub struct MockLSLNetwork {
    devices: HashMap<String, Arc<MockLSLDevice>>,
    streams: HashMap<String, LSLStreamInfo>,
    is_running: Arc<Mutex<bool>>,
}

impl MockLSLNetwork {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            streams: HashMap::new(),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn start(&mut self) -> Result<(), DeviceError> {
        *self.is_running.lock().await = true;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), DeviceError> {
        *self.is_running.lock().await = false;
        self.devices.clear();
        self.streams.clear();
        Ok(())
    }

    pub fn add_device(&mut self, device_id: String, device: Arc<MockLSLDevice>) {
        self.devices.insert(device_id, device);
    }

    pub fn register_stream(&mut self, stream_id: String, info: LSLStreamInfo) {
        self.streams.insert(stream_id, info);
    }

    pub fn discover_streams(&self, stream_type: Option<&str>) -> Vec<LSLStreamInfo> {
        self.streams
            .values()
            .filter(|info| {
                stream_type.map_or(true, |t| info.stream_type == t)
            })
            .cloned()
            .collect()
    }

    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }
}

/// Performance measurement utilities
pub struct PerformanceTracker {
    name: String,
    start_time: Instant,
    measurements: Vec<Duration>,
}

impl PerformanceTracker {
    pub fn new(name: String) -> Self {
        Self {
            name,
            start_time: Instant::now(),
            measurements: Vec::new(),
        }
    }

    pub fn start_measurement(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn end_measurement(&mut self) {
        self.measurements.push(self.start_time.elapsed());
    }

    pub fn record_measurement(&mut self, duration: Duration) {
        self.measurements.push(duration);
    }

    pub fn get_statistics(&self) -> PerformanceStats {
        if self.measurements.is_empty() {
            return PerformanceStats::default();
        }

        let mut sorted = self.measurements.clone();
        sorted.sort();

        let count = sorted.len();
        let total: Duration = sorted.iter().sum();
        let mean = total / count as u32;

        let median = sorted[count / 2];
        let p95 = sorted[(count as f64 * 0.95) as usize];
        let p99 = sorted[(count as f64 * 0.99) as usize];
        let min = sorted[0];
        let max = sorted[count - 1];

        PerformanceStats {
            name: self.name.clone(),
            count,
            mean,
            median,
            p95,
            p99,
            min,
            max,
        }
    }

    pub fn print_statistics(&self) {
        let stats = self.get_statistics();
        println!("Performance Statistics for {}:", stats.name);
        println!("  Count: {}", stats.count);
        println!("  Mean: {:?}", stats.mean);
        println!("  Median: {:?}", stats.median);
        println!("  95th percentile: {:?}", stats.p95);
        println!("  99th percentile: {:?}", stats.p99);
        println!("  Min: {:?}", stats.min);
        println!("  Max: {:?}", stats.max);
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub name: String,
    pub count: usize,
    pub mean: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            count: 0,
            mean: Duration::ZERO,
            median: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            min: Duration::ZERO,
            max: Duration::ZERO,
        }
    }
}

/// Timing validation utilities
pub struct TimingValidator {
    expected_interval: Duration,
    tolerance: Duration,
    timestamps: Vec<f64>,
}

impl TimingValidator {
    pub fn new(expected_interval: Duration, tolerance: Duration) -> Self {
        Self {
            expected_interval,
            tolerance,
            timestamps: Vec::new(),
        }
    }

    pub fn add_timestamp(&mut self, timestamp: f64) {
        self.timestamps.push(timestamp);
    }

    pub fn validate_timing(&self) -> TimingValidationResult {
        if self.timestamps.len() < 2 {
            return TimingValidationResult {
                is_valid: true,
                mean_interval: Duration::ZERO,
                max_deviation: Duration::ZERO,
                violations: 0,
                total_samples: self.timestamps.len(),
            };
        }

        let mut intervals = Vec::new();
        let mut violations = 0;

        for i in 1..self.timestamps.len() {
            let interval_secs = self.timestamps[i] - self.timestamps[i - 1];
            let interval = Duration::from_secs_f64(interval_secs);
            intervals.push(interval);

            let deviation = if interval > self.expected_interval {
                interval - self.expected_interval
            } else {
                self.expected_interval - interval
            };

            if deviation > self.tolerance {
                violations += 1;
            }
        }

        let mean_interval = intervals.iter().sum::<Duration>() / intervals.len() as u32;
        let max_deviation = intervals
            .iter()
            .map(|&interval| {
                if interval > self.expected_interval {
                    interval - self.expected_interval
                } else {
                    self.expected_interval - interval
                }
            })
            .max()
            .unwrap_or(Duration::ZERO);

        TimingValidationResult {
            is_valid: violations == 0,
            mean_interval,
            max_deviation,
            violations,
            total_samples: self.timestamps.len(),
        }
    }
}

#[derive(Debug)]
pub struct TimingValidationResult {
    pub is_valid: bool,
    pub mean_interval: Duration,
    pub max_deviation: Duration,
    pub violations: usize,
    pub total_samples: usize,
}

/// Data integrity checker
pub struct DataIntegrityChecker {
    expected_sequence: Vec<f64>,
    received_sequence: Vec<f64>,
}

impl DataIntegrityChecker {
    pub fn new() -> Self {
        Self {
            expected_sequence: Vec::new(),
            received_sequence: Vec::new(),
        }
    }

    pub fn add_expected(&mut self, data: Vec<f64>) {
        self.expected_sequence.extend(data);
    }

    pub fn add_received(&mut self, data: Vec<f64>) {
        self.received_sequence.extend(data);
    }

    pub fn check_integrity(&self) -> DataIntegrityResult {
        let expected_len = self.expected_sequence.len();
        let received_len = self.received_sequence.len();

        if expected_len == 0 {
            return DataIntegrityResult {
                is_valid: true,
                loss_rate: 0.0,
                corruption_rate: 0.0,
                missing_samples: 0,
                corrupted_samples: 0,
                total_expected: 0,
            };
        }

        let mut missing_samples = 0;
        let mut corrupted_samples = 0;

        // Check for missing samples
        if received_len < expected_len {
            missing_samples = expected_len - received_len;
        }

        // Check for corrupted samples (compare available data)
        let compare_len = expected_len.min(received_len);
        for i in 0..compare_len {
            let diff = (self.expected_sequence[i] - self.received_sequence[i]).abs();
            if diff > 1e-10 { // Allow for floating point precision
                corrupted_samples += 1;
            }
        }

        let loss_rate = missing_samples as f64 / expected_len as f64;
        let corruption_rate = corrupted_samples as f64 / compare_len as f64;

        DataIntegrityResult {
            is_valid: missing_samples == 0 && corrupted_samples == 0,
            loss_rate,
            corruption_rate,
            missing_samples,
            corrupted_samples,
            total_expected: expected_len,
        }
    }
}

#[derive(Debug)]
pub struct DataIntegrityResult {
    pub is_valid: bool,
    pub loss_rate: f64,
    pub corruption_rate: f64,
    pub missing_samples: usize,
    pub corrupted_samples: usize,
    pub total_expected: usize,
}

/// Test scenario builder for complex multi-device tests
pub struct TestScenarioBuilder {
    devices: HashMap<String, MockStreamConfig>,
    duration: Duration,
    sync_events: Vec<SyncEvent>,
}

#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub timestamp: Duration,
    pub event_type: String,
    pub target_devices: Vec<String>,
    pub data: HashMap<String, serde_json::Value>,
}

impl TestScenarioBuilder {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            duration: Duration::from_secs(1),
            sync_events: Vec::new(),
        }
    }

    pub fn add_device(mut self, device_id: String, config: MockStreamConfig) -> Self {
        self.devices.insert(device_id, config);
        self
    }

    pub fn set_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn add_sync_event(mut self, event: SyncEvent) -> Self {
        self.sync_events.push(event);
        self
    }

    pub fn build(self) -> TestScenario {
        TestScenario {
            devices: self.devices,
            duration: self.duration,
            sync_events: self.sync_events,
        }
    }
}

#[derive(Debug)]
pub struct TestScenario {
    pub devices: HashMap<String, MockStreamConfig>,
    pub duration: Duration,
    pub sync_events: Vec<SyncEvent>,
}

impl TestScenario {
    pub async fn execute(&self) -> TestScenarioResult {
        let start_time = Instant::now();
        let mut results = HashMap::new();

        // Create devices
        let mut device_handles = HashMap::new();
        for (device_id, config) in &self.devices {
            let device = MockLSLDevice::new(device_id.clone(), format!("Test Device {}", device_id));
            device.create_outlet("test_outlet".to_string(), config.clone()).await.unwrap();
            device_handles.insert(device_id.clone(), Arc::new(device));
        }

        // Execute scenario
        let mut event_results = Vec::new();
        for event in &self.sync_events {
            // Wait until event time
            while start_time.elapsed() < event.timestamp {
                sleep(Duration::from_millis(1)).await;
            }

            // Execute event on target devices
            let event_start = Instant::now();
            for device_id in &event.target_devices {
                if let Some(device) = device_handles.get(device_id) {
                    // Execute event-specific logic here
                    // For now, just record that the event was processed
                }
            }

            event_results.push(EventResult {
                event_type: event.event_type.clone(),
                scheduled_time: event.timestamp,
                actual_time: start_time.elapsed(),
                execution_duration: event_start.elapsed(),
                success: true,
            });
        }

        // Wait for scenario completion
        while start_time.elapsed() < self.duration {
            sleep(Duration::from_millis(10)).await;
        }

        // Collect results from all devices
        for (device_id, device) in &device_handles {
            let outlets = device.outlets.read().await;
            if let Some(outlet) = outlets.get("test_outlet") {
                let sample_count = outlet.get_sample_count().await;
                results.insert(device_id.clone(), DeviceResult {
                    sample_count,
                    errors: 0, // Would track actual errors in real implementation
                });
            }
        }

        TestScenarioResult {
            total_duration: start_time.elapsed(),
            device_results: results,
            event_results,
            success: true,
        }
    }
}

#[derive(Debug)]
pub struct TestScenarioResult {
    pub total_duration: Duration,
    pub device_results: HashMap<String, DeviceResult>,
    pub event_results: Vec<EventResult>,
    pub success: bool,
}

#[derive(Debug)]
pub struct DeviceResult {
    pub sample_count: usize,
    pub errors: usize,
}

#[derive(Debug)]
pub struct EventResult {
    pub event_type: String,
    pub scheduled_time: Duration,
    pub actual_time: Duration,
    pub execution_duration: Duration,
    pub success: bool,
}

/// Memory usage tracking for performance tests
pub struct MemoryTracker {
    initial_usage: usize,
    peak_usage: usize,
}

impl MemoryTracker {
    pub fn new() -> Self {
        let initial = get_memory_usage();
        Self {
            initial_usage: initial,
            peak_usage: initial,
        }
    }

    pub fn update(&mut self) {
        let current = get_memory_usage();
        if current > self.peak_usage {
            self.peak_usage = current;
        }
    }

    pub fn get_increase(&self) -> usize {
        self.peak_usage.saturating_sub(self.initial_usage)
    }

    pub fn get_peak(&self) -> usize {
        self.peak_usage
    }
}

// Platform-specific memory usage function
#[cfg(target_os = "macos")]
fn get_memory_usage() -> usize {
    use std::process::Command;

    let output = Command::new("ps")
        .args(&["-o", "rss=", "-p"])
        .arg(std::process::id().to_string())
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"0".to_vec(),
            stderr: Vec::new(),
        });

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0) * 1024 // Convert KB to bytes
}

#[cfg(not(target_os = "macos"))]
fn get_memory_usage() -> usize {
    // Fallback implementation - return a dummy value
    std::process::id() as usize * 1024
}