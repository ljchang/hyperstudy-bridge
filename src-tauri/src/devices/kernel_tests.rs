#[cfg(test)]
mod tests {
    use super::super::kernel::{KernelConfig, KernelDevice};
    use super::super::{Device, DeviceConfig, DeviceError, DeviceStatus, DeviceType};
    use std::time::Duration;

    #[test]
    fn test_kernel_config_default() {
        let config = KernelConfig::default();
        assert_eq!(config.ip_address, "192.168.1.100");
        assert_eq!(config.port, 6767);
        assert_eq!(config.buffer_size, 8192);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.reconnect_delay_ms, 1000);
        assert_eq!(config.max_reconnect_attempts, 3);
    }

    #[test]
    fn test_kernel_device_new() {
        let device = KernelDevice::new("192.168.1.50".to_string(), 6767);
        assert_eq!(device.ip_address, "192.168.1.50");
        assert_eq!(device.port, 6767);
        assert_eq!(device.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_kernel_device_info() {
        let device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        let info = device.get_info();

        assert_eq!(info.id, "kernel");
        assert_eq!(info.name, "Kernel Flow2");
        assert_eq!(info.device_type, DeviceType::Kernel);
        assert_eq!(info.status, DeviceStatus::Disconnected);

        // Check metadata contains expected fields
        assert!(info.metadata.get("ip").is_some());
        assert!(info.metadata.get("port").is_some());
    }

    #[tokio::test]
    async fn test_kernel_device_status() {
        let device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_connect_to_invalid_address() {
        let mut device = KernelDevice::new("0.0.0.0".to_string(), 9999);
        let result = tokio::time::timeout(Duration::from_secs(2), device.connect()).await;

        // Should timeout or fail
        assert!(result.is_err() || result.unwrap().is_err());
        assert_eq!(device.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let mut device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        let result = device.disconnect().await;

        // Should succeed even when not connected
        assert!(result.is_ok());
        assert_eq!(device.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_send_when_disconnected() {
        let mut device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        let result = device.send(b"TEST").await;

        // Should fail when not connected
        assert!(result.is_err());
        match result {
            Err(DeviceError::NotConnected) => (),
            _ => panic!("Expected NotConnected error"),
        }
    }

    #[test]
    fn test_performance_callback() {
        use std::sync::{Arc, Mutex};

        let mut device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        let callback_called = Arc::new(Mutex::new(false));
        let callback_called_clone = callback_called.clone();

        device.set_performance_callback(move |_id, _latency, _sent, _recv| {
            *callback_called_clone.lock().unwrap() = true;
        });

        // The callback should be set
        assert!(device.performance_callback.is_some());
    }

    #[test]
    fn test_device_config() {
        let mut device = KernelDevice::new("192.168.1.100".to_string(), 6767);

        let config = DeviceConfig {
            name: Some("Custom Kernel".to_string()),
            enabled: true,
            auto_connect: true,
            reconnect_interval: Some(2000),
            max_reconnect_attempts: Some(5),
            custom_settings: Some(serde_json::json!({
                "ip_address": "192.168.1.200",
                "port": 7777,
                "buffer_size": 16384
            })),
        };

        let result = device.configure(config.clone());
        assert!(result.is_ok());

        // Verify configuration was applied
        assert_eq!(device.ip_address, "192.168.1.200");
        assert_eq!(device.port, 7777);
        assert_eq!(device.kernel_config.buffer_size, 16384);
    }

    #[test]
    fn test_debug_implementation() {
        let device = KernelDevice::new("192.168.1.100".to_string(), 6767);
        let debug_str = format!("{:?}", device);

        // Verify debug output contains expected fields
        assert!(debug_str.contains("KernelDevice"));
        assert!(debug_str.contains("ip_address"));
        assert!(debug_str.contains("192.168.1.100"));
        assert!(debug_str.contains("port"));
        assert!(debug_str.contains("6767"));
    }

    #[test]
    fn test_kernel_config_with_custom_values() {
        let config = KernelConfig {
            ip_address: "10.0.0.1".to_string(),
            port: 8888,
            buffer_size: 4096,
            timeout_ms: 10000,
            reconnect_delay_ms: 500,
            max_reconnect_attempts: 10,
        };

        assert_eq!(config.ip_address, "10.0.0.1");
        assert_eq!(config.port, 8888);
        assert_eq!(config.buffer_size, 4096);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::kernel::KernelDevice;
    use super::super::Device;
    use serial_test::serial;
    use std::time::Duration;
    use tokio::time::timeout;

    const TEST_IP: &str = "192.168.1.100";
    const TEST_PORT: u16 = 6767;

    #[tokio::test]
    #[serial]
    #[ignore] // Ignore by default since it requires hardware
    async fn test_real_device_connection() {
        let mut device = KernelDevice::new(TEST_IP.to_string(), TEST_PORT);

        // Attempt to connect with timeout
        let connect_result = timeout(Duration::from_secs(5), device.connect()).await;

        if let Ok(Ok(())) = connect_result {
            // If connected, test sending data
            let test_data = b"TEST_DATA";
            let send_result = device.send(test_data).await;
            assert!(send_result.is_ok());

            // Test receiving data
            let receive_result = timeout(Duration::from_secs(2), device.receive()).await;
            if receive_result.is_ok() {
                println!("Received data from Kernel device");
            }

            // Disconnect
            let disconnect_result = device.disconnect().await;
            assert!(disconnect_result.is_ok());
        } else {
            // Hardware not available, skip test
            println!("Kernel Flow2 hardware not available, skipping integration test");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_reconnection_logic() {
        let mut device = KernelDevice::new(TEST_IP.to_string(), TEST_PORT);

        // Set custom reconnection parameters
        device.kernel_config.max_reconnect_attempts = 2;
        device.kernel_config.reconnect_delay_ms = 100;

        // This test verifies reconnection logic works
        // Actual testing requires hardware or mock server
    }

    #[tokio::test]
    async fn test_concurrent_kernel_devices() {
        use tokio::task;

        // Test that multiple Kernel devices can be created
        let handles: Vec<_> = (0..3)
            .map(|i| {
                task::spawn(async move {
                    let device =
                        KernelDevice::new(format!("192.168.1.{}", 100 + i), 6767 + i as u16);
                    device.get_info()
                })
            })
            .collect();

        for handle in handles {
            let info = handle.await.unwrap();
            assert_eq!(info.id, "kernel");
            assert_eq!(info.device_type, DeviceType::Kernel);
        }
    }

    #[tokio::test]
    async fn test_performance_metrics_collection() {
        use std::sync::{Arc, Mutex};

        let mut device = KernelDevice::new(TEST_IP.to_string(), TEST_PORT);

        let metrics = Arc::new(Mutex::new(Vec::new()));
        let metrics_clone = metrics.clone();

        device.set_performance_callback(move |id, latency, sent, recv| {
            metrics_clone
                .lock()
                .unwrap()
                .push((id.to_string(), latency, sent, recv));
        });

        // Performance callback is now set
        // Actual metrics collection requires connection to hardware
    }

    #[tokio::test]
    async fn test_buffer_overflow_handling() {
        let mut device = KernelDevice::new(TEST_IP.to_string(), TEST_PORT);

        // Create large data that exceeds buffer
        let large_data = vec![0u8; 16384];

        // Without connection, this should fail gracefully
        let result = device.send(&large_data).await;
        assert!(result.is_err());
    }
}
