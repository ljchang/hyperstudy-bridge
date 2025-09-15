// Example usage of the Pupil Labs Neon Eye Tracker device module
// This file demonstrates how to integrate and use the enhanced Pupil device
// implementation in the HyperStudy Bridge application.

use crate::devices::{Device, DeviceConfig, DeviceError};
use crate::devices::pupil::{PupilDevice, EventAnnotation, StreamingConfig};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{info, error};

/// Example function demonstrating basic Pupil device usage
pub async fn basic_pupil_usage_example() -> Result<(), DeviceError> {
    info!("Starting basic Pupil Labs device example");

    // 1. Create a new device instance
    let mut device = PupilDevice::new("192.168.1.100".to_string());

    // 2. Configure the device with custom settings
    let mut config = DeviceConfig::default();
    config.timeout_ms = 10000;  // 10 second timeout
    config.auto_reconnect = true;
    config.reconnect_interval_ms = 2000;
    config.custom_settings = serde_json::json!({
        "streaming_config": {
            "gaze": true,
            "pupil": false,
            "video": false,
            "imu": false
        },
        "max_retries": 3
    });

    device.configure(config)?;

    // 3. Connect to the device
    match device.connect().await {
        Ok(()) => {
            info!("Successfully connected to Pupil Labs device");

            // 4. Request device information
            device.request_device_info().await?;

            // 5. Start gaze data streaming
            device.start_gaze_streaming().await?;

            // 6. Send some event annotations
            let event = EventAnnotation {
                timestamp: 1234567890.0,
                label: "experiment_start".to_string(),
                duration: None,
                extra_data: Some(HashMap::from([
                    ("participant_id".to_string(), serde_json::Value::String("P001".to_string())),
                    ("condition".to_string(), serde_json::Value::String("control".to_string())),
                ])),
            };
            device.send_event(event).await?;

            // 7. Simulate receiving data for a few seconds
            for i in 0..5 {
                match device.receive().await {
                    Ok(data) => {
                        if !data.is_empty() {
                            info!("Received {} bytes from device", data.len());

                            // Check for latest gaze data
                            if let Some(gaze_data) = device.get_latest_gaze_data() {
                                info!("Latest gaze position: {:?}, confidence: {}",
                                      gaze_data.gaze_position_2d, gaze_data.confidence);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving data: {}", e);
                    }
                }

                sleep(Duration::from_millis(500)).await;
            }

            // 8. Stop streaming and disconnect
            device.stop_gaze_streaming().await?;
            device.disconnect().await?;

            info!("Basic Pupil device example completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Failed to connect to Pupil Labs device: {}", e);
            Err(e)
        }
    }
}

/// Example function demonstrating recording functionality
pub async fn recording_example() -> Result<(), DeviceError> {
    info!("Starting Pupil Labs recording example");

    let mut device = PupilDevice::new("192.168.1.100".to_string());

    // Connect to device
    device.connect().await?;

    // Start recording with a template
    device.start_recording(Some("experiment_template".to_string())).await?;
    info!("Recording started");

    // Send experiment events
    let events = vec![
        EventAnnotation {
            timestamp: 1234567890.0,
            label: "stimulus_onset".to_string(),
            duration: Some(2.0),
            extra_data: Some(HashMap::from([
                ("stimulus_type".to_string(), serde_json::Value::String("visual".to_string())),
                ("stimulus_id".to_string(), serde_json::Value::Number(serde_json::Number::from(123))),
            ])),
        },
        EventAnnotation {
            timestamp: 1234567892.0,
            label: "participant_response".to_string(),
            duration: None,
            extra_data: Some(HashMap::from([
                ("response_time_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(1250))),
                ("accuracy".to_string(), serde_json::Value::Bool(true)),
            ])),
        },
    ];

    for event in events {
        device.send_event(event).await?;
        sleep(Duration::from_millis(100)).await;
    }

    // Stop recording
    device.stop_recording().await?;
    info!("Recording stopped");

    // Disconnect
    device.disconnect().await?;

    Ok(())
}

/// Example function demonstrating device discovery
pub async fn discovery_example() -> Result<(), DeviceError> {
    info!("Starting device discovery example");

    // Discover available devices
    let devices = PupilDevice::discover_devices().await?;
    info!("Found {} potential Pupil Labs devices", devices.len());

    for device_ip in devices {
        info!("Testing connection to device at: {}", device_ip);

        let mut device = PupilDevice::new(device_ip.clone());

        // Set shorter timeout for discovery
        let mut config = DeviceConfig::default();
        config.timeout_ms = 3000;  // 3 second timeout for discovery
        config.auto_reconnect = false;
        device.configure(config)?;

        // Try to connect
        match device.connect().await {
            Ok(()) => {
                info!("Successfully connected to device at {}", device_ip);

                // Get device information
                device.request_device_info().await?;
                sleep(Duration::from_millis(500)).await;

                if let Some(device_info) = device.get_device_info() {
                    info!("Device info: {} ({})", device_info.device_name, device_info.serial_number);
                }

                device.disconnect().await?;
                break;  // Found a working device
            }
            Err(e) => {
                info!("Could not connect to device at {}: {}", device_ip, e);
            }
        }
    }

    Ok(())
}

/// Example function demonstrating advanced streaming configuration
pub async fn advanced_streaming_example() -> Result<(), DeviceError> {
    info!("Starting advanced streaming configuration example");

    let mut device = PupilDevice::new("192.168.1.100".to_string());

    // Configure for multiple data streams
    let mut config = DeviceConfig::default();
    config.custom_settings = serde_json::json!({
        "streaming_config": {
            "gaze": true,
            "pupil": true,
            "video": false,
            "imu": true,
            "frame_rate": 120.0
        }
    });

    device.configure(config)?;
    device.connect().await?;

    // Start multiple streams
    device.start_gaze_streaming().await?;

    // Monitor data streams
    let mut gaze_count = 0;
    let mut pupil_count = 0;

    for _ in 0..20 {  // Monitor for 10 seconds
        match device.receive().await {
            Ok(data) => {
                if !data.is_empty() {
                    // Check for new gaze data
                    if device.get_latest_gaze_data().is_some() {
                        gaze_count += 1;
                    }

                    // Check for new pupil data
                    if device.get_latest_pupil_data().is_some() {
                        pupil_count += 1;
                    }
                }
            }
            Err(e) => {
                error!("Receive error: {}", e);
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    info!("Received {} gaze samples and {} pupil samples", gaze_count, pupil_count);

    // Cleanup
    device.stop_gaze_streaming().await?;
    device.disconnect().await?;

    Ok(())
}

/// Example function demonstrating error handling and reconnection
pub async fn error_handling_example() -> Result<(), DeviceError> {
    info!("Starting error handling and reconnection example");

    let mut device = PupilDevice::new("192.168.1.100".to_string());

    // Configure with auto-reconnect
    let mut config = DeviceConfig::default();
    config.auto_reconnect = true;
    config.reconnect_interval_ms = 1000;
    config.timeout_ms = 5000;
    config.custom_settings = serde_json::json!({
        "max_retries": 5
    });

    device.configure(config)?;

    // Connect with retry logic
    let mut connection_attempts = 0;
    loop {
        connection_attempts += 1;

        match device.connect().await {
            Ok(()) => {
                info!("Connected successfully on attempt {}", connection_attempts);
                break;
            }
            Err(e) => {
                error!("Connection attempt {} failed: {}", connection_attempts, e);

                if connection_attempts >= 3 {
                    return Err(DeviceError::ConnectionFailed("Max connection attempts exceeded".to_string()));
                }

                sleep(Duration::from_secs(2)).await;
            }
        }
    }

    // Test heartbeat functionality
    for i in 0..5 {
        match device.heartbeat().await {
            Ok(()) => {
                info!("Heartbeat {} successful", i + 1);
            }
            Err(e) => {
                error!("Heartbeat {} failed: {}", i + 1, e);
            }
        }

        sleep(Duration::from_secs(1)).await;
    }

    device.disconnect().await?;
    Ok(())
}