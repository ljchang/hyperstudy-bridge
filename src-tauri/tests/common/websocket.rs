//! Improved WebSocket test client
//!
//! Provides a WebSocket client with better timeout handling,
//! explicit cleanup, and proper error types.

use super::harness::{TestError, TestResult};
use futures_util::{SinkExt, StreamExt};
use hyperstudy_bridge::bridge::{BridgeCommand, BridgeResponse};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

/// Test WebSocket client with improved error handling
pub struct TestWebSocketClient {
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    sent_messages: Arc<RwLock<Vec<String>>>,
    received_messages: Arc<RwLock<Vec<BridgeResponse>>>,
    is_connected: Arc<AtomicBool>,
}

impl TestWebSocketClient {
    /// Connect to a WebSocket server with timeout
    ///
    /// # Example
    /// ```ignore
    /// let client = TestWebSocketClient::connect_with_timeout(
    ///     "ws://127.0.0.1:9000",
    ///     Duration::from_secs(5),
    /// ).await?;
    /// ```
    pub async fn connect_with_timeout(url: &str, timeout: Duration) -> TestResult<Self> {
        let connect_future = connect_async(url);

        let (ws_stream, _) = tokio::time::timeout(timeout, connect_future)
            .await
            .map_err(|_| TestError::Timeout(format!("WebSocket connection to {} timed out", url)))?
            .map_err(|e| TestError::WebSocket(format!("Failed to connect to {}: {}", url, e)))?;

        Ok(Self {
            ws_stream,
            sent_messages: Arc::new(RwLock::new(Vec::new())),
            received_messages: Arc::new(RwLock::new(Vec::new())),
            is_connected: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Connect without timeout (for backward compatibility)
    pub async fn connect(url: &str) -> TestResult<Self> {
        Self::connect_with_timeout(url, Duration::from_secs(30)).await
    }

    /// Send a command to the server
    pub async fn send_command(&mut self, command: BridgeCommand) -> TestResult<()> {
        if !self.is_connected() {
            return Err(TestError::WebSocket("Not connected".to_string()));
        }

        let json_str = serde_json::to_string(&command)
            .map_err(|e| TestError::WebSocket(format!("Failed to serialize command: {}", e)))?;

        self.sent_messages.write().await.push(json_str.clone());

        self.ws_stream
            .send(Message::Text(json_str.into()))
            .await
            .map_err(|e| TestError::WebSocket(format!("Failed to send message: {}", e)))?;

        Ok(())
    }

    /// Send a command and wait for a response
    pub async fn send_and_wait(
        &mut self,
        command: BridgeCommand,
        timeout: Duration,
    ) -> TestResult<BridgeResponse> {
        self.send_command(command).await?;
        self.wait_for_response(timeout).await
    }

    /// Wait for the next response with timeout
    pub async fn wait_for_response(&mut self, timeout: Duration) -> TestResult<BridgeResponse> {
        let response = tokio::time::timeout(timeout, self.receive_response())
            .await
            .map_err(|_| TestError::Timeout("Waiting for WebSocket response timed out".to_string()))??;

        response.ok_or_else(|| TestError::WebSocket("Connection closed while waiting for response".to_string()))
    }

    /// Receive the next response (no timeout)
    pub async fn receive_response(&mut self) -> TestResult<Option<BridgeResponse>> {
        if let Some(msg) = self.ws_stream.next().await {
            match msg.map_err(|e| TestError::WebSocket(format!("WebSocket error: {}", e)))? {
                Message::Text(text) => {
                    let text_str: &str = &text;
                    let response: BridgeResponse = serde_json::from_str(text_str)
                        .map_err(|e| TestError::WebSocket(format!("Failed to parse response: {}", e)))?;
                    self.received_messages.write().await.push(response.clone());
                    Ok(Some(response))
                }
                Message::Close(_) => {
                    self.is_connected.store(false, Ordering::Relaxed);
                    Ok(None)
                }
                Message::Ping(data) => {
                    // Auto-respond to pings
                    self.ws_stream.send(Message::Pong(data)).await.ok();
                    // Recurse to get next real message
                    Box::pin(self.receive_response()).await
                }
                _ => Ok(None),
            }
        } else {
            self.is_connected.store(false, Ordering::Relaxed);
            Ok(None)
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    /// Get all sent messages
    pub async fn get_sent_messages(&self) -> Vec<String> {
        self.sent_messages.read().await.clone()
    }

    /// Get all received messages
    pub async fn get_received_messages(&self) -> Vec<BridgeResponse> {
        self.received_messages.read().await.clone()
    }

    /// Clear message history
    pub async fn clear_messages(&mut self) {
        self.sent_messages.write().await.clear();
        self.received_messages.write().await.clear();
    }

    /// Close the connection explicitly
    pub async fn close(mut self) -> TestResult<()> {
        if self.is_connected() {
            self.ws_stream
                .send(Message::Close(None))
                .await
                .map_err(|e| TestError::WebSocket(format!("Failed to send close: {}", e)))?;
            self.is_connected.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Send raw text message
    pub async fn send_raw(&mut self, message: &str) -> TestResult<()> {
        self.ws_stream
            .send(Message::Text(message.to_string().into()))
            .await
            .map_err(|e| TestError::WebSocket(format!("Failed to send raw message: {}", e)))
    }

    /// Receive raw text message
    pub async fn receive_raw(&mut self, timeout: Duration) -> TestResult<Option<String>> {
        let msg = tokio::time::timeout(timeout, self.ws_stream.next())
            .await
            .map_err(|_| TestError::Timeout("Waiting for raw message timed out".to_string()))?;

        match msg {
            Some(Ok(Message::Text(text))) => Ok(Some(text.to_string())),
            Some(Ok(Message::Close(_))) => {
                self.is_connected.store(false, Ordering::Relaxed);
                Ok(None)
            }
            Some(Ok(_)) => Ok(None),
            Some(Err(e)) => Err(TestError::WebSocket(format!("WebSocket error: {}", e))),
            None => {
                self.is_connected.store(false, Ordering::Relaxed);
                Ok(None)
            }
        }
    }
}

/// Helper function to try connecting with retries
pub async fn connect_with_retry(
    url: &str,
    max_attempts: usize,
    retry_delay: Duration,
) -> TestResult<TestWebSocketClient> {
    let mut last_error = None;

    for attempt in 0..max_attempts {
        match TestWebSocketClient::connect_with_timeout(url, Duration::from_secs(5)).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_attempts - 1 {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        TestError::WebSocket(format!("Failed to connect to {} after {} attempts", url, max_attempts))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most WebSocket tests require a running server, so they're integration tests
    // These are just unit tests for the helper functions

    #[tokio::test]
    async fn test_connect_timeout_on_bad_url() {
        let result = TestWebSocketClient::connect_with_timeout(
            "ws://127.0.0.1:59999", // Unlikely to have a server
            Duration::from_millis(100),
        )
        .await;

        assert!(result.is_err());
    }
}
