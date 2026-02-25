//! EyeLink calibration callback system.
//!
//! The EyeLink C API uses a callback-driven calibration flow via the HOOKFCNS struct.
//! When `do_tracker_setup()` is called, `eyelink_core` calls back into function pointers
//! to instruct the display to show/hide calibration targets.
//!
//! Since C function pointers can't capture Rust closures, we use a global static
//! `broadcast::Sender` to dispatch calibration events from C callbacks to the
//! WebSocket layer, which forwards them to the frontend for rendering.

use super::ffi;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::os::raw::{c_char, c_short, c_void};
use std::sync::{Mutex, OnceLock};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

// ============================================================================
// Calibration Event Types
// ============================================================================

/// Events emitted during EyeLink calibration, forwarded to frontend via WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CalibrationEvent {
    /// Calibration display should be set up (fullscreen overlay)
    SetupDisplay,
    /// A calibration target should be drawn at the given pixel coordinates
    DrawTarget { x: i16, y: i16 },
    /// The current calibration target should be erased
    EraseTarget,
    /// The entire calibration display should be cleared
    ClearDisplay,
    /// Calibration display should be closed
    ExitDisplay,
    /// Play a beep sound (type indicates success/error/target)
    PlayBeep { beep_type: i32 },
    /// Alert message from the tracker
    Alert { message: String },
}

// ============================================================================
// Global State for C Callbacks
// ============================================================================

/// Global sender for calibration events.
/// C function pointers can't capture state, so we use a global Mutex-wrapped sender.
/// This is set before calling do_tracker_setup() and cleared after.
static CALIBRATION_SENDER: Mutex<Option<broadcast::Sender<CalibrationEvent>>> = Mutex::new(None);

/// Global key input queue.
/// The frontend sends key presses (accept/cancel) which we queue here.
/// The get_input_key callback reads from this queue.
/// Uses VecDeque for O(1) pop_front instead of Vec::remove(0) which is O(n).
static KEY_INPUT_QUEUE: Mutex<VecDeque<c_short>> = Mutex::new(VecDeque::new());

// ============================================================================
// C Callback Functions (registered via HOOKFCNS)
// ============================================================================

// Return type is INT16 (c_short) for setup_cal_display_hook per the C typedef.
unsafe extern "C" fn setup_cal_display_hook() -> c_short {
    debug!(device = "eyelink", "Calibration: setup_cal_display");
    send_calibration_event(CalibrationEvent::SetupDisplay);
    0
}

// void return per C typedef
unsafe extern "C" fn exit_cal_display_hook() {
    debug!(device = "eyelink", "Calibration: exit_cal_display");
    send_calibration_event(CalibrationEvent::ExitDisplay);
}

unsafe extern "C" fn record_abort_hide_hook() {
    // No-op — we don't need to hide anything for recording abort
}

unsafe extern "C" fn setup_image_display_hook(_width: c_short, _height: c_short) -> c_short {
    // Camera image display — not needed for web UI calibration
    0
}

unsafe extern "C" fn image_title_hook(_threshold: c_short, _title: *mut c_char) {
    // Camera image title — not used
}

unsafe extern "C" fn draw_image_line_hook(
    _width: c_short,
    _line: *mut c_char,
    _totlines: c_short,
    _image_line: c_short,
) {
    // Camera image rendering — not used (we skip camera display in web UI)
}

unsafe extern "C" fn set_image_palette_hook(
    _ncolors: c_short,
    _r: *mut u8,
    _g: *mut u8,
    _b: *mut u8,
) {
    // Camera image palette — not used
}

unsafe extern "C" fn exit_image_display_hook() {
    // Camera image display cleanup — not used
}

unsafe extern "C" fn clear_cal_display_hook() {
    debug!(device = "eyelink", "Calibration: clear_cal_display");
    send_calibration_event(CalibrationEvent::ClearDisplay);
}

unsafe extern "C" fn erase_cal_target_hook() {
    debug!(device = "eyelink", "Calibration: erase_cal_target");
    send_calibration_event(CalibrationEvent::EraseTarget);
}

unsafe extern "C" fn draw_cal_target_hook(x: c_short, y: c_short) {
    debug!(
        device = "eyelink",
        "Calibration: draw_cal_target at ({}, {})", x, y
    );
    send_calibration_event(CalibrationEvent::DrawTarget { x, y });
}

/// Beep when calibration target is presented
unsafe extern "C" fn cal_target_beep_hook() {
    debug!(device = "eyelink", "Calibration: cal_target_beep");
    send_calibration_event(CalibrationEvent::PlayBeep {
        beep_type: ffi::CAL_TARG_BEEP,
    });
}

