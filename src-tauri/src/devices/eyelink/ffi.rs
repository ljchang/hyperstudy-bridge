//! FFI bindings to the SR Research `eyelink_core` C library via runtime dynamic loading.
//!
//! Uses `libloading` to load the eyelink_core shared library at runtime instead of
//! linking at build time. This means:
//! - The EyeLink SDK does NOT need to be installed to build the application
//! - One universal binary works for all users
//! - If the SDK is not installed, a clear error message is returned instead of crashing
//!
//! References:
//!   - SR Research EyeLink Programmer's Guide
//!   - core_expt.h from the EyeLink Developers Kit

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use libloading::Library;
use std::os::raw::{c_char, c_short, c_void};
use std::sync::OnceLock;

// ============================================================================
// Constants
// ============================================================================

/// Binocular eye indices
pub const LEFT_EYE: usize = 0;
pub const RIGHT_EYE: usize = 1;

/// Connection modes for open_eyelink_connection
pub const CONNECT_NORMAL: c_short = 0;
pub const CONNECT_DUMMY: c_short = -1;

/// Return values
pub const OK_RESULT: c_short = 0;

/// eyelink_is_connected() return values
pub const EYELINK_NOT_CONNECTED: c_short = 0;
pub const EYELINK_CONNECTED: c_short = 1;
pub const EYELINK_BROADCAST: c_short = 2;

/// Sample data flags
pub const SAMPLE_LEFT: u16 = 0x8000;
pub const SAMPLE_RIGHT: u16 = 0x4000;

/// Calibration target beep types (used in CalibrationEvent::PlayBeep)
pub const CAL_TARG_BEEP: i32 = 1;
pub const CAL_GOOD_BEEP: i32 = 0;
pub const CAL_ERR_BEEP: i32 = -1;
pub const DC_TARG_BEEP: i32 = 3;
pub const DC_GOOD_BEEP: i32 = 2;
pub const DC_ERR_BEEP: i32 = -2;

/// Key input constants
pub const JUNK_KEY: c_short = 0;
pub const TERMINATE_KEY: c_short = 27; // ESC
pub const ENTER_KEY: c_short = 13;

// ============================================================================
// Data Types
// ============================================================================

/// Float sample from EyeLink — real-time gaze data.
/// This matches the FSAMPLE struct from edf_data.h / eyelink.h exactly.
///
/// **CRITICAL**: The field order and sizes must match the C ABI precisely.
/// The eyelink_core library writes directly into this struct via pointer cast.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FSAMPLE {
    /// Timestamp (tracker time, milliseconds)
    pub time: u32,
    /// Pupil x position [left, right]
    pub px: [f32; 2],
    /// Pupil y position [left, right]
    pub py: [f32; 2],
    /// Head-reference x [left, right] (head-mounted only)
    pub hx: [f32; 2],
    /// Head-reference y [left, right] (head-mounted only)
    pub hy: [f32; 2],
    /// Pupil area [left, right]
    pub pa: [f32; 2],
    /// Gaze x position in pixels [left, right] — primary data
    pub gx: [f32; 2],
    /// Gaze y position in pixels [left, right] — primary data
    pub gy: [f32; 2],
    /// Angular resolution: pixels per degree x
    pub rx: f32,
    /// Angular resolution: pixels per degree y
    pub ry: f32,
    // -- Velocity fields (must be present to match C layout) --
    /// Gaze x velocity [left, right]
    pub gxvel: [f32; 2],
    /// Gaze y velocity [left, right]
    pub gyvel: [f32; 2],
    /// Head-reference x velocity [left, right]
    pub hxvel: [f32; 2],
    /// Head-reference y velocity [left, right]
    pub hyvel: [f32; 2],
    /// Resolution x velocity [left, right]
    pub rxvel: [f32; 2],
    /// Resolution y velocity [left, right]
    pub ryvel: [f32; 2],
    /// Filtered gaze x velocity [left, right]
    pub fgxvel: [f32; 2],
    /// Filtered gaze y velocity [left, right]
    pub fgyvel: [f32; 2],
    /// Filtered head-reference x velocity [left, right]
    pub fhxvel: [f32; 2],
    /// Filtered head-reference y velocity [left, right]
    pub fhyvel: [f32; 2],
    /// Filtered resolution x velocity [left, right]
    pub frxvel: [f32; 2],
    /// Filtered resolution y velocity [left, right]
    pub fryvel: [f32; 2],
    // -- End velocity fields --
    /// Head tracker data (raw, not prescaled)
    pub hdata: [c_short; 8],
    /// Flags indicating which fields contain valid data
    pub flags: u16,
    /// Extra input data from tracker
    pub input: u16,
    /// Button state and changes
    pub buttons: u16,
    /// Head tracker data type (0 = none)
    pub htype: c_short,
    /// Process error flags
    pub errors: u16,
}

