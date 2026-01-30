// LSL Integration Tests for CI/CD Pipeline
// These tests verify basic LSL device functionality

use hyperstudy_bridge::devices::lsl::LslDevice;
use hyperstudy_bridge::devices::{Device, DeviceConfig, DeviceStatus, DeviceType};

/// Test LSL device basic lifecycle
#[tokio::test]
async fn test_lsl_device_lifecycle() {
    let mut device = LslDevice::new("test_lsl".to_string(), None);

    // Initial state
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    let info = device.get_info();
    assert_eq!(info.device_type, DeviceType::LSL);
    assert_eq!(info.id, "test_lsl");

    // Connect
    let result = device.connect().await;
    assert!(result.is_ok());
    assert_eq!(device.get_status(), DeviceStatus::Connected);

    // Disconnect
    let result = device.disconnect().await;
    assert!(result.is_ok());
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);
}

/// Test LSL device configuration
#[tokio::test]
async fn test_lsl_device_configuration() {
    let mut device = LslDevice::new("config_test".to_string(), None);

    let config = DeviceConfig {
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

/// Test LSL device info
#[tokio::test]
async fn test_lsl_device_info() {
    let device = LslDevice::new("info_test".to_string(), None);
    let info = device.get_info();

    assert_eq!(info.device_type, DeviceType::LSL);
    assert_eq!(info.id, "info_test");
    assert_eq!(info.status, DeviceStatus::Disconnected);
}

/// Test LSL device heartbeat
#[tokio::test]
async fn test_lsl_heartbeat() {
    let mut device = LslDevice::new("heartbeat_test".to_string(), None);

    device.connect().await.unwrap();

    // Test heartbeat when connected
    let result = device.heartbeat().await;
    assert!(result.is_ok());
}

/// Test LSL send when disconnected
#[tokio::test]
async fn test_lsl_send_disconnected() {
    let mut device = LslDevice::new("send_test".to_string(), None);

    // Try to send when not connected
    let result = device.send(b"test data").await;
    // Should fail because not connected
    assert!(result.is_err());
}

/// Test LSL performance callback setup
#[test]
fn test_lsl_performance_callback() {
    use std::sync::{Arc, Mutex};

    let mut device = LslDevice::new("callback_test".to_string(), None);
    let callback_called = Arc::new(Mutex::new(false));
    let callback_called_clone = callback_called.clone();

    device.set_performance_callback(move |_id, _latency, _sent, _recv| {
        *callback_called_clone.lock().unwrap() = true;
    });

    // The callback should be set (we can't directly check, but this shouldn't panic)
}

/// Test concurrent device creation
#[tokio::test]
async fn test_lsl_concurrent_creation() {
    use tokio::task;

    // Test that multiple LSL devices can be created
    let handles: Vec<_> = (0..3)
        .map(|i| {
            task::spawn(async move {
                let device = LslDevice::new(format!("concurrent_{}", i), None);
                device.get_info()
            })
        })
        .collect();

    for handle in handles {
        let info = handle.await.unwrap();
        assert_eq!(info.device_type, DeviceType::LSL);
    }
}

/// Test debug implementation
#[test]
fn test_lsl_debug_implementation() {
    let device = LslDevice::new("debug_test".to_string(), None);
    let debug_str = format!("{:?}", device);

    // Verify debug output contains expected fields
    assert!(debug_str.contains("LslDevice"));
    assert!(debug_str.contains("device_id"));
}