/// Beep when calibration point is done (error=0 means good)
unsafe extern "C" fn cal_done_beep_hook(error: c_short) {
    debug!(
        device = "eyelink",
        "Calibration: cal_done_beep error={}", error
    );
    send_calibration_event(CalibrationEvent::PlayBeep {
        beep_type: if error == 0 {
            ffi::CAL_GOOD_BEEP
        } else {
            ffi::CAL_ERR_BEEP
        },
    });
}

/// Beep when drift-correct point is done
unsafe extern "C" fn dc_done_beep_hook(error: c_short) {
    debug!(
        device = "eyelink",
        "Calibration: dc_done_beep error={}", error
    );
    send_calibration_event(CalibrationEvent::PlayBeep {
        beep_type: if error == 0 {
            ffi::DC_GOOD_BEEP
        } else {
            ffi::DC_ERR_BEEP
        },
    });
}

/// Beep when drift-correct target is presented
unsafe extern "C" fn dc_target_beep_hook() {
    debug!(device = "eyelink", "Calibration: dc_target_beep");
    send_calibration_event(CalibrationEvent::PlayBeep {
        beep_type: ffi::DC_TARG_BEEP,
    });
}

unsafe extern "C" fn get_input_key_hook(_event: *mut c_void) -> c_short {
    // Check the key input queue for any pending key presses from the frontend
    if let Ok(mut queue) = KEY_INPUT_QUEUE.lock() {
        if let Some(key) = queue.pop_front() {
            return key;
        }
    }
    ffi::JUNK_KEY
}