/// Calibration/graphics hook functions.
/// The EyeLink C API calls back into these function pointers during calibration
/// (triggered by do_tracker_setup). We register our Rust callbacks here to
/// forward calibration events over WebSocket to the frontend.
///
/// **CRITICAL**: This must match the HOOKFCNS typedef in core_expt.h exactly.
/// The C struct has 4 separate beep hooks (not a single play_target_beep), and
/// the return types (void vs INT16) must match precisely.
#[repr(C)]
pub struct HOOKFCNS {
    /// Called to set up the calibration display
    pub setup_cal_display_hook: Option<unsafe extern "C" fn() -> c_short>,
    /// Called when calibration display should close
    pub exit_cal_display_hook: Option<unsafe extern "C" fn()>,
    /// Called when recording should be hidden (abort)
    pub record_abort_hide_hook: Option<unsafe extern "C" fn()>,
    /// Called to set up camera image display
    pub setup_image_display_hook:
        Option<unsafe extern "C" fn(width: c_short, height: c_short) -> c_short>,
    /// Called with camera image title
    pub image_title_hook: Option<unsafe extern "C" fn(threshold: c_short, title: *mut c_char)>,
    /// Called to draw a line of camera image data
    pub draw_image_line_hook: Option<
        unsafe extern "C" fn(
            width: c_short,
            line: *mut c_char,
            totlines: c_short,
            image_line: c_short,
        ),
    >,
    /// Called to set camera image palette
    pub set_image_palette_hook:
        Option<unsafe extern "C" fn(ncolors: c_short, r: *mut u8, g: *mut u8, b: *mut u8)>,
    /// Called when camera image display should close
    pub exit_image_display_hook: Option<unsafe extern "C" fn()>,
    /// Called to clear the calibration display
    pub clear_cal_display_hook: Option<unsafe extern "C" fn()>,
    /// Called to erase the current calibration target
    pub erase_cal_target_hook: Option<unsafe extern "C" fn()>,
    /// Called to draw a calibration target at (x, y)
    pub draw_cal_target_hook: Option<unsafe extern "C" fn(x: c_short, y: c_short)>,
    /// Beep for calibration target presentation
    pub cal_target_beep_hook: Option<unsafe extern "C" fn()>,
    /// Beep when calibration point is done (error=0 means good)
    pub cal_done_beep_hook: Option<unsafe extern "C" fn(error: c_short)>,
    /// Beep when drift-correct point is done
    pub dc_done_beep_hook: Option<unsafe extern "C" fn(error: c_short)>,
    /// Beep for drift-correct target presentation
    pub dc_target_beep_hook: Option<unsafe extern "C" fn()>,
    /// Called to get keyboard input during calibration
    pub get_input_key_hook: Option<unsafe extern "C" fn(event: *mut c_void) -> c_short>,
    /// Called to display alert messages from the tracker
    pub alert_printf_hook: Option<unsafe extern "C" fn(msg: *const c_char)>,
}

// ============================================================================
// Runtime-loaded Library
// ============================================================================

/// Holds dynamically loaded eyelink_core function pointers.
///
/// The `_lib` field keeps the shared library loaded in memory for the process
/// lifetime. All function pointers are resolved once at load time.
pub struct EyeLinkLib {
    _lib: Library, // Must stay alive while function pointers are in use

