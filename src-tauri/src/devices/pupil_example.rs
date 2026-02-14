// Example usage of the Pupil Labs Neon device module (REST API)
//
// This file demonstrates how to use the PupilDevice to control a Neon
// Companion App via its REST API. Gaze data streaming is handled separately
// by the LSL neon.rs module — this module is for control operations only.
//
// Prerequisites:
//   - Neon Companion App running on the phone
//   - Phone and computer on the same network
//   - For gaze data: "Stream over LSL" enabled in Companion App settings

use crate::devices::pupil::PupilDevice;
use crate::devices::{Device, DeviceConfig, DeviceError};
use tokio::time::{sleep, Duration};
use tracing::{error, info};

/// Example: Basic connection and status check
pub async fn basic_connection_example() -> Result<(), DeviceError> {
    info!("Starting Neon basic connection example");

    // 1. Create device — defaults to port 8080 if not specified
    let mut device = PupilDevice::new("neon.local:8080".to_string());

    // 2. Configure with custom timeout
    let mut config = DeviceConfig::default();
    config.timeout_ms = 10000;
    config.auto_reconnect = true;
    config.reconnect_interval_ms = 2000;
    device.configure(config)?;

    // 3. Connect — verifies Companion App reachability via GET /api/status
    device.connect().await?;
    info!("Connected to Neon Companion");

    // 4. Read cached status (populated during connect)
    if let Some(status) = device.get_cached_status() {
        info!(
            "Device: {} ({})",
            status.phone.device_name, status.phone.device_id
        );
        info!(
            "Battery: {:.0}% ({})",
            status.phone.battery_level * 100.0,
            status.phone.battery_state
        );
        info!("Sensors: {} connected", status.sensors.len());
    }

    // 5. Disconnect
    device.disconnect().await?;
    info!("Basic connection example completed");
    Ok(())
}

/// Example: Recording lifecycle (start → events → stop)
pub async fn recording_example() -> Result<(), DeviceError> {
    info!("Starting Neon recording example");

    let mut device = PupilDevice::new("neon.local:8080".to_string());
    device.connect().await?;

    // Start recording — returns UUID
    let recording_id = device.start_recording().await?;
    info!("Recording started: {}", recording_id);

    // Send experiment events with nanosecond timestamps
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;

    device
        .send_neon_event("experiment_start", Some(now_ns))
        .await?;

    // Simulate experiment with timed events
    sleep(Duration::from_secs(2)).await;
    device.send_neon_event("stimulus_onset", None).await?;

    sleep(Duration::from_secs(1)).await;
    device
        .send_neon_event("participant_response", None)
        .await?;

    // Stop recording (saves to Neon Cloud)
    device.stop_recording().await?;
    info!("Recording stopped and saved");

    device.disconnect().await?;
    Ok(())
}

/// Example: Using the send() command routing via JSON
///
/// This shows how the bridge WebSocket handler routes commands —
/// the same interface used by HyperStudy web app.
pub async fn bridge_command_example() -> Result<(), DeviceError> {
    info!("Starting bridge command routing example");

    let mut device = PupilDevice::new("neon.local:8080".to_string());
    device.connect().await?;

    // These JSON commands are what arrive via WebSocket from HyperStudy:

    // Start recording
    device
        .send(br#"{"command": "recording_start"}"#)
        .await?;

    // Send an event with timestamp
    device
        .send(br#"{"command": "event", "name": "trial_1_start", "timestamp": 1700000000000000000}"#)
        .await?;

    // Query status
    device.send(br#"{"command": "status"}"#).await?;

    // Stop recording
    device
        .send(br#"{"command": "recording_stop"}"#)
        .await?;

    device.disconnect().await?;
    info!("Bridge command routing example completed");
    Ok(())
}

/// Example: Integration with Neon LSL manager for gaze data
///
/// The PupilDevice handles control (REST API), while NeonLslManager
/// handles data streaming (LSL). This example shows the full workflow.
pub async fn full_integration_example() -> Result<(), DeviceError> {
    info!("Starting full Neon integration example");
    info!("NOTE: Gaze data streaming requires LSL to be enabled in Neon Companion App");
    info!("      Use NeonLslManager (neon.rs) for gaze data — not PupilDevice");

    // Step 1: Connect control channel via REST API
    let mut device = PupilDevice::new("neon.local:8080".to_string());
    device.connect().await?;

    // Step 2: Start recording via REST
    let recording_id = device.start_recording().await?;
    info!("Recording {}: ready for gaze data collection", recording_id);

    // Step 3: Gaze data flows via LSL (handled by bridge/websocket.rs):
    //   - DiscoverNeon → finds "{Name}_Neon Gaze" LSL stream
    //   - ConnectNeonGaze → InletManager receives Float32 samples at 200Hz
    //   - Samples forwarded via WebSocket to HyperStudy

    // Step 4: Send event markers during experiment
    for trial in 1..=3 {
        let event_name = format!("trial_{}_start", trial);
        device.send_neon_event(&event_name, None).await?;
        sleep(Duration::from_secs(1)).await;
    }

    // Step 5: Stop recording
    device.stop_recording().await?;
    device.disconnect().await?;

    info!("Full integration example completed");
    Ok(())
}

/// Example: Error handling and connection testing
pub async fn error_handling_example() -> Result<(), DeviceError> {
    info!("Starting error handling example");

    let mut device = PupilDevice::new("neon.local:8080".to_string());

    // Test connection without fully connecting
    match device.test_connection().await {
        Ok(true) => info!("Neon Companion is reachable"),
        Ok(false) => {
            error!("Neon Companion is not reachable at neon.local:8080");
            return Ok(());
        }
        Err(e) => {
            error!("Connection test error: {}", e);
            return Err(e);
        }
    }

    // Connect with retry logic
    let mut config = DeviceConfig::default();
    config.auto_reconnect = true;
    config.reconnect_interval_ms = 1000;
    config.timeout_ms = 5000;
    config.custom_settings = serde_json::json!({ "max_retries": 5 });
    device.configure(config)?;

    device.connect().await?;

    // Periodic heartbeat to detect disconnection
    for i in 0..5 {
        match device.heartbeat().await {
            Ok(()) => info!("Heartbeat {} OK", i + 1),
            Err(e) => {
                error!("Heartbeat {} failed: {} — device may have disconnected", i + 1, e);
                break;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }

    device.disconnect().await?;
    Ok(())
}
