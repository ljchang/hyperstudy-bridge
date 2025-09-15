use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::devices::{DeviceType, DeviceStatus, DeviceInfo, DeviceConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BridgeCommand {
    #[serde(rename = "command")]
    Command {
        device: String,
        action: CommandAction,
        payload: Option<Value>,
        id: Option<String>,
    },
    #[serde(rename = "query")]
    Query {
        target: QueryTarget,
        id: Option<String>,
    },
    #[serde(rename = "subscribe")]
    Subscribe {
        device: Option<String>,
        events: Vec<String>,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe {
        device: Option<String>,
        events: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandAction {
    Connect,
    Disconnect,
    Send,
    Configure,
    Status,
    Heartbeat,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryTarget {
    Devices,
    Device(String),
    Metrics,
    Connections,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BridgeResponse {
    #[serde(rename = "status")]
    Status {
        device: String,
        status: DeviceStatus,
        timestamp: u64,
    },
    #[serde(rename = "data")]
    Data {
        device: String,
        payload: Value,
        timestamp: u64,
    },
    #[serde(rename = "error")]
    Error {
        device: Option<String>,
        message: String,
        code: Option<String>,
        timestamp: u64,
    },
    #[serde(rename = "ack")]
    Ack {
        id: String,
        success: bool,
        message: Option<String>,
        timestamp: u64,
    },
    #[serde(rename = "event")]
    Event {
        device: Option<String>,
        event: String,
        payload: Value,
        timestamp: u64,
    },
    #[serde(rename = "query_result")]
    QueryResult {
        id: Option<String>,
        data: Value,
        timestamp: u64,
    },
}

impl BridgeResponse {
    pub fn error(message: String) -> Self {
        Self::Error {
            device: None,
            message,
            code: None,
            timestamp: Self::timestamp(),
        }
    }

    pub fn device_error(device: String, message: String) -> Self {
        Self::Error {
            device: Some(device),
            message,
            code: None,
            timestamp: Self::timestamp(),
        }
    }

    pub fn ack(id: String, success: bool, message: Option<String>) -> Self {
        Self::Ack {
            id,
            success,
            message,
            timestamp: Self::timestamp(),
        }
    }

    pub fn data(device: String, payload: Value) -> Self {
        Self::Data {
            device,
            payload,
            timestamp: Self::timestamp(),
        }
    }

    pub fn status(device: String, status: DeviceStatus) -> Self {
        Self::Status {
            device,
            status,
            timestamp: Self::timestamp(),
        }
    }

    pub fn event(device: Option<String>, event: String, payload: Value) -> Self {
        Self::Event {
            device,
            event,
            payload,
            timestamp: Self::timestamp(),
        }
    }

    pub fn query_result(id: Option<String>, data: Value) -> Self {
        Self::QueryResult {
            id,
            data,
            timestamp: Self::timestamp(),
        }
    }

    fn timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

pub struct MessageHandler;

impl MessageHandler {
    pub fn parse_command(data: &str) -> Result<BridgeCommand, String> {
        serde_json::from_str(data)
            .map_err(|e| format!("Failed to parse command: {}", e))
    }

    pub fn serialize_response(response: &BridgeResponse) -> Result<String, String> {
        serde_json::to_string(response)
            .map_err(|e| format!("Failed to serialize response: {}", e))
    }

    pub fn validate_device_type(device: &str) -> Option<DeviceType> {
        match device.to_lowercase().as_str() {
            "ttl" => Some(DeviceType::TTL),
            "kernel" => Some(DeviceType::Kernel),
            "pupil" => Some(DeviceType::Pupil),
            "biopac" => Some(DeviceType::Biopac),
            "lsl" => Some(DeviceType::LSL),
            "mock" => Some(DeviceType::Mock),
            _ => None,
        }
    }
}