    // Connection
    pub open_eyelink_connection: unsafe extern "C" fn(c_short) -> c_short,
    pub close_eyelink_connection: unsafe extern "C" fn(),
    pub set_eyelink_address: unsafe extern "C" fn(*const c_char),
    pub eyelink_is_connected: unsafe extern "C" fn() -> c_short,

    // Recording
    pub start_recording: unsafe extern "C" fn(c_short, c_short, c_short, c_short) -> c_short,
    pub stop_recording: unsafe extern "C" fn(),

    // Commands — variadic in C, but we always call with exactly ("%s", string)
    pub eyecmd_printf: unsafe extern "C" fn(*const c_char, *const c_char) -> c_short,
    pub eyemsg_printf: unsafe extern "C" fn(*const c_char, *const c_char) -> c_short,

    // Data
    pub eyelink_newest_float_sample: unsafe extern "C" fn(*mut FSAMPLE) -> c_short,
    pub eyelink_get_next_data: unsafe extern "C" fn(*mut c_void) -> c_short,

    // Calibration
    pub do_tracker_setup: unsafe extern "C" fn() -> c_short,
    pub setup_graphic_hook_functions: unsafe extern "C" fn(*mut HOOKFCNS),

    // Version
    pub eyelink_get_tracker_version: unsafe extern "C" fn(*mut c_char) -> c_short,
}

// SAFETY: EyeLinkLib contains only function pointers (which are inherently Send+Sync)
// and a Library handle. The eyelink_core C library is designed for single-threaded use,
// but our safe wrappers below serialize access through Tokio's spawn_blocking.
unsafe impl Send for EyeLinkLib {}
unsafe impl Sync for EyeLinkLib {}

impl EyeLinkLib {
    /// Attempt to load the eyelink_core shared library from platform-specific paths.
    fn load() -> Result<Self, String> {
        let lib = Self::load_library()?;

        // SAFETY: We resolve each symbol by name from the loaded library.
        // The function signatures must match the C declarations in core_expt.h.
        unsafe {
            let open_eyelink_connection = *lib
                .get::<unsafe extern "C" fn(c_short) -> c_short>(b"open_eyelink_connection\0")
                .map_err(|e| format!("Failed to load open_eyelink_connection: {}", e))?;
            let close_eyelink_connection = *lib
                .get::<unsafe extern "C" fn()>(b"close_eyelink_connection\0")
                .map_err(|e| format!("Failed to load close_eyelink_connection: {}", e))?;
            let set_eyelink_address = *lib
                .get::<unsafe extern "C" fn(*const c_char)>(b"set_eyelink_address\0")
                .map_err(|e| format!("Failed to load set_eyelink_address: {}", e))?;
            let eyelink_is_connected = *lib
                .get::<unsafe extern "C" fn() -> c_short>(b"eyelink_is_connected\0")
                .map_err(|e| format!("Failed to load eyelink_is_connected: {}", e))?;
            let start_recording = *lib
                .get::<unsafe extern "C" fn(c_short, c_short, c_short, c_short) -> c_short>(
                    b"start_recording\0",
                )
                .map_err(|e| format!("Failed to load start_recording: {}", e))?;
            let stop_recording = *lib
                .get::<unsafe extern "C" fn()>(b"stop_recording\0")
                .map_err(|e| format!("Failed to load stop_recording: {}", e))?;
            let eyecmd_printf = *lib
                .get::<unsafe extern "C" fn(*const c_char, *const c_char) -> c_short>(
                    b"eyecmd_printf\0",
                )
                .map_err(|e| format!("Failed to load eyecmd_printf: {}", e))?;
            let eyemsg_printf = *lib
                .get::<unsafe extern "C" fn(*const c_char, *const c_char) -> c_short>(
                    b"eyemsg_printf\0",
                )
                .map_err(|e| format!("Failed to load eyemsg_printf: {}", e))?;
            let eyelink_newest_float_sample = *lib
                .get::<unsafe extern "C" fn(*mut FSAMPLE) -> c_short>(
                    b"eyelink_newest_float_sample\0",
                )
                .map_err(|e| format!("Failed to load eyelink_newest_float_sample: {}", e))?;
            let eyelink_get_next_data = *lib
                .get::<unsafe extern "C" fn(*mut c_void) -> c_short>(b"eyelink_get_next_data\0")
                .map_err(|e| format!("Failed to load eyelink_get_next_data: {}", e))?;
            let do_tracker_setup = *lib
                .get::<unsafe extern "C" fn() -> c_short>(b"do_tracker_setup\0")
                .map_err(|e| format!("Failed to load do_tracker_setup: {}", e))?;
            let setup_graphic_hook_functions = *lib
                .get::<unsafe extern "C" fn(*mut HOOKFCNS)>(b"setup_graphic_hook_functions\0")
                .map_err(|e| format!("Failed to load setup_graphic_hook_functions: {}", e))?;
            let eyelink_get_tracker_version = *lib
                .get::<unsafe extern "C" fn(*mut c_char) -> c_short>(
                    b"eyelink_get_tracker_version\0",
                )
                .map_err(|e| format!("Failed to load eyelink_get_tracker_version: {}", e))?;

            Ok(EyeLinkLib {
                _lib: lib,
                open_eyelink_connection,
                close_eyelink_connection,
                set_eyelink_address,
                eyelink_is_connected,
                start_recording,
                stop_recording,
                eyecmd_printf,
                eyemsg_printf,
                eyelink_newest_float_sample,
                eyelink_get_next_data,
                do_tracker_setup,
                setup_graphic_hook_functions,
                eyelink_get_tracker_version,
            })
        }
    }

