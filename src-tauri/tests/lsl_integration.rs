// LSL Integration Tests for CI/CD Pipeline
// These tests are designed to run in automated environments without requiring actual LSL hardware

use hyperstudy_bridge::devices::lsl::{LSLDevice, LSLStreamConfig, LSLSample};
use hyperstudy_bridge::devices::{Device, DeviceStatus, DeviceType};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, timeout};

/// Test LSL device basic lifecycle
#[tokio::test]
async fn test_lsl_device_lifecycle() {
    let mut device = LSLDevice::new("test_lsl".to_string(), "Test LSL Device".to_string());

    // Initial state
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    assert_eq!(device.get_info().device_type, DeviceType::LSL);

    // Connect
    let result = device.connect().await;
    assert!(result.is_ok());
    assert_eq!(device.get_status(), DeviceStatus::Connected);

    // Disconnect
    let result = device.disconnect().await;
    assert!(result.is_ok());
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);
}

/// Test LSL stream creation and configuration
#[tokio::test]
async fn test_lsl_stream_creation() {
    let device = LSLDevice::new("stream_test".to_string(), "Stream Test Device".to_string());

    let config = LSLStreamConfig {
        name: "TestStream".to_string(),
        stream_type: "EEG".to_string(),
        channel_count: 8,
        sampling_rate: 250.0,
        channel_format: "float32".to_string(),
        source_id: Some("test_source".to_string()),
        metadata: HashMap::new(),
    };

    // Create outlet
    let result = device.create_outlet("test_outlet".to_string(), config.clone()).await;
    assert!(result.is_ok());

    // Create inlet
    let result = device.create_inlet("test_inlet".to_string(), "TestStream".to_string()).await;
    assert!(result.is_ok());
}

/// Test LSL data flow
#[tokio::test]
async fn test_lsl_data_flow() {
    let device = LSLDevice::new("data_flow_test".to_string(), "Data Flow Test Device".to_string());

    let config = LSLStreamConfig {
        channel_count: 4,
        sampling_rate: 100.0,
        ..Default::default()
    };

    device.create_outlet("data_outlet".to_string(), config).await.unwrap();

    // Create test sample
    let sample = LSLSample {
        data: vec![1.0, 2.0, 3.0, 4.0],
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        metadata: HashMap::new(),
    };

    // Push sample
    let result = device.push_to_outlet("data_outlet", sample).await;
    assert!(result.is_ok());
}

/// Test LSL device configuration
#[tokio::test]
async fn test_lsl_device_configuration() {
    let mut device = LSLDevice::new("config_test".to_string(), "Config Test Device".to_string());

    let config = hyperstudy_bridge::devices::DeviceConfig {
        auto_reconnect: false,
        reconnect_interval_ms: 2000,
        timeout_ms: 10000,
        custom_settings: serde_json::json!({
            "buffer_size": 1000,
            "compression": true
        }),
    };

    let result = device.configure(config);
    assert!(result.is_ok());
}

/// Test LSL error handling
#[tokio::test]
async fn test_lsl_error_handling() {
    let device = LSLDevice::new("error_test".to_string(), "Error Test Device".to_string());

    // Try to push to non-existent outlet
    let sample = LSLSample {
        data: vec![1.0],
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        metadata: HashMap::new(),
    };

    let result = device.push_to_outlet("nonexistent", sample).await;
    assert!(result.is_err());

    // Try to pull from non-existent inlet
    let result = device.pull_from_inlet("nonexistent", 100).await;
    assert!(result.is_err());
}

/// Test LSL time synchronization
#[tokio::test]
async fn test_lsl_time_synchronization() {
    let device = LSLDevice::new("time_test".to_string(), "Time Test Device".to_string());

    // Test clock synchronization
    let result = device.synchronize_clock().await;
    assert!(result.is_ok());

    // Test timestamp generation
    let timestamp1 = device.get_lsl_timestamp().await;
    sleep(Duration::from_millis(10)).await;
    let timestamp2 = device.get_lsl_timestamp().await;

    // Timestamps should be increasing
    assert!(timestamp2 > timestamp1);

    // Time difference should be reasonable (around 10ms)
    let diff = timestamp2 - timestamp1;
    assert!(diff >= 0.01); // At least 10ms
    assert!(diff < 0.1);   // But less than 100ms
}

/// Test LSL stream discovery
#[tokio::test]
async fn test_lsl_stream_discovery() {
    let device = LSLDevice::new("discovery_test".to_string(), "Discovery Test Device".to_string());

    // Test discovery without filter
    let streams = device.discover_streams(None).await.unwrap();
    // For mock implementation, should return empty list
    assert!(streams.is_empty());

    // Test discovery with filter
    let eeg_streams = device.discover_streams(Some("EEG")).await.unwrap();
    assert!(eeg_streams.is_empty());
}

