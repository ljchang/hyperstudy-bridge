//! Custom tracing layer for capturing logs and emitting them to the Tauri frontend.
//!
//! This module provides:
//! - `LogBuffer`: A thread-safe circular buffer for storing recent log entries
//! - `TauriLogLayer`: A custom tracing Layer that captures log events and emits them to the frontend
//! - `LogEntry`: The serializable log entry structure shared with the frontend

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, OnceLock, RwLock};
use tauri::{AppHandle, Emitter};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Global app handle for emitting events. Set during Tauri setup.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Global log buffer for storing logs before and after AppHandle is available.
static LOG_BUFFER: OnceLock<Arc<RwLock<LogBuffer>>> = OnceLock::new();

/// Default capacity for the log buffer
const DEFAULT_BUFFER_CAPACITY: usize = 500;

/// A log entry that can be serialized and sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub device: Option<String>,
    pub source: String,
}

impl LogEntry {
    /// Create a new log entry with the current timestamp.
    pub fn new(level: &str, message: String, device: Option<String>, source: String) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            message,
            device,
            source,
        }
    }
}

/// A circular buffer for storing log entries.
#[derive(Debug)]
pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    /// Create a new log buffer with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a log entry to the buffer, removing the oldest if at capacity.
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Get all log entries as a vector (newest last).
    pub fn get_all(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Get the number of entries in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the buffer.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_BUFFER_CAPACITY)
    }
}

/// Initialize the global app handle for event emission.
/// This must be called during Tauri's setup phase.
pub fn set_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Get a reference to the global log buffer.
/// Initializes the buffer on first access.
pub fn get_log_buffer() -> Arc<RwLock<LogBuffer>> {
    LOG_BUFFER
        .get_or_init(|| Arc::new(RwLock::new(LogBuffer::default())))
        .clone()
}

/// Get all logs from the global buffer.
pub fn get_all_logs() -> Vec<LogEntry> {
    let buffer = get_log_buffer();
    let guard = buffer.read().unwrap();
    guard.get_all()
}

/// A visitor that extracts message and device fields from tracing events.
struct LogVisitor {
    message: String,
    device: Option<String>,
}

impl LogVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            device: None,
        }
    }
}

impl Visit for LogVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "device" => self.device = Some(value.to_string()),
            _ => {
                // Append other string fields to message
                if !self.message.is_empty() {
                    self.message.push_str("; ");
                }
                self.message.push_str(&format!("{}={}", field.name(), value));
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{:?}", value),
            "device" => self.device = Some(format!("{:?}", value)),
            _ => {
                // Append other debug fields to message
                if !self.message.is_empty() {
                    self.message.push_str("; ");
                }
                self.message
                    .push_str(&format!("{}={:?}", field.name(), value));
            }
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if !self.message.is_empty() {
            self.message.push_str("; ");
        }
        self.message
            .push_str(&format!("{}={}", field.name(), value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if !self.message.is_empty() {
            self.message.push_str("; ");
        }
        self.message
            .push_str(&format!("{}={}", field.name(), value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if !self.message.is_empty() {
            self.message.push_str("; ");
        }
        self.message
            .push_str(&format!("{}={}", field.name(), value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if !self.message.is_empty() {
            self.message.push_str("; ");
        }
        self.message
            .push_str(&format!("{}={}", field.name(), value));
    }
}

/// A custom tracing Layer that captures log events and emits them to the Tauri frontend.
///
/// This layer:
/// 1. Extracts the message, level, and optional device field from log events
/// 2. Stores them in a circular buffer for historical access
/// 3. Emits them to the frontend via Tauri's event system (once AppHandle is available)
pub struct TauriLogLayer;

impl TauriLogLayer {
    pub fn new() -> Self {
        // Ensure the buffer is initialized
        let _ = get_log_buffer();
        Self
    }
}

impl Default for TauriLogLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for TauriLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Extract fields from the event
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        // Get the level as a string
        let level = match *event.metadata().level() {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "debug", // Map TRACE to debug for frontend
        };

        // Get the source from the target (module path)
        let target = event.metadata().target();
        let source = if target.starts_with("hyperstudy_bridge::") {
            // Extract the module name after the crate name
            target
                .strip_prefix("hyperstudy_bridge::")
                .unwrap_or(target)
                .split("::")
                .next()
                .unwrap_or("bridge")
        } else if target.starts_with("hyperstudy_bridge") {
            "bridge"
        } else {
            // External crate or unknown
            target.split("::").next().unwrap_or("system")
        };

        // Create the log entry
        let entry = LogEntry::new(level, visitor.message.clone(), visitor.device.clone(), source.to_string());

        // Store in buffer
        if let Ok(mut buffer) = get_log_buffer().write() {
            buffer.push(entry.clone());
        }

        // Emit to frontend if app handle is available
        if let Some(handle) = APP_HANDLE.get() {
            // Emit the log event to the frontend
            let _ = handle.emit("log_event", &entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_push_and_get() {
        let mut buffer = LogBuffer::new(3);

        buffer.push(LogEntry::new("info", "msg1".to_string(), None, "test".to_string()));
        buffer.push(LogEntry::new("info", "msg2".to_string(), None, "test".to_string()));
        buffer.push(LogEntry::new("info", "msg3".to_string(), None, "test".to_string()));

        let logs = buffer.get_all();
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "msg1");
        assert_eq!(logs[2].message, "msg3");
    }

    #[test]
    fn test_log_buffer_circular() {
        let mut buffer = LogBuffer::new(2);

        buffer.push(LogEntry::new("info", "msg1".to_string(), None, "test".to_string()));
        buffer.push(LogEntry::new("info", "msg2".to_string(), None, "test".to_string()));
        buffer.push(LogEntry::new("info", "msg3".to_string(), None, "test".to_string()));

        let logs = buffer.get_all();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].message, "msg2"); // msg1 was evicted
        assert_eq!(logs[1].message, "msg3");
    }

    #[test]
    fn test_log_entry_new() {
        let entry = LogEntry::new("error", "test message".to_string(), Some("ttl".to_string()), "bridge".to_string());

        assert_eq!(entry.level, "error");
        assert_eq!(entry.message, "test message");
        assert_eq!(entry.device, Some("ttl".to_string()));
        assert_eq!(entry.source, "bridge");
        assert!(!entry.timestamp.is_empty());
    }
}