    /// Load the platform-specific shared library.
    fn load_library() -> Result<Library, String> {
        let lib_paths = Self::library_paths();

        let mut last_error = String::new();
        for path in &lib_paths {
            match unsafe { Library::new(path) } {
                Ok(lib) => return Ok(lib),
                Err(e) => {
                    last_error = format!("{}", e);
                }
            }
        }

        Err(format!(
            "EyeLink SDK not found. Install the SR Research EyeLink Developers Kit \
             from www.sr-research.com/support (tried: {}, last error: {})",
            lib_paths.join(", "),
            last_error,
        ))
    }

    /// Platform-specific library search paths.
    fn library_paths() -> Vec<&'static str> {
        #[cfg(target_os = "macos")]
        {
            vec!["/Library/Frameworks/eyelink_core.framework/eyelink_core"]
        }

        #[cfg(target_os = "linux")]
        {
            vec!["libeyelink_core.so"]
        }

        #[cfg(target_os = "windows")]
        {
            vec!["eyelink_core.dll"]
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            vec![]
        }
    }
}

/// Process-lifetime cache for the loaded library.
static EYELINK_LIB: OnceLock<Result<EyeLinkLib, String>> = OnceLock::new();

/// Get a reference to the loaded EyeLink library, loading it on first call.
///
/// Returns `Ok(&EyeLinkLib)` if the SDK is installed and loaded successfully,
/// or `Err(String)` with an informative message if the SDK is not found.
pub fn get_eyelink_lib() -> Result<&'static EyeLinkLib, String> {
    EYELINK_LIB
        .get_or_init(EyeLinkLib::load)
        .as_ref()
        .map_err(|e| e.clone())
}

// ============================================================================
// Safe Wrappers
// ============================================================================

/// Safe wrapper to set the EyeLink Host PC address.
pub fn set_address(addr: &str) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    let c_addr = std::ffi::CString::new(addr)
        .map_err(|_| "EyeLink address must not contain NUL bytes".to_string())?;
    unsafe {
        (lib.set_eyelink_address)(c_addr.as_ptr());
    }
    Ok(())
}

/// Safe wrapper to open connection.
/// Returns Ok(()) on success, Err with description on failure.
pub fn connect(mode: c_short) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    let result = unsafe { (lib.open_eyelink_connection)(mode) };
    if result == OK_RESULT {
        Ok(())
    } else {
        Err(format!(
            "open_eyelink_connection failed with code: {}",
            result
        ))
    }
}