/// Test LSL performance under load
#[tokio::test]
async fn test_lsl_performance() {
    let device = LSLDevice::new("perf_test".to_string(), "Performance Test Device".to_string());

    let config = LSLStreamConfig {
        channel_count: 64,
        sampling_rate: 1000.0,
        ..Default::default()
    };

    device.create_outlet("perf_outlet".to_string(), config).await.unwrap();

    let start_time = std::time::Instant::now();
    let sample_count = 1000;

    // Push many samples
    for i in 0..sample_count {
        let sample = LSLSample {
            data: (0..64).map(|j| (i * 64 + j) as f64 * 0.001).collect(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            metadata: HashMap::new(),
        };

        device.push_to_outlet("perf_outlet", sample).await.unwrap();
    }

    let duration = start_time.elapsed();
    let samples_per_second = sample_count as f64 / duration.as_secs_f64();

    // Should handle at least 1000 samples/second
    assert!(samples_per_second >= 500.0); // Relaxed for CI environment
}

/// Test LSL concurrent access
#[tokio::test]
async fn test_lsl_concurrent_access() {
    use std::sync::Arc;

    let device = Arc::new(LSLDevice::new("concurrent_test".to_string(), "Concurrent Test Device".to_string()));

    let config = LSLStreamConfig {
        channel_count: 1,
        ..Default::default()
    };

    device.create_outlet("concurrent_outlet".to_string(), config).await.unwrap();

    let mut handles = Vec::new();

    // Launch multiple concurrent tasks
    for i in 0..5 {
        let device_clone = Arc::clone(&device);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let sample = LSLSample {
                    data: vec![i as f64 * 10.0 + j as f64],
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
                    metadata: HashMap::new(),
                };

                // This should not panic or cause race conditions
                let _result = device_clone.push_to_outlet("concurrent_outlet", sample).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

/// Test LSL with timeout scenarios
#[tokio::test]
async fn test_lsl_timeouts() {
    let device = LSLDevice::new("timeout_test".to_string(), "Timeout Test Device".to_string());

    device.create_inlet("timeout_inlet".to_string(), "NonExistentStream".to_string()).await.unwrap();

    // Test pull with short timeout
    let result = timeout(
        Duration::from_millis(100),
        device.pull_from_inlet("timeout_inlet", 50)
    ).await;

    // Should either timeout or return None quickly
    assert!(result.is_ok()); // Timeout wrapper should succeed
    let inner_result = result.unwrap();
    // The actual result might be Ok(None) or Err depending on implementation
    match inner_result {
        Ok(None) => {}, // Expected for mock
        Err(_) => {},   // Also acceptable
        Ok(Some(_)) => panic!("Unexpected data from non-existent stream"),
    }
}

/// Test LSL device heartbeat functionality
#[tokio::test]
async fn test_lsl_heartbeat() {
    let mut device = LSLDevice::new("heartbeat_test".to_string(), "Heartbeat Test Device".to_string());

    device.connect().await.unwrap();

    // Test heartbeat
    let result = device.heartbeat().await;
    assert!(result.is_ok());

    // Test heartbeat updates status appropriately
    let status = device.get_status();
    assert!(matches!(status, DeviceStatus::Connected | DeviceStatus::Disconnected));
}

/// Test LSL data validation
#[tokio::test]
async fn test_lsl_data_validation() {
    let device = LSLDevice::new("validation_test".to_string(), "Validation Test Device".to_string());

    let config = LSLStreamConfig {
        channel_count: 2,
        ..Default::default()
    };

    device.create_outlet("validation_outlet".to_string(), config).await.unwrap();

    // Test with correct channel count
    let valid_sample = LSLSample {
        data: vec![1.0, 2.0], // 2 channels as expected
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        metadata: HashMap::new(),
    };

    let result = device.push_to_outlet("validation_outlet", valid_sample).await;
    assert!(result.is_ok());

    // Test with incorrect channel count
    let invalid_sample = LSLSample {
        data: vec![1.0, 2.0, 3.0], // 3 channels, but expecting 2
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        metadata: HashMap::new(),
    };

    let result = device.push_to_outlet("validation_outlet", invalid_sample).await;
    assert!(result.is_err());
}

/// Test LSL stream configuration validation
#[tokio::test]
async fn test_lsl_stream_config_validation() {
    let device = LSLDevice::new("config_validation_test".to_string(), "Config Validation Test Device".to_string());

    // Test valid configuration
    let valid_config = LSLStreamConfig {
        name: "ValidStream".to_string(),
        stream_type: "EEG".to_string(),
        channel_count: 64,
        sampling_rate: 1000.0,
        channel_format: "float32".to_string(),
        source_id: Some("valid_source".to_string()),
        metadata: HashMap::new(),
    };

    let result = device.create_outlet("valid_outlet".to_string(), valid_config).await;
    assert!(result.is_ok());

    // Test invalid configuration (empty name should be caught by LSLOutlet::new)
    let invalid_config = LSLStreamConfig {
        name: "".to_string(), // Empty name
        ..Default::default()
    };

    let result = device.create_outlet("invalid_outlet".to_string(), invalid_config).await;
    assert!(result.is_err());
}

/// Test integration with Device trait
#[tokio::test]
async fn test_lsl_device_trait_integration() {
    let mut device = LSLDevice::new("trait_test".to_string(), "Trait Test Device".to_string());

    // Test Device trait methods
    let info = device.get_info();
    assert_eq!(info.device_type, DeviceType::LSL);
    assert_eq!(info.id, "trait_test");
    assert_eq!(info.name, "Trait Test Device");

    // Test send/receive through Device trait
    device.connect().await.unwrap();

    let config = LSLStreamConfig::default();
    device.create_outlet("trait_outlet".to_string(), config.clone()).await.unwrap();
    device.create_inlet("trait_inlet".to_string(), "TestStream".to_string()).await.unwrap();

    // Test send
    let sample = LSLSample {
        data: vec![1.0, 2.0, 3.0],
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
        metadata: HashMap::new(),
    };

    let json_data = serde_json::to_vec(&sample).unwrap();
    let result = device.send(&json_data).await;
    assert!(result.is_ok());

    // Test receive (will timeout for mock implementation)
    let result = timeout(Duration::from_millis(100), device.receive()).await;
    // Should timeout since mock doesn't produce data
    assert!(result.is_err() || matches!(result.unwrap(), Err(_)));
}