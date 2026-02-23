//! EyeLink 1000 Plus eye tracker integration.
//!
//! Provides Rust FFI bindings to the SR Research `eyelink_core` C library
//! for connection management, recording, event markers, calibration, and
//! real-time gaze data streaming.
//!
//! ## Architecture
//!
//! The EyeLink integration bypasses the generic `Device` trait because:
//! - Calibration requires broadcast channels (HOOKFCNS callbacks)
//! - Gaze streaming uses a dedicated polling thread
//! - The API is fundamentally different from TCP/WebSocket devices
//!
//! Instead, `EyeLinkManager` lives directly on `AppState` (like `NeonLslManager`).
//!
//! ## Feature Gate
//!
//! This module is gated behind the `eyelink` Cargo feature so the Bridge
//! builds without the EyeLink Developers Kit when not needed.

pub mod calibration;
pub mod ffi;
pub mod gaze_stream;

use serde::{Deserialize, Serialize};
use std::os::raw::c_short;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{info, warn};

use calibration::CalibrationEvent;
use gaze_stream::GazeSample;

// ============================================================================
// EyeLink Manager
// ============================================================================

/// Manages the EyeLink tracker connection, recording, and gaze streaming.
///
/// This is the primary entry point for all EyeLink operations. It is stored
/// in `AppState` and shared across WebSocket handler tasks.
#[derive(Debug)]
pub struct EyeLinkManager {
    state: Arc<RwLock<EyeLinkState>>,
}

/// Internal mutable state for the EyeLink manager.
#[derive(Debug)]
struct EyeLinkState {
    connected: bool,
    recording: bool,
    tracker_ip: String,
    tracker_version: Option<String>,
    sample_rate: u32,
    display_width: u32,
    display_height: u32,
    /// Handle to the gaze streaming task (if active)
    gaze_task: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal for gaze streaming
    gaze_shutdown: Option<mpsc::Sender<()>>,
    /// Handle to the calibration task (if active)
    calibration_task: Option<tokio::task::JoinHandle<()>>,
}

/// Status information returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EyeLinkStatus {
    pub connected: bool,
    pub recording: bool,
    pub tracker_ip: String,
    pub tracker_version: Option<String>,
    pub sample_rate: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub gaze_streaming: bool,
    pub calibrating: bool,
}

