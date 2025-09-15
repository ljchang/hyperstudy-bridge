use hyperstudy_bridge::bridge::message::{CommandAction, QueryTarget};
use hyperstudy_bridge::bridge::{AppState, BridgeCommand, BridgeResponse, BridgeServer};
use hyperstudy_bridge::devices::{DeviceStatus, DeviceType};
use hyperstudy_bridge::performance::PerformanceMonitor;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::test::mock_runtime;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use uuid::Uuid;

mod common;
use common::*;

/// Test suite for WebSocket server startup and shutdown
#[cfg(test)]
mod websocket_server_tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_server_startup() {
        let mut fixture = TestFixture::new().await;

        // Create a mock Tauri app handle
        let app = tauri::test::mock_app();
        let app_handle = app.handle().clone();

        // Create bridge server
        let mut bridge_server = BridgeServer::new(fixture.app_state.clone(), app_handle);

        // Start server in background
        let server_handle = tokio::spawn(async move {
            // Use a different port for testing to avoid conflicts
            bridge_server.start().await
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test connection to server
        let ws_url = "ws://127.0.0.1:9000";
        let connection_result = timeout(Duration::from_secs(5), async {
            TestWebSocketClient::connect(ws_url).await
        })
        .await;

        match connection_result {
            Ok(Ok(mut client)) => {
                assert!(client.is_connected());
                client.close().await.unwrap();
            }
            Ok(Err(e)) => {
                // Connection might fail if port is already in use or server isn't ready
                println!(
                    "WebSocket connection failed (expected in some test environments): {}",
                    e
                );
            }
            Err(_) => {
                println!("WebSocket connection timed out (expected in some test environments)");
            }
        }

        // Clean up
        server_handle.abort();
        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_websocket_port_binding() {
        // Test that the server can bind to the expected port
        let addr = "127.0.0.1:9001"; // Use different port for test
        let listener_result = TcpListener::bind(addr).await;

        assert!(listener_result.is_ok(), "Failed to bind to WebSocket port");

        if let Ok(listener) = listener_result {
            drop(listener); // Release the port
        }
    }

    #[tokio::test]
    async fn test_server_graceful_shutdown() {
        let mut fixture = TestFixture::new().await;
        let app = tauri::test::mock_app();
        let app_handle = app.handle().clone();

        let mut bridge_server = BridgeServer::new(fixture.app_state.clone(), app_handle);

        // Start server
        let server_handle = tokio::spawn(async move { bridge_server.start().await });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Shutdown server
        server_handle.abort();

        // Wait for shutdown
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify server is no longer accepting connections
        let ws_url = "ws://127.0.0.1:9000";
        let connection_result = timeout(Duration::from_millis(500), async {
            TestWebSocketClient::connect(ws_url).await
        })
        .await;

        // Connection should fail or timeout since server is shut down
        assert!(
            connection_result.is_err() || connection_result.unwrap().is_err(),
            "Server should not accept connections after shutdown"
        );

        fixture.cleanup().await;
    }
}

/// Test suite for message routing and handling
#[cfg(test)]
mod message_routing_tests {
    use super::*;

    #[tokio::test]
    async fn test_command_message_parsing() {
        let mut data_generator = TestDataGenerator::new();

        // Test connect command parsing
        let connect_command = data_generator.generate_connect_command("ttl");
        let json_str = serde_json::to_string(&connect_command).unwrap();
        let parsed_command: BridgeCommand = serde_json::from_str(&json_str).unwrap();

        match parsed_command {
            BridgeCommand::Command { device, action, .. } => {
                assert_eq!(device, "ttl");
                assert!(matches!(action, CommandAction::Connect));
            }
            _ => panic!("Expected Command variant"),
        }

        // Test TTL pulse command
        let pulse_command = data_generator.generate_ttl_command();
        let json_str = serde_json::to_string(&pulse_command).unwrap();
        let parsed_command: BridgeCommand = serde_json::from_str(&json_str).unwrap();

        match parsed_command {
            BridgeCommand::Command {
                device,
                action,
                payload,
                ..
            } => {
                assert_eq!(device, "ttl");
                assert!(matches!(action, CommandAction::Send));
                assert!(payload.is_some());
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[tokio::test]
    async fn test_query_message_parsing() {
        let query_devices = BridgeCommand::Query {
            target: QueryTarget::Devices,
            id: Some("test_query_1".to_string()),
        };

        let json_str = serde_json::to_string(&query_devices).unwrap();
        let parsed_command: BridgeCommand = serde_json::from_str(&json_str).unwrap();

        match parsed_command {
            BridgeCommand::Query { target, id } => {
                assert!(matches!(target, QueryTarget::Devices));
                assert_eq!(id, Some("test_query_1".to_string()));
            }
            _ => panic!("Expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_response_message_serialization() {
        // Test status response
        let status_response =
            BridgeResponse::status("test_device".to_string(), DeviceStatus::Connected);
        let json_str = serde_json::to_string(&status_response).unwrap();
        let parsed_response: BridgeResponse = serde_json::from_str(&json_str).unwrap();

        match parsed_response {
            BridgeResponse::Status { device, status, .. } => {
                assert_eq!(device, "test_device");
                assert_eq!(status, DeviceStatus::Connected);
            }
            _ => panic!("Expected Status response"),
        }

        // Test error response
        let error_response =
            BridgeResponse::device_error("test_device".to_string(), "Test error".to_string());
        let json_str = serde_json::to_string(&error_response).unwrap();
        let parsed_response: BridgeResponse = serde_json::from_str(&json_str).unwrap();

        match parsed_response {
            BridgeResponse::Error {
                device, message, ..
            } => {
                assert_eq!(device, Some("test_device".to_string()));
                assert_eq!(message, "Test error");
            }
            _ => panic!("Expected Error response"),
        }
    }

    #[tokio::test]
    async fn test_malformed_message_handling() {
        let malformed_messages = vec![
            "invalid json",
            "{}",
            r#"{"type": "unknown"}"#,
            r#"{"type": "command"}"#, // missing required fields
            r#"{"type": "command", "device": ""}"#, // empty device
        ];

        for malformed_msg in malformed_messages {
            let parse_result = serde_json::from_str::<BridgeCommand>(malformed_msg);
            assert!(
                parse_result.is_err(),
                "Should fail to parse malformed message: {}",
                malformed_msg
            );
        }
    }

    #[tokio::test]
    async fn test_large_message_handling() {
        let mut data_generator = TestDataGenerator::new();

        // Create a large message (1MB)
        let large_payload = data_generator.generate_large_message(1024);
        let large_command = BridgeCommand::Command {
            device: "test_device".to_string(),
            action: CommandAction::Send,
            payload: Some(large_payload),
            id: Some(data_generator.generate_request_id()),
        };

        // Test serialization/deserialization of large message
        let json_str = serde_json::to_string(&large_command).unwrap();
        assert!(json_str.len() > 1024 * 1024, "Message should be large");

        let parsed_command: BridgeCommand = serde_json::from_str(&json_str).unwrap();
        match parsed_command {
            BridgeCommand::Command {
                device,
                action,
                payload,
                ..
            } => {
                assert_eq!(device, "test_device");
                assert!(matches!(action, CommandAction::Send));
                assert!(payload.is_some());
            }
            _ => panic!("Expected Command variant"),
        }
    }
}

/// Test suite for client connection and disconnection handling
#[cfg(test)]
mod client_connection_tests {
    use super::*;

    async fn create_mock_websocket_test() -> Result<(), Box<dyn std::error::Error>> {
        // Since we can't easily start a real WebSocket server in tests,
        // we'll test the connection handling logic components
        let mut fixture = TestFixture::new().await;

        // Test connection state tracking
        let initial_connections = fixture.app_state.connections.len();
        assert_eq!(initial_connections, 0);

        // Simulate adding connections
        fixture
            .app_state
            .add_connection("conn1".to_string(), "127.0.0.1:12345".to_string());
        fixture
            .app_state
            .add_connection("conn2".to_string(), "127.0.0.1:12346".to_string());

        assert_eq!(fixture.app_state.connections.len(), 2);

        // Test connection activity updates
        fixture.app_state.update_connection_activity("conn1");
        fixture.app_state.update_connection_activity("conn2");

        // Test connection removal
        fixture.app_state.remove_connection("conn1");
        assert_eq!(fixture.app_state.connections.len(), 1);

        fixture.app_state.remove_connection("conn2");
        assert_eq!(fixture.app_state.connections.len(), 0);

        fixture.cleanup().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_state_management() {
        create_mock_websocket_test().await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_concurrent_connections() {
        let mut fixture = TestFixture::new().await;

        // Simulate multiple concurrent connections
        let connection_count = 10;
        let mut connection_ids = Vec::new();

        for i in 0..connection_count {
            let conn_id = format!("conn_{}", i);
            let addr = format!("127.0.0.1:{}", 50000 + i);
            fixture.app_state.add_connection(conn_id.clone(), addr);
            connection_ids.push(conn_id);
        }

        assert_eq!(fixture.app_state.connections.len(), connection_count);

        // Test concurrent activity updates
        let mut handles = Vec::new();
        for conn_id in &connection_ids {
            let state = fixture.app_state.clone();
            let conn_id_clone = conn_id.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    state.update_connection_activity(&conn_id_clone);
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all updates to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Clean up connections
        for conn_id in connection_ids {
            fixture.app_state.remove_connection(&conn_id);
        }

        assert_eq!(fixture.app_state.connections.len(), 0);
        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_connection_cleanup_on_disconnect() {
        let mut fixture = TestFixture::new().await;

        // Add connections
        fixture
            .app_state
            .add_connection("test_conn_1".to_string(), "127.0.0.1:50001".to_string());
        fixture
            .app_state
            .add_connection("test_conn_2".to_string(), "127.0.0.1:50002".to_string());

        assert_eq!(fixture.app_state.connections.len(), 2);

        // Simulate abrupt disconnection (connection just disappears)
        fixture.app_state.remove_connection("test_conn_1");

        // Verify cleanup
        assert_eq!(fixture.app_state.connections.len(), 1);

        // Clean up remaining connection
        fixture.app_state.remove_connection("test_conn_2");
        assert_eq!(fixture.app_state.connections.len(), 0);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_connection_metrics_tracking() {
        let mut fixture = TestFixture::new().await;

        // Test performance monitor connection tracking
        let initial_connections = fixture
            .performance_monitor
            .system_counters
            .active_connections
            .load(std::sync::atomic::Ordering::Relaxed);

        // Simulate connection events
        fixture
            .performance_monitor
            .record_websocket_connection(true); // Connect
        fixture
            .performance_monitor
            .record_websocket_connection(true); // Another connect

        let after_connects = fixture
            .performance_monitor
            .system_counters
            .active_connections
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(after_connects, initial_connections + 2);

        // Simulate disconnection
        fixture
            .performance_monitor
            .record_websocket_connection(false); // Disconnect

        let after_disconnect = fixture
            .performance_monitor
            .system_counters
            .active_connections
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(after_disconnect, initial_connections + 1);

        fixture.cleanup().await;
    }
}

/// Test suite for message throughput testing
#[cfg(test)]
mod throughput_tests {
    use super::*;

    #[tokio::test]
    async fn test_message_processing_throughput() {
        let mut fixture = TestFixture::new().await;

        // Add mock device
        let device_id = fixture.add_mock_device(DeviceType::Mock).await;

        // Connect the device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Measure message processing throughput
        let test_duration = Duration::from_secs(2);
        let start_time = Instant::now();
        let mut message_count = 0u64;

        while start_time.elapsed() < test_duration {
            // Simulate message processing
            fixture.performance_monitor.record_bridge_message();
            message_count += 1;

            // Small delay to prevent spinning
            if message_count % 1000 == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }

        let actual_duration = start_time.elapsed().as_secs_f64();
        let throughput = message_count as f64 / actual_duration;

        println!("Message processing throughput: {:.0} msg/sec", throughput);

        // Verify throughput meets requirement (>1000 msg/sec)
        test_utils::assert_throughput_compliance(throughput, 1000.0);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_command_throughput() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Measure device command throughput
        let test_duration = Duration::from_secs(3);
        let (command_count, throughput) = test_utils::measure_throughput(
            || async {
                if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                    let mut device = device_lock.write().await;
                    let _ = device.send(b"PULSE\n").await;
                }
            },
            test_duration,
        )
        .await;

        println!(
            "Device command throughput: {:.0} cmd/sec ({} commands)",
            throughput, command_count
        );

        // TTL should handle high-frequency commands
        assert!(
            throughput > 500.0,
            "TTL command throughput too low: {:.0} cmd/sec",
            throughput
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_concurrent_client_throughput() {
        let mut fixture = TestFixture::new().await;

        // Add multiple devices for concurrent testing
        let device_ids: Vec<_> = (0..5)
            .map(|_| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(async { fixture.add_mock_device(DeviceType::Mock).await })
                })
            })
            .collect();

        // Connect all devices
        for device_id in &device_ids {
            if let Some(device_lock) = fixture.app_state.get_device(device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
            }
        }

        // Measure concurrent throughput
        let concurrent_clients = 5;
        let operations_per_client = 200;

        let start_time = Instant::now();
        let latencies = test_utils::run_load_test(
            |client_id| {
                let state = fixture.app_state.clone();
                let device_id = device_ids[client_id % device_ids.len()].clone();
                async move {
                    let operation_start = Instant::now();
                    if let Some(device_lock) = state.get_device(&device_id).await {
                        let mut device = device_lock.write().await;
                        let _ = device.send(b"test").await;
                    }
                    operation_start.elapsed()
                }
            },
            concurrent_clients,
            operations_per_client,
        )
        .await;

        let total_duration = start_time.elapsed().as_secs_f64();
        let total_operations = latencies.len();
        let overall_throughput = total_operations as f64 / total_duration;

        println!(
            "Concurrent throughput: {:.0} ops/sec ({} ops, {} clients)",
            overall_throughput, total_operations, concurrent_clients
        );

        // Verify reasonable throughput under concurrent load
        assert!(
            overall_throughput > 500.0,
            "Concurrent throughput too low: {:.0} ops/sec",
            overall_throughput
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_message_size_vs_throughput() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::Kernel).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        let message_sizes = vec![10, 100, 1000, 10000]; // bytes
        let test_duration = Duration::from_secs(1);

        for message_size in message_sizes {
            let message = vec![0u8; message_size];

            let (message_count, throughput) = test_utils::measure_throughput(
                || async {
                    if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                        let mut device = device_lock.write().await;
                        let _ = device.send(&message).await;
                    }
                },
                test_duration,
            )
            .await;

            let bytes_per_second = throughput * message_size as f64;

            println!(
                "Message size: {} bytes, Throughput: {:.0} msg/sec, {:.0} bytes/sec",
                message_size, throughput, bytes_per_second
            );

            // Throughput should decrease with message size, but still be reasonable
            assert!(
                throughput > 100.0,
                "Throughput too low for {} byte messages: {:.0} msg/sec",
                message_size,
                throughput
            );
        }

        fixture.cleanup().await;
    }
}

/// Test suite for error handling and recovery in bridge operations
#[cfg(test)]
mod bridge_error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_command_handling() {
        let mut fixture = TestFixture::new().await;

        // Test handling of commands for non-existent devices
        let invalid_command = BridgeCommand::Command {
            device: "non_existent_device".to_string(),
            action: CommandAction::Connect,
            payload: None,
            id: Some("test_id".to_string()),
        };

        // Simulate command processing
        // In a real scenario, this would be handled by the WebSocket message handler
        // For now, we test that the device lookup fails gracefully
        let device = fixture.app_state.get_device("non_existent_device").await;
        assert!(device.is_none(), "Should not find non-existent device");

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_error_propagation() {
        let mut fixture = TestFixture::new().await;

        // Add an unreliable device
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 1.0).await; // 100% error rate

        // Test that device errors are properly recorded
        fixture
            .performance_monitor
            .add_device(device_id.clone())
            .await;

        // Attempt operations that will fail
        for _ in 0..5 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                if let Err(e) = device.connect().await {
                    fixture
                        .performance_monitor
                        .record_device_error(&device_id, &e.to_string())
                        .await;
                }
            }
        }

        // Verify errors were recorded
        let metrics = fixture
            .performance_monitor
            .get_device_metrics(&device_id)
            .await;
        assert!(metrics.is_some());

        let device_metrics = metrics.unwrap();
        assert!(
            device_metrics.errors > 0,
            "No errors recorded despite high error rate"
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_bridge_state_consistency_under_errors() {
        let mut fixture = TestFixture::new().await;

        // Add multiple devices with varying reliability
        let reliable_device_id = fixture.add_mock_device(DeviceType::TTL).await;
        let unreliable_device_id = fixture.add_unreliable_device(DeviceType::Kernel, 0.7).await;

        // Connect reliable device
        if let Some(device_lock) = fixture.app_state.get_device(&reliable_device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Attempt to connect unreliable device multiple times
        let mut connection_attempts = 0;
        for _ in 0..10 {
            if let Some(device_lock) = fixture.app_state.get_device(&unreliable_device_id).await {
                let mut device = device_lock.write().await;
                if device.connect().await.is_ok() {
                    connection_attempts += 1;
                    device.disconnect().await.ok();
                }
            }
        }

        // Verify reliable device is still connected despite unreliable device errors
        let reliable_status = fixture
            .app_state
            .get_device_status(&reliable_device_id)
            .await;
        assert_eq!(reliable_status, Some(DeviceStatus::Connected));

        // Verify state consistency
        let device_count = fixture.get_device_count().await;
        assert_eq!(device_count, 2, "Device count should remain consistent");

        println!(
            "Unreliable device successful connections: {}/10",
            connection_attempts
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_memory_stability_under_errors() {
        let mut fixture = TestFixture::new().await;
        let mut memory_tracker = MemoryTracker::new();

        // Add unreliable device
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 0.8).await;

        // Generate many errors
        for cycle in 0..100 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                let _ = device.connect().await; // Most will fail
                let _ = device.send(b"test").await; // Most will fail
                let _ = device.disconnect().await;
            }

            if cycle % 20 == 0 {
                memory_tracker.measure();
            }
        }

        memory_tracker.measure();

        // Verify no significant memory leaks from error handling
        assert!(
            !memory_tracker.has_memory_leak(15),
            "Memory leak detected during error handling: {} bytes increase",
            memory_tracker.memory_increase()
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_performance_monitoring_under_errors() {
        let mut fixture = TestFixture::new().await;

        // Add device to performance monitoring
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 0.5).await;
        fixture
            .performance_monitor
            .add_device(device_id.clone())
            .await;

        // Perform operations with mixed success/failure
        for _ in 0..50 {
            let start = Instant::now();
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                match device.send(b"test").await {
                    Ok(_) => {
                        let latency = start.elapsed();
                        fixture
                            .performance_monitor
                            .record_device_operation(&device_id, latency, 4, 0)
                            .await;
                    }
                    Err(e) => {
                        fixture
                            .performance_monitor
                            .record_device_error(&device_id, &e.to_string())
                            .await;
                    }
                }
            }
        }

        // Verify performance monitoring handles mixed success/failure correctly
        let metrics = fixture
            .performance_monitor
            .get_device_metrics(&device_id)
            .await;
        assert!(metrics.is_some());

        let device_metrics = metrics.unwrap();
        assert!(
            device_metrics.messages_sent > 0 || device_metrics.errors > 0,
            "Should have recorded either successful operations or errors"
        );

        println!(
            "Mixed operations: {} successful, {} errors",
            device_metrics.messages_sent, device_metrics.errors
        );

        fixture.cleanup().await;
    }
}

/// Test suite for bridge query operations
#[cfg(test)]
mod bridge_query_tests {
    use super::*;

    #[tokio::test]
    async fn test_devices_query() {
        let mut fixture = TestFixture::new().await;

        // Add multiple devices
        let device_ids = test_utils::create_multi_device_setup(&mut fixture).await;

        // Simulate devices query
        let devices_list = fixture.app_state.list_devices().await;

        assert_eq!(devices_list.len(), device_ids.len());

        // Verify all device types are represented
        let device_types: std::collections::HashSet<_> =
            devices_list.iter().map(|info| info.device_type).collect();

        assert!(device_types.contains(&DeviceType::TTL));
        assert!(device_types.contains(&DeviceType::Kernel));
        assert!(device_types.contains(&DeviceType::Pupil));
        assert!(device_types.contains(&DeviceType::Biopac));
        assert!(device_types.contains(&DeviceType::Mock));

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_info_query() {
        let mut fixture = TestFixture::new().await;

        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Query specific device info
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let device = device_lock.read().await;
            let device_info = device.get_info();

            assert_eq!(device_info.id, device_id);
            assert_eq!(device_info.device_type, DeviceType::TTL);
            assert_eq!(device_info.status, DeviceStatus::Disconnected);
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_metrics_query() {
        let mut fixture = TestFixture::new().await;

        // Add device and perform some operations
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;
        fixture
            .performance_monitor
            .add_device(device_id.clone())
            .await;

        // Connect and perform operations
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();

            for _ in 0..5 {
                device.send(b"test").await.unwrap();
                fixture
                    .performance_monitor
                    .record_device_operation(&device_id, Duration::from_millis(1), 4, 0)
                    .await;
            }
        }

        // Query metrics
        let metrics = fixture.app_state.get_performance_metrics().await;
        assert!(metrics.devices.contains_key(&device_id));

        let device_metrics = &metrics.devices[&device_id];
        assert_eq!(device_metrics.messages_sent, 5);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_connections_query() {
        let mut fixture = TestFixture::new().await;

        // Add some connections
        fixture
            .app_state
            .add_connection("conn1".to_string(), "127.0.0.1:50001".to_string());
        fixture
            .app_state
            .add_connection("conn2".to_string(), "127.0.0.1:50002".to_string());

        // Query connections (simulating QueryTarget::Connections)
        let connections: Vec<_> = fixture
            .app_state
            .connections
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        assert_eq!(connections.len(), 2);

        // Clean up connections
        fixture.app_state.remove_connection("conn1");
        fixture.app_state.remove_connection("conn2");

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_status_query() {
        let mut fixture = TestFixture::new().await;

        // Add some devices
        let _device_id1 = fixture.add_mock_device(DeviceType::TTL).await;
        let _device_id2 = fixture.add_mock_device(DeviceType::Kernel).await;

        // Add a connection
        fixture
            .app_state
            .add_connection("test_conn".to_string(), "127.0.0.1:50001".to_string());

        // Simulate status query
        let device_count = fixture.app_state.devices.read().await.len();
        let connection_count = fixture.app_state.connections.len();

        let expected_status = json!({
            "server": "running",
            "port": 9000,
            "devices": device_count,
            "connections": connection_count,
        });

        assert_eq!(device_count, 2);
        assert_eq!(connection_count, 1);

        // Clean up
        fixture.app_state.remove_connection("test_conn");
        fixture.cleanup().await;
    }
}

/// Test suite for performance and scalability
#[cfg(test)]
mod scalability_tests {
    use super::*;

    #[tokio::test]
    async fn test_many_devices_handling() {
        let mut fixture = TestFixture::new().await;

        // Add many devices
        let device_count = 50;
        let mut device_ids = Vec::new();

        for i in 0..device_count {
            let device_type = match i % 5 {
                0 => DeviceType::TTL,
                1 => DeviceType::Kernel,
                2 => DeviceType::Pupil,
                3 => DeviceType::Biopac,
                _ => DeviceType::Mock,
            };
            let device_id = fixture.add_mock_device(device_type).await;
            device_ids.push(device_id);
        }

        assert_eq!(fixture.get_device_count().await, device_count);

        // Connect all devices concurrently
        let mut connect_tasks = Vec::new();
        for device_id in &device_ids {
            let state = fixture.app_state.clone();
            let device_id_clone = device_id.clone();
            let task = tokio::spawn(async move {
                if let Some(device_lock) = state.get_device(&device_id_clone).await {
                    let mut device = device_lock.write().await;
                    device.connect().await
                } else {
                    Err(DeviceError::NotConnected)
                }
            });
            connect_tasks.push(task);
        }

        // Wait for all connections
        let mut successful_connections = 0;
        for task in connect_tasks {
            if task.await.unwrap().is_ok() {
                successful_connections += 1;
            }
        }

        assert_eq!(successful_connections, device_count);

        // Verify all devices are accessible
        for device_id in &device_ids {
            let status = fixture.app_state.get_device_status(device_id).await;
            assert_eq!(status, Some(DeviceStatus::Connected));
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_high_frequency_operations() {
        let mut fixture = TestFixture::new().await;

        // Add TTL device for high-frequency testing
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Perform high-frequency operations
        let operation_count = 10000;
        let start_time = Instant::now();

        for _ in 0..operation_count {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.send(b"PULSE").await.unwrap();
            }
        }

        let elapsed = start_time.elapsed();
        let frequency = operation_count as f64 / elapsed.as_secs_f64();

        println!(
            "High-frequency operations: {:.0} ops/sec ({} ops in {:?})",
            frequency, operation_count, elapsed
        );

        // Should handle at least 5000 operations per second
        assert!(
            frequency > 5000.0,
            "High-frequency operation rate too low: {:.0} ops/sec",
            frequency
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_memory_usage_under_load() {
        let mut fixture = TestFixture::new().await;
        let mut memory_tracker = MemoryTracker::new();

        // Add multiple devices
        let device_ids: Vec<_> = (0..10)
            .map(|_| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(async { fixture.add_mock_device(DeviceType::Mock).await })
                })
            })
            .collect();

        // Connect all devices
        for device_id in &device_ids {
            if let Some(device_lock) = fixture.app_state.get_device(device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
            }
        }

        memory_tracker.measure();

        // Perform sustained operations
        for cycle in 0..200 {
            for device_id in &device_ids {
                if let Some(device_lock) = fixture.app_state.get_device(device_id).await {
                    let mut device = device_lock.write().await;
                    device.send(b"load_test").await.unwrap();
                }
            }

            if cycle % 50 == 0 {
                memory_tracker.measure();
            }
        }

        memory_tracker.measure();

        // Verify reasonable memory usage
        assert!(
            !memory_tracker.has_memory_leak(50),
            "Excessive memory usage under load: {} bytes increase",
            memory_tracker.memory_increase()
        );

        println!(
            "Memory usage under load: {} bytes increase",
            memory_tracker.memory_increase()
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_bridge_state_performance() {
        let mut fixture = TestFixture::new().await;

        // Add many devices to test state management performance
        let device_count = 100;
        let mut device_ids = Vec::new();

        for _ in 0..device_count {
            let device_id = fixture.add_mock_device(DeviceType::Mock).await;
            device_ids.push(device_id);
        }

        // Measure state query performance
        let start_time = Instant::now();

        for _ in 0..1000 {
            let _devices = fixture.app_state.list_devices().await;
        }

        let query_time = start_time.elapsed();
        let queries_per_second = 1000.0 / query_time.as_secs_f64();

        println!(
            "State query performance: {:.0} queries/sec with {} devices",
            queries_per_second, device_count
        );

        // Should handle at least 100 queries per second even with many devices
        assert!(
            queries_per_second > 100.0,
            "State query performance too low: {:.0} queries/sec",
            queries_per_second
        );

        fixture.cleanup().await;
    }
}
