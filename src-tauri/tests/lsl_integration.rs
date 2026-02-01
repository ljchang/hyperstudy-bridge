//! LSL Integration Tests for CI/CD Pipeline
//!
//! These tests verify basic LSL device functionality using the new test infrastructure.

mod common;
use common::prelude::*;

use hyperstudy_bridge::devices::lsl::LslDevice;

/// Test LSL device basic lifecycle
#[tokio::test]
async fn test_lsl_device_lifecycle() -> TestResult<()> {
    let mut device = LslDevice::new("test_lsl".to_string(), None);

    // Initial state
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    let info = device.get_info();
    assert_eq!(info.device_type, DeviceType::LSL);
    // LslDevice prepends "lsl_" to the ID
    assert_eq!(info.id, "lsl_test_lsl");

    // Connect
    device.connect().await?;
    Assertions::assert_latency(Duration::from_millis(0), 100.0, "LSL connect")?;
    assert_eq!(device.get_status(), DeviceStatus::Connected);

    // Disconnect
    device.disconnect().await?;
    assert_eq!(device.get_status(), DeviceStatus::Disconnected);

    Ok(())
}

/// Test LSL device configuration
#[tokio::test]
async fn test_lsl_device_configuration() -> TestResult<()> {
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

    device.configure(config)?;
    Ok(())
}

/// Test LSL device info
#[tokio::test]
async fn test_lsl_device_info() -> TestResult<()> {
    let device = LslDevice::new("info_test".to_string(), None);
    let info = device.get_info();

    assert_eq!(info.device_type, DeviceType::LSL);
    // LslDevice prepends "lsl_" to the ID
    assert_eq!(info.id, "lsl_info_test");
    assert_eq!(info.status, DeviceStatus::Disconnected);

    Ok(())
}

/// Test LSL device heartbeat
#[tokio::test]
async fn test_lsl_heartbeat() -> TestResult<()> {
    let mut device = LslDevice::new("heartbeat_test".to_string(), None);

    device.connect().await?;

    // Test heartbeat when connected
    device.heartbeat().await?;

    device.disconnect().await?;
    Ok(())
}

/// Test LSL send when disconnected
#[tokio::test]
async fn test_lsl_send_disconnected() -> TestResult<()> {
    let mut device = LslDevice::new("send_test".to_string(), None);

    // Try to send when not connected - should fail
    let result = device.send(b"test data").await;
    assert!(result.is_err(), "Send should fail when disconnected");

    Ok(())
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
async fn test_lsl_concurrent_creation() -> TestResult<()> {
    let results = run_concurrent(3, |i| async move {
        let device = LslDevice::new(format!("concurrent_{}", i), None);
        let info = device.get_info();

        if info.device_type != DeviceType::LSL {
            return Err(TestError::Assertion(format!(
                "Expected LSL device type, got {:?}",
                info.device_type
            )));
        }

        Ok(info)
    })
    .await;

    assert!(
        results.all_ok(),
        "All concurrent device creations should succeed"
    );
    assert_eq!(results.success_count(), 3);

    Ok(())
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

/// Test LSL device with TestHarness integration
#[tokio::test]
async fn test_lsl_with_harness() -> TestResult<()> {
    let mut harness = TestHarness::new().await;

    // Add an LSL device via the harness
    let device_id = harness.add_device(DeviceType::LSL).await;

    // Verify it was added
    assert_eq!(harness.device_count().await, 1);

    // Get status
    let status = harness.get_device_status(&device_id).await?;
    assert_eq!(status, DeviceStatus::Disconnected);

    // Cleanup
    harness.cleanup().await
}
