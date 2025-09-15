use super::{Device, DeviceConfig, DeviceError, DeviceInfo, DeviceStatus, DeviceType};
use async_trait::async_trait;
use std::collections::VecDeque;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

#[derive(Debug)]
pub struct MockDevice {
    id: String,
    name: String,
    status: DeviceStatus,
    config: DeviceConfig,
    send_buffer: VecDeque<Vec<u8>>,
    receive_buffer: VecDeque<Vec<u8>>,
    simulate_latency: bool,
    latency_ms: u64,
}

impl MockDevice {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            status: DeviceStatus::Disconnected,
            config: DeviceConfig::default(),
            send_buffer: VecDeque::new(),
            receive_buffer: VecDeque::new(),
            simulate_latency: false,
            latency_ms: 10,
        }
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.simulate_latency = true;
        self.latency_ms = latency_ms;
        self
    }

    pub fn push_receive_data(&mut self, data: Vec<u8>) {
        self.receive_buffer.push_back(data);
    }

    pub fn pop_send_data(&mut self) -> Option<Vec<u8>> {
        self.send_buffer.pop_front()
    }

    pub fn get_all_sent_data(&self) -> Vec<Vec<u8>> {
        self.send_buffer.clone().into_iter().collect()
    }

    async fn simulate_operation(&self) {
        if self.simulate_latency {
            sleep(Duration::from_millis(self.latency_ms)).await;
        }
    }
}

#[async_trait]
impl Device for MockDevice {
    async fn connect(&mut self) -> Result<(), DeviceError> {
        info!("Connecting mock device: {}", self.name);
        self.status = DeviceStatus::Connecting;

        self.simulate_operation().await;

        self.status = DeviceStatus::Connected;
        info!("Mock device connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DeviceError> {
        info!("Disconnecting mock device: {}", self.name);

        self.simulate_operation().await;

        self.status = DeviceStatus::Disconnected;
        self.send_buffer.clear();
        self.receive_buffer.clear();
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        debug!("Mock device sending {} bytes", data.len());

        self.simulate_operation().await;

        self.send_buffer.push_back(data.to_vec());

        if data == b"ECHO" {
            self.receive_buffer.push_back(data.to_vec());
        }

        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>, DeviceError> {
        if self.status != DeviceStatus::Connected {
            return Err(DeviceError::NotConnected);
        }

        self.simulate_operation().await;

        Ok(self.receive_buffer.pop_front().unwrap_or_default())
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            device_type: DeviceType::Mock,
            status: self.status,
            metadata: serde_json::json!({
                "simulate_latency": self.simulate_latency,
                "latency_ms": self.latency_ms,
                "send_buffer_size": self.send_buffer.len(),
                "receive_buffer_size": self.receive_buffer.len(),
            }),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        self.status
    }

    fn configure(&mut self, config: DeviceConfig) -> Result<(), DeviceError> {
        self.config = config;

        if let Some(custom) = self.config.custom_settings.as_object() {
            if let Some(latency) = custom.get("simulate_latency").and_then(|v| v.as_bool()) {
                self.simulate_latency = latency;
            }

            if let Some(ms) = custom.get("latency_ms").and_then(|v| v.as_u64()) {
                self.latency_ms = ms;
            }
        }

        Ok(())
    }

    async fn heartbeat(&mut self) -> Result<(), DeviceError> {
        if self.status == DeviceStatus::Connected {
            Ok(())
        } else {
            Err(DeviceError::NotConnected)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_device_basic_operations() {
        let mut device = MockDevice::new("test_1".to_string(), "Test Device".to_string());

        assert_eq!(device.get_status(), DeviceStatus::Disconnected);

        device.connect().await.unwrap();
        assert_eq!(device.get_status(), DeviceStatus::Connected);

        let test_data = b"Hello, World!";
        device.send(test_data).await.unwrap();

        let sent = device.pop_send_data().unwrap();
        assert_eq!(sent, test_data);

        device.disconnect().await.unwrap();
        assert_eq!(device.get_status(), DeviceStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_mock_device_echo() {
        let mut device = MockDevice::new("echo_test".to_string(), "Echo Device".to_string());

        device.connect().await.unwrap();

        device.send(b"ECHO").await.unwrap();

        let received = device.receive().await.unwrap();
        assert_eq!(received, b"ECHO");
    }

    #[tokio::test]
    async fn test_mock_device_not_connected_error() {
        let mut device = MockDevice::new("error_test".to_string(), "Error Device".to_string());

        let result = device.send(b"test").await;
        assert!(matches!(result, Err(DeviceError::NotConnected)));

        let result = device.receive().await;
        assert!(matches!(result, Err(DeviceError::NotConnected)));
    }
}