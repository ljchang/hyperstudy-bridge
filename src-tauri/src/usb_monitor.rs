//! USB device monitoring for detecting physical device connection/disconnection events.
//!
//! This module provides detection of USB device events by polling the serial port list.
//! It's primarily used to detect when the TTL pulse generator device is physically
//! unplugged, allowing immediate status updates without waiting for the next failed
//! I/O operation.
//!
//! The polling approach is used instead of OS-level event APIs because:
//! 1. It works reliably across all platforms (macOS, Linux, Windows)
//! 2. It uses the existing serialport crate (no additional dependencies)
//! 3. It's simpler and more maintainable
//! 4. The polling interval (1 second) provides acceptable responsiveness for UI updates

use serialport::{available_ports, SerialPortType};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Adafruit RP2040 USB identifiers for TTL pulse generator
pub const TTL_USB_VID: u16 = 0x239A;
pub const TTL_USB_PID: u16 = 0x80F1;

/// Polling interval for USB device detection
const POLL_INTERVAL_MS: u64 = 1000;

/// USB events we care about for device monitoring
#[derive(Debug, Clone)]
pub enum UsbEvent {
    /// A USB device was connected
    Connected {
        vid: u16,
        pid: u16,
        serial_number: Option<String>,
        port_name: String,
    },
    /// A USB device was disconnected
    Disconnected {
        vid: u16,
        pid: u16,
        serial_number: Option<String>,
        port_name: String,
    },
}

impl UsbEvent {
    /// Check if this event is for a TTL device
    pub fn is_ttl_device(&self) -> bool {
        match self {
            UsbEvent::Connected { vid, pid, .. } | UsbEvent::Disconnected { vid, pid, .. } => {
                *vid == TTL_USB_VID && *pid == TTL_USB_PID
            }
        }
    }

    /// Check if this is a disconnect event
    pub fn is_disconnect(&self) -> bool {
        matches!(self, UsbEvent::Disconnected { .. })
    }

    /// Get the port name associated with this event
    pub fn port_name(&self) -> &str {
        match self {
            UsbEvent::Connected { port_name, .. } | UsbEvent::Disconnected { port_name, .. } => {
                port_name
            }
        }
    }

    /// Get the serial number if available
    pub fn serial_number(&self) -> Option<&str> {
        match self {
            UsbEvent::Connected { serial_number, .. }
            | UsbEvent::Disconnected { serial_number, .. } => serial_number.as_deref(),
        }
    }
}

/// Information about a detected TTL device
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TtlDeviceInfo {
    port_name: String,
    serial_number: Option<String>,
}

/// Get the current set of connected TTL devices
fn get_connected_ttl_devices() -> HashSet<TtlDeviceInfo> {
    let mut devices = HashSet::new();

    if let Ok(ports) = available_ports() {
        for port in ports {
            if let SerialPortType::UsbPort(usb_info) = &port.port_type {
                if usb_info.vid == TTL_USB_VID && usb_info.pid == TTL_USB_PID {
                    // On macOS, skip /dev/tty.* ports (duplicates of /dev/cu.*)
                    if !port.port_name.starts_with("/dev/tty.") {
                        devices.insert(TtlDeviceInfo {
                            port_name: port.port_name,
                            serial_number: usb_info.serial_number.clone(),
                        });
                    }
                }
            }
        }
    }

    devices
}

/// Start USB device monitoring and return a receiver for filtered TTL device events.
///
/// This spawns a background task that polls the serial port list and detects
/// when TTL devices (VID: 0x239A, PID: 0x80F1) are connected or disconnected.
///
/// # Returns
/// A channel receiver that will receive `UsbEvent` notifications when TTL devices
/// are connected or disconnected.
pub fn start_usb_monitor() -> mpsc::Receiver<UsbEvent> {
    let (event_tx, event_rx) = mpsc::channel(100);

    // Spawn the polling task
    tokio::spawn(async move {
        info!(
            "Starting USB device polling (interval: {}ms) for TTL devices (VID: 0x{:04X}, PID: 0x{:04X})",
            POLL_INTERVAL_MS, TTL_USB_VID, TTL_USB_PID
        );

        let mut interval_timer = interval(Duration::from_millis(POLL_INTERVAL_MS));
        let mut previous_devices = get_connected_ttl_devices();

        // Log initial state
        if previous_devices.is_empty() {
            debug!("No TTL devices currently connected");
        } else {
            for device in &previous_devices {
                debug!(
                    "TTL device already connected: {} (S/N: {:?})",
                    device.port_name, device.serial_number
                );
            }
        }

        loop {
            interval_timer.tick().await;

            let current_devices = get_connected_ttl_devices();

            // Check for newly connected devices
            for device in current_devices.difference(&previous_devices) {
                info!(
                    "TTL device connected: {} (S/N: {:?})",
                    device.port_name, device.serial_number
                );

                let event = UsbEvent::Connected {
                    vid: TTL_USB_VID,
                    pid: TTL_USB_PID,
                    serial_number: device.serial_number.clone(),
                    port_name: device.port_name.clone(),
                };

                if event_tx.send(event).await.is_err() {
                    warn!("USB event receiver dropped, stopping monitor");
                    return;
                }
            }

            // Check for disconnected devices
            for device in previous_devices.difference(&current_devices) {
                warn!(
                    "TTL device disconnected: {} (S/N: {:?})",
                    device.port_name, device.serial_number
                );

                let event = UsbEvent::Disconnected {
                    vid: TTL_USB_VID,
                    pid: TTL_USB_PID,
                    serial_number: device.serial_number.clone(),
                    port_name: device.port_name.clone(),
                };

                if event_tx.send(event).await.is_err() {
                    warn!("USB event receiver dropped, stopping monitor");
                    return;
                }
            }

            previous_devices = current_devices;
        }
    });

    event_rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usb_event_is_ttl_device() {
        let connect_event = UsbEvent::Connected {
            vid: TTL_USB_VID,
            pid: TTL_USB_PID,
            serial_number: Some("test123".to_string()),
            port_name: "/dev/cu.usbmodem1234".to_string(),
        };
        assert!(connect_event.is_ttl_device());

        let disconnect_event = UsbEvent::Disconnected {
            vid: TTL_USB_VID,
            pid: TTL_USB_PID,
            serial_number: None,
            port_name: "/dev/cu.usbmodem1234".to_string(),
        };
        assert!(disconnect_event.is_ttl_device());

        let other_device = UsbEvent::Connected {
            vid: 0x1234,
            pid: 0x5678,
            serial_number: None,
            port_name: "/dev/cu.usbmodem5678".to_string(),
        };
        assert!(!other_device.is_ttl_device());
    }

    #[test]
    fn test_usb_event_is_disconnect() {
        let connect_event = UsbEvent::Connected {
            vid: TTL_USB_VID,
            pid: TTL_USB_PID,
            serial_number: None,
            port_name: "/dev/cu.usbmodem1234".to_string(),
        };
        assert!(!connect_event.is_disconnect());

        let disconnect_event = UsbEvent::Disconnected {
            vid: TTL_USB_VID,
            pid: TTL_USB_PID,
            serial_number: None,
            port_name: "/dev/cu.usbmodem1234".to_string(),
        };
        assert!(disconnect_event.is_disconnect());
    }

    #[test]
    fn test_get_connected_ttl_devices() {
        // This test just verifies the function doesn't panic
        // Actual device detection depends on hardware presence
        let devices = get_connected_ttl_devices();
        // The result is valid whether empty or populated - just verify no panic
        let _ = devices.len();
    }
}
