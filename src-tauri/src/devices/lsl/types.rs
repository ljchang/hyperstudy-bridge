use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// LSL channel format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelFormat {
    Float32,
    Float64,
    String,
    Int32,
    Int16,
    Int8,
}

impl fmt::Display for ChannelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelFormat::Float32 => write!(f, "float32"),
            ChannelFormat::Float64 => write!(f, "float64"),
            ChannelFormat::String => write!(f, "string"),
            ChannelFormat::Int32 => write!(f, "int32"),
            ChannelFormat::Int16 => write!(f, "int16"),
            ChannelFormat::Int8 => write!(f, "int8"),
        }
    }
}

/// Stream types for different device categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamType {
    /// TTL pulse markers
    Markers,
    /// fNIRS optical density data
    FNIRS,
    /// Eye tracking gaze data
    Gaze,
    /// Physiological signals (EEG, EMG, ECG, etc.)
    Biosignals,
    /// Generic data stream
    Generic,
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamType::Markers => write!(f, "Markers"),
            StreamType::FNIRS => write!(f, "fNIRS"),
            StreamType::Gaze => write!(f, "Gaze"),
            StreamType::Biosignals => write!(f, "Biosignals"),
            StreamType::Generic => write!(f, "Generic"),
        }
    }
}

/// Stream information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub name: String,
    pub stream_type: StreamType,
    pub channel_count: u32,
    pub nominal_srate: f64,
    pub channel_format: ChannelFormat,
    pub source_id: String,
    pub hostname: String,
    pub metadata: HashMap<String, String>,
}

impl StreamInfo {
    pub fn new(
        name: String,
        stream_type: StreamType,
        channel_count: u32,
        nominal_srate: f64,
        channel_format: ChannelFormat,
        source_id: String,
    ) -> Self {
        Self {
            name,
            stream_type,
            channel_count,
            nominal_srate,
            channel_format,
            source_id,
            hostname: String::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create stream info for TTL markers
    pub fn ttl_markers(device_id: &str) -> Self {
        Self::new(
            format!("{}_TTL_Markers", device_id),
            StreamType::Markers,
            1,
            0.0, // Irregular sampling rate
            ChannelFormat::String,
            device_id.to_string(),
        )
    }

    /// Create stream info for Kernel fNIRS data
    pub fn kernel_fnirs(device_id: &str, channel_count: u32) -> Self {
        let mut info = Self::new(
            format!("{}_fNIRS", device_id),
            StreamType::FNIRS,
            channel_count,
            10.0, // Typical fNIRS sampling rate
            ChannelFormat::Float32,
            device_id.to_string(),
        );
        info.metadata
            .insert("unit".to_string(), "optical_density".to_string());
        info
    }

    /// Create stream info for Pupil gaze data
    pub fn pupil_gaze(device_id: &str) -> Self {
        let mut info = Self::new(
            format!("{}_Gaze", device_id),
            StreamType::Gaze,
            3,     // x, y, confidence
            120.0, // Typical eye tracker sampling rate
            ChannelFormat::Float32,
            device_id.to_string(),
        );
        info.metadata
            .insert("unit".to_string(), "normalized".to_string());
        info
    }

    /// Create stream info for Biopac biosignals
    pub fn biopac_biosignals(device_id: &str, channel_count: u32, sampling_rate: f64) -> Self {
        let mut info = Self::new(
            format!("{}_Biosignals", device_id),
            StreamType::Biosignals,
            channel_count,
            sampling_rate,
            ChannelFormat::Float32,
            device_id.to_string(),
        );
        info.metadata
            .insert("unit".to_string(), "microvolts".to_string());
        info
    }

    /// Add metadata to the stream info
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set hostname
    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = hostname;
        self
    }
}

/// LSL data sample with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub data: SampleData,
    pub timestamp: f64,
}

/// Sample data variants for different channel formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SampleData {
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    String(Vec<String>),
    Int32(Vec<i32>),
    Int16(Vec<i16>),
    Int8(Vec<i8>),
}

impl SampleData {
    /// Get the number of channels in the sample
    pub fn channel_count(&self) -> usize {
        match self {
            SampleData::Float32(data) => data.len(),
            SampleData::Float64(data) => data.len(),
            SampleData::String(data) => data.len(),
            SampleData::Int32(data) => data.len(),
            SampleData::Int16(data) => data.len(),
            SampleData::Int8(data) => data.len(),
        }
    }

    /// Create a TTL marker sample
    pub fn ttl_marker(marker: String) -> Self {
        SampleData::String(vec![marker])
    }

    /// Create a float32 sample (common for most numeric data)
    pub fn float32(data: Vec<f32>) -> Self {
        SampleData::Float32(data)
    }

    /// Convert to bytes for transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SampleData::Float32(data) => {
                let mut bytes = Vec::with_capacity(data.len() * 4);
                for &value in data {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes
            }
            SampleData::Float64(data) => {
                let mut bytes = Vec::with_capacity(data.len() * 8);
                for &value in data {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes
            }
            SampleData::String(data) => serde_json::to_vec(data).unwrap_or_default(),
            SampleData::Int32(data) => {
                let mut bytes = Vec::with_capacity(data.len() * 4);
                for &value in data {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes
            }
            SampleData::Int16(data) => {
                let mut bytes = Vec::with_capacity(data.len() * 2);
                for &value in data {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes
            }
            SampleData::Int8(data) => data.iter().map(|&x| x as u8).collect(),
        }
    }
}

/// Stream status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub active: bool,
    pub sample_count: u64,
    pub last_timestamp: f64,
    pub data_loss: f64, // Percentage
    pub time_correction: f64,
}

impl Default for StreamStatus {
    fn default() -> Self {
        Self {
            active: false,
            sample_count: 0,
            last_timestamp: 0.0,
            data_loss: 0.0,
            time_correction: 0.0,
        }
    }
}

/// LSL configuration for device integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LslConfig {
    /// Auto-create outlets for bridge devices
    pub auto_create_outlets: bool,
    /// Discover and connect to external streams
    pub auto_discover_inlets: bool,
    /// Maximum number of inlet connections
    pub max_inlets: usize,
    /// Stream discovery timeout in seconds
    pub discovery_timeout: f64,
    /// Buffer size for data buffering
    pub buffer_size: usize,
    /// Enable time synchronization
    pub enable_time_sync: bool,
    /// Stream name filters (regex patterns)
    pub stream_filters: Vec<String>,
    /// Metadata to add to created outlets
    pub outlet_metadata: HashMap<String, String>,
}

impl Default for LslConfig {
    fn default() -> Self {
        Self {
            auto_create_outlets: true,
            auto_discover_inlets: false,
            max_inlets: 10,
            discovery_timeout: 5.0,
            buffer_size: 1000,
            enable_time_sync: true,
            stream_filters: vec![],
            outlet_metadata: HashMap::new(),
        }
    }
}

/// Errors specific to LSL operations
#[derive(Debug, thiserror::Error)]
pub enum LslError {
    #[error("Failed to create stream outlet: {0}")]
    OutletCreationFailed(String),

    #[error("Failed to create stream inlet: {0}")]
    InletCreationFailed(String),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Data format mismatch: expected {expected}, got {actual}")]
    DataFormatMismatch { expected: String, actual: String },

    #[error("Buffer overflow: {0}")]
    BufferOverflow(String),

    #[error("Time synchronization failed: {0}")]
    TimeSyncFailed(String),

    #[error("Stream discovery timeout")]
    DiscoveryTimeout,

    #[error("Invalid sample data: {0}")]
    InvalidSampleData(String),

    #[error("LSL library error: {0}")]
    LslLibraryError(String),
}