/// Safe wrapper to close connection.
pub fn disconnect() -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    unsafe {
        (lib.close_eyelink_connection)();
    }
    Ok(())
}

/// Safe wrapper to check connection status.
pub fn is_connected() -> Result<c_short, String> {
    let lib = get_eyelink_lib()?;
    Ok(unsafe { (lib.eyelink_is_connected)() })
}

/// Safe wrapper to start recording.
pub fn begin_recording(
    file_samples: bool,
    file_events: bool,
    link_samples: bool,
    link_events: bool,
) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    let result = unsafe {
        (lib.start_recording)(
            file_samples as c_short,
            file_events as c_short,
            link_samples as c_short,
            link_events as c_short,
        )
    };
    if result == OK_RESULT {
        Ok(())
    } else {
        Err(format!("start_recording failed with code: {}", result))
    }
}

/// Safe wrapper to stop recording.
pub fn end_recording() -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    unsafe {
        (lib.stop_recording)();
    }
    Ok(())
}

/// Safe wrapper to send a command to the tracker.
///
/// Uses "%s" format specifier to prevent format string injection —
/// user-provided strings are passed as data arguments, not format strings.
pub fn send_command(cmd: &str) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    let c_cmd = std::ffi::CString::new(cmd).map_err(|_| "Command contains NUL byte".to_string())?;
    let fmt = c"%s".as_ptr();
    let result = unsafe { (lib.eyecmd_printf)(fmt, c_cmd.as_ptr()) };
    if result == OK_RESULT {
        Ok(())
    } else {
        Err(format!("eyecmd_printf failed with code: {}", result))
    }
}

/// Safe wrapper to send a message to the EDF file (marker).
///
/// Uses "%s" format specifier to prevent format string injection —
/// user-provided strings are passed as data arguments, not format strings.
pub fn send_message(msg: &str) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    let c_msg = std::ffi::CString::new(msg).map_err(|_| "Message contains NUL byte".to_string())?;
    let fmt = c"%s".as_ptr();
    let result = unsafe { (lib.eyemsg_printf)(fmt, c_msg.as_ptr()) };
    if result == OK_RESULT {
        Ok(())
    } else {
        Err(format!("eyemsg_printf failed with code: {}", result))
    }
}

/// Safe wrapper to get the newest float sample.
/// Returns Some(sample) if new data is available, None if SDK not loaded or no data.
pub fn get_newest_sample() -> Option<FSAMPLE> {
    let lib = get_eyelink_lib().ok()?;
    let mut sample = FSAMPLE::default();
    let result = unsafe { (lib.eyelink_newest_float_sample)(&mut sample) };
    if result > 0 {
        Some(sample)
    } else {
        None
    }
}

/// Safe wrapper to get tracker version.
pub fn get_tracker_version() -> Result<(c_short, String), String> {
    let lib = get_eyelink_lib()?;
    let mut version_buf = [0i8; 256];
    let version = unsafe { (lib.eyelink_get_tracker_version)(version_buf.as_mut_ptr()) };
    let version_str = unsafe {
        std::ffi::CStr::from_ptr(version_buf.as_ptr())
            .to_string_lossy()
            .into_owned()
    };
    Ok((version, version_str))
}

/// Safe wrapper for do_tracker_setup (blocking calibration loop).
pub fn do_tracker_setup() -> Result<c_short, String> {
    let lib = get_eyelink_lib()?;
    Ok(unsafe { (lib.do_tracker_setup)() })
}

/// Register calibration hook functions.
///
/// # Safety
/// The hooks struct must outlive the calibration session. In practice,
/// we use a static HOOKFCNS instance that lives for the process lifetime.
pub unsafe fn register_hooks(hooks: *mut HOOKFCNS) -> Result<(), String> {
    let lib = get_eyelink_lib()?;
    (lib.setup_graphic_hook_functions)(hooks);
    Ok(())
}
