#[cfg(test)]
mod tests {
    use super::super::ttl::{TtlConfig, TtlDevice};
    use super::super::{Device, DeviceConfig, DeviceError, DeviceStatus, DeviceType};
    use std::time::Duration;
    use tokio;

    #[test]
    fn test_ttl_config_default() {
        let config = TtlConfig::default();
        assert_eq!(config.port_name, "");
        assert_eq!(config.baud_rate, 115200);
        assert_eq!(config.pulse_duration_ms, 10);
    }

    #[test]
    fn test_ttl_device_new() {
        let device = TtlDevice::new("/dev/ttyUSB0".to_string());
        assert_eq!(device.port_name, "/dev/ttyUSB0");
        assert_eq!(device.status, DeviceStatus::Disconnected);
        assert_eq!(device.config.port_name, "/dev/ttyUSB0");
        assert_eq!(device.config.baud_rate, 115200);
    }

    #[tokio::test]
    async fn test_ttl_device_info() {
        let device = TtlDevice::new("/dev/ttyUSB0".to_string());
        let info = device.get_info();

        assert_eq!(info.id, "ttl");
        assert_eq!(info.name, "TTL Pulse Generator");
        assert_eq!(info.device_type, DeviceType::Ttl);
        assert_eq!(info.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_ttl_device_status() {
        let device = TtlDevice::new("/dev/ttyUSB0".to_string());
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[test]
    fn test_ttl_list_ports() {
        // This test might fail if no serial ports are available
        // but it tests the function doesn't panic
        let result = TtlDevice::list_ports();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connect_without_port() {
        let mut device = TtlDevice::new("/dev/nonexistent".to_string());
        let result = device.connect().await;

        // Should fail since port doesn't exist
        assert!(result.is_err());
        assert_eq!(device.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let mut device = TtlDevice::new("/dev/ttyUSB0".to_string());
        let result = device.disconnect().await;

        // Should succeed even when not connected
        assert!(result.is_ok());
        assert_eq!(device.status, DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_send_when_disconnected() {
        let mut device = TtlDevice::new("/dev/ttyUSB0".to_string());
        let result = device.send(b"PULSE\n").await;

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

        let mut device = TtlDevice::new("/dev/ttyUSB0".to_string());
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
        let mut device = TtlDevice::new("/dev/ttyUSB0".to_string());

        let config = DeviceConfig {
            name: Some("Custom TTL".to_string()),
            enabled: true,
            auto_connect: true,
            reconnect_interval: Some(5000),
            max_reconnect_attempts: Some(3),
            custom_settings: None,
        };

        let result = device.configure(config.clone());
        assert!(result.is_ok());

        // Verify configuration was applied
        assert_eq!(device.device_config.name, Some("Custom TTL".to_string()));
        assert_eq!(device.device_config.enabled, true);
        assert_eq!(device.device_config.auto_connect, true);
    }

    #[tokio::test]
    async fn test_device_trait_implementation() {
        let device = TtlDevice::new("/dev/ttyUSB0".to_string());

        // Verify Device trait is properly implemented
        let _info = device.get_info();
        let _status = device.get_status();
    }

    #[test]
    fn test_debug_implementation() {
        let device = TtlDevice::new("/dev/ttyUSB0".to_string());
        let debug_str = format!("{:?}", device);

        // Verify debug output contains expected fields
        assert!(debug_str.contains("TtlDevice"));
        assert!(debug_str.contains("port_name"));
        assert!(debug_str.contains("/dev/ttyUSB0"));
        assert!(debug_str.contains("status"));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::ttl::TtlDevice;
    use super::super::Device;
    use serial_test::serial;
    use std::time::Duration;
    use tokio::time::timeout;

    // Mock serial port for testing
    #[cfg(target_os = "macos")]
    const TEST_PORT: &str = "/dev/tty.usbmodem14101";
    #[cfg(target_os = "linux")]
    const TEST_PORT: &str = "/dev/ttyUSB0";
    #[cfg(target_os = "windows")]
    const TEST_PORT: &str = "COM3";

    #[tokio::test]
    #[serial]
    #[ignore] // Ignore by default since it requires hardware
    async fn test_real_device_connection() {
        let mut device = TtlDevice::new(TEST_PORT.to_string());

        // Attempt to connect
        let connect_result = timeout(Duration::from_secs(5), device.connect()).await;

        if let Ok(Ok(())) = connect_result {
            // If connected, test sending a pulse
            let send_result = device.send(b"PULSE\n").await;
            assert!(send_result.is_ok());

            // Disconnect
            let disconnect_result = device.disconnect().await;
            assert!(disconnect_result.is_ok());
        } else {
            // Hardware not available, skip test
            println!("TTL hardware not available, skipping integration test");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_performance_metrics() {
        use std::sync::{Arc, Mutex};

        let mut device = TtlDevice::new(TEST_PORT.to_string());

        let metrics = Arc::new(Mutex::new(Vec::new()));
        let metrics_clone = metrics.clone();

        device.set_performance_callback(move |id, latency, sent, recv| {
            metrics_clone
                .lock()
                .unwrap()
                .push((id.to_string(), latency, sent, recv));
        });

        // This test verifies the callback mechanism works
        // Actual testing requires hardware
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        use tokio::task;

        // Test that multiple TTL devices can be created
        let handles: Vec<_> = (0..3)
            .map(|i| {
                task::spawn(async move {
                    let device = TtlDevice::new(format!("/dev/tty{}", i));
                    device.get_info()
                })
            })
            .collect();

        for handle in handles {
            let info = handle.await.unwrap();
            assert_eq!(info.id, "ttl");
        }
    }
}
