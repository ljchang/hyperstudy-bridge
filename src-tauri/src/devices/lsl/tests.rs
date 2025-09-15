//! Integration tests for LSL device implementation

#[cfg(test)]
mod tests {
    use crate::devices::lsl::*;
    use crate::devices::{Device, DeviceConfig, DeviceStatus};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_lsl_device_full_lifecycle() {
        // Create LSL device
        let mut device = LslDevice::new("test_lsl".to_string(), None);

        // Test initial state
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
        let info = device.get_info();
        assert_eq!(info.device_type, crate::devices::DeviceType::LSL);
        assert!(info.name.contains("LSL Bridge"));

        // Test connection
        let result = device.connect().await;
        assert!(result.is_ok(), "Failed to connect: {:?}", result);
        assert_eq!(device.get_status(), DeviceStatus::Connected);

        // Test heartbeat
        let result = device.heartbeat().await;
        assert!(result.is_ok(), "Heartbeat failed: {:?}", result);

        // Test sending data (TTL marker)
        let ttl_data = b"\x00PULSE";  // 0 = TTL device type + "PULSE" marker
        let result = device.send(ttl_data).await;
        assert!(result.is_ok(), "Failed to send TTL data: {:?}", result);

        // Test sending data (Kernel fNIRS data)
        let fnirs_data = b"\x01\x00\x00\x80?\x00\x00\x00@\x00\x00@@"; // 1.0, 2.0, 3.0 as f32
        let mut prefixed_data = vec![1u8]; // 1 = Kernel device type
        prefixed_data.extend_from_slice(fnirs_data);
        let result = device.send(&prefixed_data).await;
        assert!(result.is_ok(), "Failed to send fNIRS data: {:?}", result);

        // Test receiving (should not fail, might return empty)
        let result = device.receive().await;
        assert!(result.is_ok(), "Failed to receive data: {:?}", result);

        // Test disconnection
        let result = device.disconnect().await;
        assert!(result.is_ok(), "Failed to disconnect: {:?}", result);
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_lsl_configuration() {
        let mut device = LslDevice::new("test_config".to_string(), None);

        // Test configuration
        let config = DeviceConfig {
            custom_settings: serde_json::json!({
                "auto_create_outlets": false,
                "auto_discover_inlets": true,
                "buffer_size": 2000,
                "enable_time_sync": false
            }),
            ..Default::default()
        };

        let result = device.configure(config);
        assert!(result.is_ok());

        // Verify configuration was applied
        assert!(!device.lsl_config.auto_create_outlets);
        assert!(device.lsl_config.auto_discover_inlets);
        assert_eq!(device.lsl_config.buffer_size, 2000);
        assert!(!device.lsl_config.enable_time_sync);
    }

    #[tokio::test]
    async fn test_stream_discovery() {
        let device = LslDevice::new("test_discovery".to_string(), None);

        // Test stream discovery with no filters
        let result = device.discover_streams(vec![]).await;
        assert!(result.is_ok(), "Discovery failed: {:?}", result);

        // Test with filter for TTL streams
        let filter = StreamFilter {
            stream_type: Some(StreamType::Markers),
            ..Default::default()
        };
        let result = device.discover_streams(vec![filter]).await;
        assert!(result.is_ok(), "Filtered discovery failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_time_synchronization() {
        let time_sync = Arc::new(TimeSync::new(true));

        // Test synchronization
        let result = time_sync.synchronize().await;
        assert!(result.is_ok(), "Time sync failed: {:?}", result);

        // Test time creation
        let timestamp = time_sync.create_timestamp();
        assert!(timestamp > 0.0);

        // Test time conversion
        let system_time = time_sync.lsl_time();
        let converted = time_sync.system_to_lsl_time(system_time);
        assert!((system_time - converted).abs() < 0.001); // Should be very close
    }

    #[tokio::test]
    async fn test_stream_types() {
        // Test TTL stream info
        let ttl_info = StreamInfo::ttl_markers("test_device");
        assert_eq!(ttl_info.stream_type, StreamType::Markers);
        assert_eq!(ttl_info.channel_count, 1);
        assert_eq!(ttl_info.channel_format, ChannelFormat::String);

        // Test Kernel fNIRS stream info
        let kernel_info = StreamInfo::kernel_fnirs("kernel_device", 16);
        assert_eq!(kernel_info.stream_type, StreamType::FNIRS);
        assert_eq!(kernel_info.channel_count, 16);
        assert_eq!(kernel_info.channel_format, ChannelFormat::Float32);

        // Test Pupil gaze stream info
        let pupil_info = StreamInfo::pupil_gaze("pupil_device");
        assert_eq!(pupil_info.stream_type, StreamType::Gaze);
        assert_eq!(pupil_info.channel_count, 3);
        assert_eq!(pupil_info.channel_format, ChannelFormat::Float32);

        // Test Biopac biosignals stream info
        let biopac_info = StreamInfo::biopac_biosignals("biopac_device", 8, 1000.0);
        assert_eq!(biopac_info.stream_type, StreamType::Biosignals);
        assert_eq!(biopac_info.channel_count, 8);
        assert_eq!(biopac_info.nominal_srate, 1000.0);
    }

    #[tokio::test]
    async fn test_sample_data_conversion() {
        // Test TTL marker
        let marker_data = SampleData::ttl_marker("TEST_PULSE".to_string());
        assert_eq!(marker_data.channel_count(), 1);

        let bytes = marker_data.to_bytes();
        assert!(!bytes.is_empty());

        // Test float32 data
        let float_data = SampleData::float32(vec![1.0, 2.0, 3.0]);
        assert_eq!(float_data.channel_count(), 3);

        let bytes = float_data.to_bytes();
        assert_eq!(bytes.len(), 12); // 3 floats * 4 bytes each
    }

    #[tokio::test]
    async fn test_comprehensive_stats() {
        let device = LslDevice::new("stats_test".to_string(), None);

        let stats = device.get_comprehensive_stats().await;

        // Verify basic structure
        assert!(stats.get("device_id").is_some());
        assert!(stats.get("status").is_some());
        assert!(stats.get("config").is_some());
        assert!(stats.get("outlets").is_some());
        assert!(stats.get("inlets").is_some());
        assert!(stats.get("time_sync").is_some());

        // Verify config values are present
        let config = stats.get("config").unwrap();
        assert!(config.get("auto_create_outlets").is_some());
        assert!(config.get("enable_time_sync").is_some());
        assert!(config.get("buffer_size").is_some());
    }

    #[test]
    fn test_stream_filter_creation() {
        // Test default filter
        let filter = StreamFilter::default();
        assert!(filter.name_pattern.is_none());
        assert!(filter.stream_type.is_none());

        // Test specific filter
        let filter = StreamFilter {
            stream_type: Some(StreamType::FNIRS),
            min_channels: Some(8),
            max_channels: Some(32),
            ..Default::default()
        };

        assert_eq!(filter.stream_type, Some(StreamType::FNIRS));
        assert_eq!(filter.min_channels, Some(8));
        assert_eq!(filter.max_channels, Some(32));
    }

    #[test]
    fn test_lsl_config_defaults() {
        let config = LslConfig::default();

        assert!(config.auto_create_outlets);
        assert!(!config.auto_discover_inlets);
        assert_eq!(config.max_inlets, 10);
        assert_eq!(config.buffer_size, 1000);
        assert!(config.enable_time_sync);
        assert!(config.stream_filters.is_empty());
        assert!(config.outlet_metadata.is_empty());
    }

    #[tokio::test]
    async fn test_error_conditions() {
        let mut device = LslDevice::new("error_test".to_string(), None);

        // Test heartbeat when disconnected
        let result = device.heartbeat().await;
        assert!(result.is_err());

        // Test sending empty data
        let result = device.send(&[]).await;
        assert!(result.is_err());

        // Test sending invalid device type
        let result = device.send(&[99, 1, 2, 3]).await;
        assert!(result.is_err());

        // Connect first for further tests
        device.connect().await.unwrap();

        // Test sending invalid TTL data (non-UTF8)
        let invalid_ttl = &[0, 0xFF, 0xFF];
        let result = device.send(invalid_ttl).await;
        assert!(result.is_err());

        // Test sending invalid float data (not multiple of 4 bytes)
        let invalid_float = &[1, 1, 2, 3]; // 3 bytes, not multiple of 4
        let result = device.send(invalid_float).await;
        assert!(result.is_err());
    }
}