unsafe extern "C" fn alert_printf_hook(msg: *const c_char) {
    let message = if msg.is_null() {
        "Unknown alert".to_string()
    } else {
        std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    warn!(device = "eyelink", "Tracker alert: {}", message);
    send_calibration_event(CalibrationEvent::Alert { message });
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Send a calibration event to the broadcast channel (if registered).
fn send_calibration_event(event: CalibrationEvent) {
    if let Ok(guard) = CALIBRATION_SENDER.lock() {
        if let Some(ref tx) = *guard {
            let _ = tx.send(event);
        }
    }
}

/// Enqueue a key press to be consumed by the EyeLink calibration loop.
/// Called from the WebSocket handler when the frontend sends a key command.
pub fn enqueue_key(key: c_short) {
    if let Ok(mut queue) = KEY_INPUT_QUEUE.lock() {
        queue.push_back(key);
    }
}

/// Send an "accept" key (Enter) to accept the current calibration point.
pub fn accept_calibration() {
    enqueue_key(ffi::ENTER_KEY);
}

/// Send a "cancel" key (Escape) to cancel calibration.
pub fn cancel_calibration() {
    enqueue_key(ffi::TERMINATE_KEY);
}

// ============================================================================
// Public API
// ============================================================================

/// Wrapper around a raw HOOKFCNS pointer to make it Send+Sync for OnceLock.
///
/// SAFETY: The HOOKFCNS struct contains only function pointers (which are
/// inherently Send+Sync) and is allocated once, never mutated, and never freed.
/// The eyelink_core C library reads this struct from any thread during calibration.
struct HookFcnsPtr(*mut ffi::HOOKFCNS);
unsafe impl Send for HookFcnsPtr {}
unsafe impl Sync for HookFcnsPtr {}

/// Get a pointer to the static HOOKFCNS struct with our callbacks registered.
///
/// The HOOKFCNS allocation is created exactly once (via OnceLock) and reused
/// for all subsequent calibration sessions. This avoids leaking ~200 bytes
/// per calibration call that the old Box::into_raw approach caused.
///
/// # Safety
/// The returned pointer is valid for the process lifetime. The function
/// pointers never change — only the global CALIBRATION_SENDER determines
/// where events are routed for each session.
fn get_hook_functions() -> *mut ffi::HOOKFCNS {
    static HOOKS: OnceLock<HookFcnsPtr> = OnceLock::new();
    HOOKS
        .get_or_init(|| {
            let hooks = Box::new(ffi::HOOKFCNS {
                setup_cal_display_hook: Some(setup_cal_display_hook),
                exit_cal_display_hook: Some(exit_cal_display_hook),
                record_abort_hide_hook: Some(record_abort_hide_hook),
                setup_image_display_hook: Some(setup_image_display_hook),
                image_title_hook: Some(image_title_hook),
                draw_image_line_hook: Some(draw_image_line_hook),
                set_image_palette_hook: Some(set_image_palette_hook),
                exit_image_display_hook: Some(exit_image_display_hook),
                clear_cal_display_hook: Some(clear_cal_display_hook),
                erase_cal_target_hook: Some(erase_cal_target_hook),
                draw_cal_target_hook: Some(draw_cal_target_hook),
                cal_target_beep_hook: Some(cal_target_beep_hook),
                cal_done_beep_hook: Some(cal_done_beep_hook),
                dc_done_beep_hook: Some(dc_done_beep_hook),
                dc_target_beep_hook: Some(dc_target_beep_hook),
                get_input_key_hook: Some(get_input_key_hook),
                alert_printf_hook: Some(alert_printf_hook),
            });
            HookFcnsPtr(Box::into_raw(hooks))
        })
        .0
}

/// Set the broadcast sender for calibration events.
/// Must be called before starting calibration.
pub fn set_calibration_sender(tx: broadcast::Sender<CalibrationEvent>) {
    if let Ok(mut guard) = CALIBRATION_SENDER.lock() {
        *guard = Some(tx);
    }
}

/// Clear the calibration sender after calibration completes.
pub fn clear_calibration_sender() {
    if let Ok(mut guard) = CALIBRATION_SENDER.lock() {
        *guard = None;
    }
}

/// Drain the key input queue, discarding any stale keys.
/// Should be called when calibration starts and ends to prevent
/// leftover keys from a cancelled session leaking into the next one.
fn drain_key_queue() {
    if let Ok(mut queue) = KEY_INPUT_QUEUE.lock() {
        queue.clear();
    }
}

/// Full cleanup for calibration state.
/// Clears both the broadcast sender and the key input queue.
/// Safe to call multiple times (idempotent).
fn cleanup_calibration() {
    clear_calibration_sender();
    drain_key_queue();
}

/// Drop guard that ensures calibration cleanup runs even if the
/// calibration task panics. This prevents CALIBRATION_SENDER from
/// holding a stale broadcast::Sender in the global static.
struct CalibrationCleanupGuard;

impl Drop for CalibrationCleanupGuard {
    fn drop(&mut self) {
        cleanup_calibration();
    }
}

/// Subscribe to calibration events.
/// Returns a receiver that will get CalibrationEvent notifications during calibration.
pub fn subscribe_calibration(
    tx: &broadcast::Sender<CalibrationEvent>,
) -> broadcast::Receiver<CalibrationEvent> {
    tx.subscribe()
}

/// Run the full EyeLink calibration/validation loop.
///
/// This is the main entry point for calibration. It:
/// 1. Registers HOOKFCNS callbacks
/// 2. Sets up the calibration event broadcast channel
/// 3. Calls do_tracker_setup() (BLOCKING — runs on spawn_blocking)
/// 4. Cleans up after calibration completes
///
/// Returns a broadcast::Sender that emits CalibrationEvents during the session.
/// WebSocket clients should subscribe before calling this.
pub fn setup_calibration_hooks() -> broadcast::Sender<CalibrationEvent> {
    let (tx, _) = broadcast::channel(64);

    // Drain any stale keys from a previous cancelled calibration
    drain_key_queue();

    // Register the calibration sender globally
    set_calibration_sender(tx.clone());

    // Register hook functions with eyelink_core (pointer is allocated once, reused)
    let hooks = get_hook_functions();
    // SAFETY: hooks pointer is valid for process lifetime (allocated once via OnceLock)
    if let Err(e) = unsafe { ffi::register_hooks(hooks) } {
        tracing::error!(
            device = "eyelink",
            "Failed to register calibration hooks: {}",
            e
        );
    }

    info!(
        device = "eyelink",
        "Calibration hooks registered, ready for do_tracker_setup()"
    );

    tx
}

/// Run do_tracker_setup() on a blocking thread.
/// This must be called after setup_calibration_hooks().
///
/// Returns the result code from do_tracker_setup().
/// Cleanup (sender clear + key queue drain) is guaranteed by a drop guard,
/// even if the blocking task panics.
pub async fn run_calibration() -> Result<c_short, String> {
    let result = tokio::task::spawn_blocking(|| -> Result<c_short, String> {
        // CalibrationCleanupGuard ensures cleanup_calibration() runs
        // even if do_tracker_setup() panics and the thread unwinds.
        let _guard = CalibrationCleanupGuard;

        info!(device = "eyelink", "Starting do_tracker_setup()");
        let result = ffi::do_tracker_setup()?;
        info!(
            device = "eyelink",
            "do_tracker_setup() returned: {}", result
        );

        // _guard drops here, calling cleanup_calibration()
        Ok(result)
    })
    .await
    .map_err(|e| format!("Calibration task panicked: {}", e))??;

    Ok(result)
}
