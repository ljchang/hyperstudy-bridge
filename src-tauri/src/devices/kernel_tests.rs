#[cfg(test)]
mod tests {
    use crate::devices::kernel::{KernelConfig, KernelDevice};
    use crate::devices::{Device, DeviceConfig, DeviceError, DeviceStatus, DeviceType};
    use std::time::Duration;

    #[test]
    fn test_kernel_config_default() {
        let config = KernelConfig::default();
        assert_eq!(config.ip_address, "127.0.0.1");
        assert_eq!(config.port, 6767);
        assert_eq!(config.buffer_size, 8192);
        assert_eq!(config.connection_timeout_ms, 5000);
        assert_eq!(config.max_reconnect_attempts, 10);
    }

    #[test]
    fn test_kernel_device_new() {
        let device = KernelDevice::new("192.168.1.50".to_string());
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_kernel_device_info() {
        let device = KernelDevice::new("192.168.1.100".to_string());
        let info = device.get_info();

        assert!(info.id.starts_with("kernel_"));
        assert!(info.name.contains("Kernel Flow2"));
        assert_eq!(info.device_type, DeviceType::Kernel);
        assert_eq!(info.status, DeviceStatus::Disconnected);

        // Check metadata contains expected fields
        assert!(info.metadata.get("ip_address").is_some());
        assert!(info.metadata.get("port").is_some());
    }

    #[tokio::test]
    async fn test_kernel_device_status() {
        let device = KernelDevice::new("192.168.1.100".to_string());
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_connect_to_invalid_address() {
        let mut device = KernelDevice::new("0.0.0.0".to_string());
        let result = tokio::time::timeout(Duration::from_secs(2), device.connect()).await;

        // Should timeout or fail
        assert!(result.is_err() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let mut device = KernelDevice::new("192.168.1.100".to_string());
        let result = device.disconnect().await;

        // Should succeed even when not connected
        assert!(result.is_ok());
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_send_when_disconnected() {
        let mut device = KernelDevice::new("192.168.1.100".to_string());
        let result = device.send(b"TEST").await;

        // Should fail when not connected
        assert!(result.is_err());
    }

    #[test]
    fn test_performance_callback() {
        use std::sync::{Arc, Mutex};

        let mut device = KernelDevice::new("192.168.1.100".to_string());
        let callback_called = Arc::new(Mutex::new(false));
        let callback_called_clone = callback_called.clone();

        device.set_performance_callback(move |_id, _latency, _sent, _recv| {
            *callback_called_clone.lock().unwrap() = true;
        });

        // The callback is set (can't directly verify but shouldn't panic)
    }

    #[test]
    fn test_device_config() {
        let mut device = KernelDevice::new("192.168.1.100".to_string());

        let config = DeviceConfig {
            auto_reconnect: true,
            reconnect_interval_ms: 2000,
            timeout_ms: 5000,
            custom_settings: serde_json::json!({
                "ip_address": "192.168.1.200",
                "port": 7777,
                "buffer_size": 16384
            }),
        };

        let result = device.configure(config.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_debug_implementation() {
        let device = KernelDevice::new("192.168.1.100".to_string());
        let debug_str = format!("{:?}", device);

        // Verify debug output contains expected fields
        assert!(debug_str.contains("KernelDevice"));
        assert!(debug_str.contains("ip_address"));
        assert!(debug_str.contains("192.168.1.100"));
    }

    #[test]
    fn test_kernel_config_with_custom_values() {
        let config = KernelConfig {
            ip_address: "10.0.0.1".to_string(),
            port: 8888,
            buffer_size: 4096,
            connection_timeout_ms: 10000,
            heartbeat_interval_ms: 3000,
            max_reconnect_attempts: 10,
            initial_reconnect_delay_ms: 100,
            max_reconnect_delay_ms: 5000,
            enable_heartbeat: true,
        };

        assert_eq!(config.ip_address, "10.0.0.1");
        assert_eq!(config.port, 8888);
        assert_eq!(config.buffer_size, 4096);
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::devices::kernel::KernelDevice;
    use crate::devices::{Device, DeviceType};
    use std::time::Duration;
    use tokio::time::timeout;

    const TEST_IP: &str = "192.168.1.100";

    #[tokio::test]
    #[ignore] // Ignore by default since it requires hardware
    async fn test_real_device_connection() {
        let mut device = KernelDevice::new(TEST_IP.to_string());

        // Attempt to connect with timeout
        let connect_result = timeout(Duration::from_secs(5), device.connect()).await;

        if let Ok(Ok(())) = connect_result {
            // If connected, test sending data
            let test_data = b"TEST_DATA";
            let send_result = device.send(test_data).await;
            assert!(send_result.is_ok());

            // Disconnect
            let disconnect_result = device.disconnect().await;
            assert!(disconnect_result.is_ok());
        } else {
            // Hardware not available, skip test
            println!("Kernel Flow2 hardware not available, skipping integration test");
        }
    }

    #[tokio::test]
    async fn test_concurrent_kernel_devices() {
        use tokio::task;

        // Test that multiple Kernel devices can be created
        let handles: Vec<_> = (0..3)
            .map(|i| {
                task::spawn(async move {
                    let device = KernelDevice::new(format!("192.168.1.{}", 100 + i));
                    device.get_info()
                })
            })
            .collect();

        for handle in handles {
            let info = handle.await.unwrap();
            assert!(info.id.starts_with("kernel_"));
            assert_eq!(info.device_type, DeviceType::Kernel);
        }
    }

    #[tokio::test]
    async fn test_performance_metrics_collection() {
        use std::sync::{Arc, Mutex};

        let mut device = KernelDevice::new(TEST_IP.to_string());

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
        let mut device = KernelDevice::new(TEST_IP.to_string());

        // Create large data that exceeds buffer
        let large_data = vec![0u8; 16384];

        // Without connection, this should fail gracefully
        let result = device.send(&large_data).await;
        assert!(result.is_err());
    }
}