impl EyeLinkManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(EyeLinkState {
                connected: false,
                recording: false,
                tracker_ip: "100.1.1.1".to_string(),
                tracker_version: None,
                sample_rate: 500,
                display_width: 1920,
                display_height: 1080,
                gaze_task: None,
                gaze_shutdown: None,
                calibration_task: None,
            })),
        }
    }

    /// Connect to the EyeLink tracker.
    ///
    /// Sets the tracker IP address and opens the TCP/IP connection via
    /// the eyelink_core C library. The write lock is only held briefly
    /// for state checks and updates — blocking FFI calls run lock-free.
    pub async fn connect(
        &self,
        ip: &str,
        sample_rate: Option<u32>,
        display_width: Option<u32>,
        display_height: Option<u32>,
    ) -> Result<EyeLinkStatus, String> {
        // Phase 1: Check state under a brief write lock
        {
            let state = self.state.read().await;
            if state.connected {
                return Err("Already connected to EyeLink".to_string());
            }
        }

        info!(device = "eyelink", "Connecting to EyeLink at {}", ip);

        // Phase 2: Blocking FFI calls (no lock held)
        let ip_owned = ip.to_string();
        let connect_result = tokio::task::spawn_blocking(move || {
            ffi::set_address(&ip_owned);
            ffi::connect(ffi::CONNECT_NORMAL)
        })
        .await
        .map_err(|e| format!("Connect task panicked: {}", e))?;

        if let Err(code) = connect_result {
            return Err(format!("EyeLink connection failed with code: {}", code));
        }

        let (version_num, version_str) = tokio::task::spawn_blocking(|| ffi::get_tracker_version())
            .await
            .map_err(|e| format!("Version query panicked: {}", e))?;

        info!(
            device = "eyelink",
            "Connected to EyeLink v{} ({})", version_num, version_str
        );

        // Phase 3: Update state and extract config for tracker configuration
        let (rate, w, h) = {
            let mut state = self.state.write().await;
            state.connected = true;
            state.tracker_ip = ip.to_string();
            state.tracker_version = Some(version_str);

            if let Some(r) = sample_rate {
                state.sample_rate = r;
            }
            if let Some(width) = display_width {
                state.display_width = width;
            }
            if let Some(height) = display_height {
                state.display_height = height;
            }

            (state.sample_rate, state.display_width, state.display_height)
        }; // Write lock dropped here

        // Phase 4: Configure tracker via blocking FFI (no lock held)
        tokio::task::spawn_blocking(move || {
            let _ = ffi::send_command(&format!("sample_rate {}", rate));
            let _ = ffi::send_command(&format!("screen_pixel_coords = 0 0 {} {}", w - 1, h - 1));
            let _ = ffi::send_command("link_sample_data = LEFT,RIGHT,GAZE,AREA,STATUS");
            let _ = ffi::send_command("file_sample_data = LEFT,RIGHT,GAZE,AREA,STATUS");
        })
        .await
        .map_err(|e| format!("Configuration panicked: {}", e))?;

        // Phase 5: Return status
        let state = self.state.read().await;
        Ok(self.build_status(&state))
    }

    /// Disconnect from the EyeLink tracker.
    ///
    /// Extracts task handles and state flags under a brief write lock, then
    /// performs the slow FFI calls without holding any lock. Re-acquires
    /// the lock only to update final state.
    pub async fn disconnect(&self) -> Result<(), String> {
        // Phase 1: Extract handles and check state under a brief write lock
        let (gaze_shutdown, gaze_task, was_recording) = {
            let mut state = self.state.write().await;
            if !state.connected {
                return Err("Not connected to EyeLink".to_string());
            }
            let shutdown = state.gaze_shutdown.take();
            let task = state.gaze_task.take();
            let recording = state.recording;
            (shutdown, task, recording)
        }; // Write lock dropped here

        // Phase 2: Shutdown gaze streaming (no lock held)
        if let Some(shutdown_tx) = gaze_shutdown {
            let _ = shutdown_tx.send(()).await;
        }
        if let Some(task) = gaze_task {
            task.abort();
        }

        // Phase 3: Stop recording via blocking FFI (no lock held)
        if was_recording {
            tokio::task::spawn_blocking(|| ffi::end_recording())
                .await
                .map_err(|e| format!("Stop recording panicked: {}", e))?;
        }

        // Phase 4: Disconnect via blocking FFI (no lock held)
        tokio::task::spawn_blocking(|| ffi::disconnect())
            .await
            .map_err(|e| format!("Disconnect panicked: {}", e))?;

        // Phase 5: Update final state under a brief write lock
        {
            let mut state = self.state.write().await;
            state.connected = false;
            state.recording = false;
            state.tracker_version = None;
        }

        info!(device = "eyelink", "Disconnected from EyeLink");
        Ok(())
    }

    /// Start recording to the EDF file.
    ///
    /// Enables file + link samples and events so we get both EDF recording
    /// and real-time link data for gaze streaming.
    pub async fn start_recording(&self) -> Result<(), String> {
        {
            let state = self.state.read().await;
            if !state.connected {
                return Err("Not connected to EyeLink".to_string());
            }
            if state.recording {
                return Err("Already recording".to_string());
            }
        } // Drop lock before blocking FFI

        tokio::task::spawn_blocking(|| ffi::begin_recording(true, true, true, true))
            .await
            .map_err(|e| format!("Start recording panicked: {}", e))?
            .map_err(|code| format!("start_recording failed with code: {}", code))?;

        let mut state = self.state.write().await;
        state.recording = true;
        info!(device = "eyelink", "Recording started");
        Ok(())
    }

    /// Stop recording.
    pub async fn stop_recording(&self) -> Result<(), String> {
        {
            let state = self.state.read().await;
            if !state.recording {
                return Err("Not recording".to_string());
            }
        } // Drop lock before blocking FFI

        tokio::task::spawn_blocking(|| ffi::end_recording())
            .await
            .map_err(|e| format!("Stop recording panicked: {}", e))?;

        let mut state = self.state.write().await;
        state.recording = false;
        info!(device = "eyelink", "Recording stopped");
        Ok(())
    }

    /// Send a message to the EDF file (event marker).
    ///
    /// This is the primary mechanism for timestamped markers in the EDF recording.
    /// Messages appear in the EDF file with the tracker's timestamp.
    pub async fn send_message(&self, msg: String) -> Result<(), String> {
        {
            let state = self.state.read().await;
            if !state.connected {
                return Err("Not connected to EyeLink".to_string());
            }
        } // Drop read lock before blocking call

        tokio::task::spawn_blocking(move || ffi::send_message(&msg))
            .await
            .map_err(|e| format!("Send message panicked: {}", e))?
            .map_err(|code| format!("eyemsg_printf failed with code: {}", code))?;

        Ok(())
    }

    /// Send a command to the tracker.
    pub async fn send_command(&self, cmd: String) -> Result<(), String> {
        {
            let state = self.state.read().await;
            if !state.connected {
                return Err("Not connected to EyeLink".to_string());
            }
        } // Drop read lock before blocking call

        tokio::task::spawn_blocking(move || ffi::send_command(&cmd))
            .await
            .map_err(|e| format!("Send command panicked: {}", e))?
            .map_err(|code| format!("eyecmd_printf failed with code: {}", code))?;

        Ok(())
    }

    /// Start the EyeLink calibration/validation loop.
    ///
    /// Returns a broadcast::Receiver for calibration events that the WebSocket
    /// handler can forward to the frontend. The calibration runs on a blocking
    /// thread since `do_tracker_setup()` doesn't return until complete.
    ///
    /// Also returns a JoinHandle that resolves when calibration finishes, allowing
    /// the WebSocket handler to send the completion result to the frontend.
    ///
    /// The calibration task automatically clears itself from state on completion.
    pub async fn start_calibration(
        &self,
    ) -> Result<
        (
            broadcast::Receiver<CalibrationEvent>,
            tokio::task::JoinHandle<Result<c_short, String>>,
        ),
        String,
    > {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err("Not connected to EyeLink".to_string());
        }

        // Guard against concurrent calibration — check if the stored task is still running
        if let Some(ref task) = state.calibration_task {
            if !task.is_finished() {
                return Err("Calibration already in progress".to_string());
            }
            // Previous calibration finished but handle wasn't cleaned up (shouldn't happen
            // normally since we auto-clear, but be defensive)
            state.calibration_task = None;
        }

        // Set up calibration hooks and get the broadcast sender
        let tx = calibration::setup_calibration_hooks();
        let rx = tx.subscribe();

        // Run calibration and auto-clear state on completion
        let state_clone = self.state.clone();
        let cal_handle = tokio::spawn(async move {
            let result = calibration::run_calibration().await;
            // Clear calibration_task when done so `calibrating` status resets
            let mut s = state_clone.write().await;
            s.calibration_task = None;
            result
        });

        // Store a second subscription handle to track calibration lifecycle.
        // We need cal_handle for the WebSocket caller, but also need a handle
        // on state for build_status() and concurrent-start guard. We spawn a
        // lightweight watcher that completes when calibration does.
        let tx_watcher = tx.clone();
        state.calibration_task = Some(tokio::spawn(async move {
            // Wait until the broadcast channel closes (all senders dropped),
            // which happens when run_calibration's CalibrationCleanupGuard drops
            let mut rx_watcher = tx_watcher.subscribe();
            while rx_watcher.recv().await.is_ok() {}
        }));

        Ok((rx, cal_handle))
    }

    /// Start streaming gaze data.
    ///
    /// Spawns a polling thread that reads `get_newest_sample()` and sends
    /// gaze data to the returned mpsc receiver. The WebSocket handler can
    /// then forward this data to the frontend.
    pub async fn start_gaze_stream(&self) -> Result<mpsc::Receiver<GazeSample>, String> {
        let mut state = self.state.write().await;

        if !state.connected {
            return Err("Not connected to EyeLink".to_string());
        }

        if state.gaze_task.is_some() {
            return Err("Gaze stream already active".to_string());
        }

        let sample_rate = state.sample_rate;
        let display_width = state.display_width;
        let display_height = state.display_height;

        let (gaze_tx, gaze_rx) = mpsc::channel(256);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let task = gaze_stream::spawn_gaze_polling(
            gaze_tx,
            shutdown_rx,
            sample_rate,
            display_width,
            display_height,
        );

        state.gaze_task = Some(task);
        state.gaze_shutdown = Some(shutdown_tx);

        info!(device = "eyelink", "Gaze stream started");
        Ok(gaze_rx)
    }

    /// Stop streaming gaze data.
    pub async fn stop_gaze_stream(&self) -> Result<(), String> {
        // Extract handles under a brief write lock
        let (shutdown_tx, task) = {
            let mut state = self.state.write().await;
            (state.gaze_shutdown.take(), state.gaze_task.take())
        }; // Write lock dropped here

        // Shutdown and wait without holding any lock
        if let Some(tx) = shutdown_tx {
            let _ = tx.send(()).await;
        }
        if let Some(task) = task {
            // Give the polling thread time to exit gracefully before aborting
            match tokio::time::timeout(Duration::from_millis(500), task).await {
                Ok(_) => {}
                Err(_) => {
                    warn!(
                        device = "eyelink",
                        "Gaze polling thread did not exit in time"
                    );
                }
            }
        }

        info!(device = "eyelink", "Gaze stream stopped");
        Ok(())
    }

    /// Get current status.
    pub async fn get_status(&self) -> EyeLinkStatus {
        let state = self.state.read().await;
        self.build_status(&state)
    }

    /// Check if connected (non-async, uses FFI).
    pub async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        state.connected
    }

    fn build_status(&self, state: &EyeLinkState) -> EyeLinkStatus {
        EyeLinkStatus {
            connected: state.connected,
            recording: state.recording,
            tracker_ip: state.tracker_ip.clone(),
            tracker_version: state.tracker_version.clone(),
            sample_rate: state.sample_rate,
            display_width: state.display_width,
            display_height: state.display_height,
            gaze_streaming: state.gaze_task.is_some(),
            calibrating: state
                .calibration_task
                .as_ref()
                .map_or(false, |t| !t.is_finished()),
        }
    }
}

impl Default for EyeLinkManager {
    fn default() -> Self {
        Self::new()
    }
}
