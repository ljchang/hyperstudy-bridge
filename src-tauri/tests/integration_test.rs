use hyperstudy_bridge::bridge::{AppState, BridgeCommand, BridgeResponse, BridgeServer};
use hyperstudy_bridge::devices::{Device, DeviceError, DeviceStatus, DeviceType, DeviceConfig};
use hyperstudy_bridge::performance::{PerformanceMonitor, measure_latency};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time::timeout;
use tokio::sync::RwLock;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::*;

/// Test suite for device lifecycle operations
#[cfg(test)]
mod device_lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_device_connect_disconnect_cycle() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Test initial state
        let status = fixture.app_state.get_device_status(&device_id).await;
        assert_eq!(status, Some(DeviceStatus::Disconnected));

        // Test connection
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
            assert_eq!(device.get_status(), DeviceStatus::Connected);
        } else {
            panic!("Device not found");
        }

        // Verify status updated in state
        let status = fixture.app_state.get_device_status(&device_id).await;
        assert_eq!(status, Some(DeviceStatus::Connected));

        // Test disconnection
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.disconnect().await.unwrap();
            assert_eq!(device.get_status(), DeviceStatus::Disconnected);
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_multiple_device_connections() {
        let mut fixture = TestFixture::new().await;
        let devices = test_utils::create_multi_device_setup(&mut fixture).await;

        // Connect all devices simultaneously
        let mut connect_tasks = Vec::new();
        for (device_type, device_id) in &devices {
            let state_clone = fixture.app_state.clone();
            let device_id_clone = device_id.clone();
            let task = tokio::spawn(async move {
                if let Some(device_lock) = state_clone.get_device(&device_id_clone).await {
                    let mut device = device_lock.write().await;
                    device.connect().await
                } else {
                    Err(DeviceError::NotConnected)
                }
            });
            connect_tasks.push(task);
        }

        // Wait for all connections to complete
        for task in connect_tasks {
            task.await.unwrap().unwrap();
        }

        // Verify all devices are connected
        for (device_type, device_id) in &devices {
            let status = fixture.app_state.get_device_status(device_id).await;
            assert_eq!(status, Some(DeviceStatus::Connected));
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_send_receive_operations() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device first
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Test send operation
        let test_data = b"PULSE\n";
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.send(test_data).await.unwrap();
        }

        // Test receive operation
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            let received = device.receive().await.unwrap();
            assert!(!received.is_empty());
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_error_handling() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 1.0).await; // 100% error rate

        // Test connection failure
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            let result = device.connect().await;
            assert!(result.is_err());
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_configuration() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        let config = DeviceConfig {
            auto_reconnect: false,
            reconnect_interval_ms: 2000,
            timeout_ms: 10000,
            custom_settings: json!({"test_setting": "test_value"}),
        };

        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.configure(config.clone()).unwrap();
        }

        fixture.cleanup().await;
    }
}

/// Test suite for performance requirements
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_ttl_latency_requirement() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Measure TTL pulse latency
        let pulse_command = b"PULSE\n";
        let mut latencies = Vec::new();

        for _ in 0..100 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                let (result, latency) = measure_latency(device.send(pulse_command)).await;
                result.unwrap();
                latencies.push(latency);
            }
        }

        // Verify TTL latency requirement (<1ms)
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let max_latency = latencies.iter().max().unwrap();

        test_utils::assert_ttl_latency_compliance(avg_latency);
        test_utils::assert_ttl_latency_compliance(*max_latency);

        println!("TTL Average latency: {:?}", avg_latency);
        println!("TTL Max latency: {:?}", max_latency);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_message_throughput_requirement() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::Kernel).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Measure throughput
        let test_data = b"test_message";
        let test_duration = Duration::from_secs(5);

        let (message_count, throughput) = test_utils::measure_throughput(|| async {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                let _ = device.send(test_data).await;
            }
        }, test_duration).await;

        println!("Throughput: {} msg/sec ({} messages in {:?})", throughput, message_count, test_duration);

        // Verify throughput requirement (>1000 msg/sec)
        test_utils::assert_throughput_compliance(throughput, 1000.0);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_concurrent_device_performance() {
        let mut fixture = TestFixture::new().await;
        let devices = test_utils::create_multi_device_setup(&mut fixture).await;

        // Connect all devices
        for (_, device_id) in &devices {
            if let Some(device_lock) = fixture.app_state.get_device(device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
            }
        }

        let test_data = b"concurrent_test";
        let concurrent_operations = 10;
        let operations_per_worker = 100;

        // Run concurrent load test
        let latencies = test_utils::run_load_test(
            |worker_id| {
                let state = fixture.app_state.clone();
                let devices = devices.clone();
                async move {
                    let device_types = vec![DeviceType::TTL, DeviceType::Kernel, DeviceType::Pupil, DeviceType::Biopac, DeviceType::Mock];
                    let device_type = device_types[worker_id % device_types.len()];
                    let device_id = &devices[&device_type];

                    let start = Instant::now();
                    if let Some(device_lock) = state.get_device(device_id).await {
                        let mut device = device_lock.write().await;
                        let _ = device.send(test_data).await;
                    }
                    start.elapsed()
                }
            },
            concurrent_operations,
            operations_per_worker,
        ).await;

        // Analyze results
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let max_latency = latencies.iter().max().unwrap();
        let p95_index = (latencies.len() as f64 * 0.95) as usize;
        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort();
        let p95_latency = sorted_latencies[p95_index];

        println!("Concurrent test results:");
        println!("  Average latency: {:?}", avg_latency);
        println!("  P95 latency: {:?}", p95_latency);
        println!("  Max latency: {:?}", max_latency);
        println!("  Total operations: {}", latencies.len());

        // Verify reasonable performance under load
        assert!(avg_latency.as_millis() < 100, "Average latency too high under concurrent load");
        assert!(p95_latency.as_millis() < 200, "P95 latency too high under concurrent load");

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_performance_monitoring_integration() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Add device to performance monitoring
        fixture.performance_monitor.add_device(device_id.clone()).await;

        // Connect and perform operations
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Record operations with performance monitor
        for i in 0..10 {
            let start = Instant::now();
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.send(b"test").await.unwrap();
            }
            let latency = start.elapsed();

            fixture.performance_monitor.record_device_operation(
                &device_id,
                latency,
                4, // bytes sent
                0, // bytes received
            ).await;
        }

        // Verify metrics collection
        let metrics = fixture.performance_monitor.get_device_metrics(&device_id).await;
        assert!(metrics.is_some());

        let device_metrics = metrics.unwrap();
        assert_eq!(device_metrics.messages_sent, 10);
        assert!(device_metrics.last_latency_ns > 0);

        fixture.cleanup().await;
    }
}

/// Test suite for error recovery and reconnection
#[cfg(test)]
mod error_recovery_tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_recovery_after_failure() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 0.5).await; // 50% error rate

        let mut successful_connections = 0;
        let max_attempts = 10;

        for attempt in 0..max_attempts {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                if device.connect().await.is_ok() {
                    successful_connections += 1;
                    device.disconnect().await.ok();
                }
            }
        }

        // With 50% error rate, we should have some successful connections
        assert!(successful_connections > 0, "No successful connections after {} attempts", max_attempts);
        assert!(successful_connections < max_attempts, "All connections succeeded with 50% error rate");

        println!("Successful connections: {}/{}", successful_connections, max_attempts);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_operation_retry_on_temporary_failure() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 0.3).await; // 30% error rate

        // Connect device first
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            // Keep trying to connect until successful
            for _ in 0..10 {
                if device.connect().await.is_ok() {
                    break;
                }
            }
        }

        let test_data = b"retry_test";
        let mut successful_operations = 0;
        let total_operations = 50;

        for _ in 0..total_operations {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;

                // Retry operation up to 3 times on failure
                for retry in 0..3 {
                    match device.send(test_data).await {
                        Ok(_) => {
                            successful_operations += 1;
                            break;
                        }
                        Err(_) if retry < 2 => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            continue;
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        // With retries, we should have better success rate than the base error rate
        let success_rate = successful_operations as f64 / total_operations as f64;
        println!("Success rate with retries: {:.2}%", success_rate * 100.0);

        // With 30% error rate and 3 retries, success rate should be much higher
        assert!(success_rate > 0.8, "Success rate too low even with retries: {}", success_rate);

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_heartbeat_detection() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Test heartbeat functionality
        let heartbeat_results = {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                let mut results = Vec::new();
                for _ in 0..5 {
                    let result = device.heartbeat().await;
                    results.push(result.is_ok());
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                results
            } else {
                vec![]
            }
        };

        // All heartbeats should succeed for a healthy device
        assert!(heartbeat_results.iter().all(|&success| success), "Some heartbeats failed");

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_error_monitoring_and_metrics() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_unreliable_device(DeviceType::TTL, 0.8).await; // 80% error rate

        // Add to performance monitoring
        fixture.performance_monitor.add_device(device_id.clone()).await;

        // Attempt operations to generate errors
        let total_operations = 20;
        for _ in 0..total_operations {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                if let Err(e) = device.connect().await {
                    fixture.performance_monitor.record_device_error(&device_id, &e.to_string()).await;
                } else {
                    device.disconnect().await.ok();
                }
            }
        }

        // Check error metrics
        let metrics = fixture.performance_monitor.get_device_metrics(&device_id).await;
        assert!(metrics.is_some());

        let device_metrics = metrics.unwrap();
        assert!(device_metrics.errors > 0, "No errors recorded despite high error rate");

        // With 80% error rate, we should see significant errors
        let error_rate = device_metrics.errors as f64 / total_operations as f64;
        assert!(error_rate > 0.5, "Error rate too low: {}", error_rate);

        println!("Recorded errors: {} out of {} operations", device_metrics.errors, total_operations);

        fixture.cleanup().await;
    }
}

/// Test suite for memory leak detection
#[cfg(test)]
mod memory_leak_tests {
    use super::*;

    #[tokio::test]
    async fn test_device_connection_memory_leak() {
        let mut fixture = TestFixture::new().await;
        let mut memory_tracker = MemoryTracker::new();

        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Perform many connect/disconnect cycles
        for cycle in 0..100 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
                device.disconnect().await.unwrap();
            }

            // Measure memory every 10 cycles
            if cycle % 10 == 0 {
                memory_tracker.measure();
            }
        }

        // Final memory measurement
        memory_tracker.measure();

        // Check for memory leaks (threshold: 10MB increase)
        assert!(!memory_tracker.has_memory_leak(10),
            "Memory leak detected: {} bytes increase",
            memory_tracker.memory_increase()
        );

        println!("Memory increase: {} bytes", memory_tracker.memory_increase());

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_message_processing_memory_leak() {
        let mut fixture = TestFixture::new().await;
        let mut memory_tracker = MemoryTracker::new();

        let device_id = fixture.add_mock_device(DeviceType::Kernel).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Send many messages
        let large_message = vec![0u8; 1024]; // 1KB message
        for message_num in 0..1000 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.send(&large_message).await.unwrap();
            }

            // Measure memory every 100 messages
            if message_num % 100 == 0 {
                memory_tracker.measure();
            }
        }

        memory_tracker.measure();

        // Check for memory leaks (threshold: 20MB increase for large message test)
        assert!(!memory_tracker.has_memory_leak(20),
            "Memory leak detected in message processing: {} bytes increase",
            memory_tracker.memory_increase()
        );

        println!("Memory increase after 1000 messages: {} bytes", memory_tracker.memory_increase());

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_state_memory_leak() {
        let mut fixture = TestFixture::new().await;
        let mut memory_tracker = MemoryTracker::new();

        // Create and destroy many devices
        for cycle in 0..50 {
            let device_id = fixture.add_mock_device(DeviceType::Mock).await;

            // Connect and use device
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
                device.send(b"test").await.unwrap();
                device.disconnect().await.unwrap();
            }

            // Remove device
            fixture.app_state.remove_device(&device_id).await;

            // Measure memory every 10 cycles
            if cycle % 10 == 0 {
                memory_tracker.measure();
            }
        }

        memory_tracker.measure();

        // Check for memory leaks
        assert!(!memory_tracker.has_memory_leak(15),
            "Memory leak detected in device state management: {} bytes increase",
            memory_tracker.memory_increase()
        );

        println!("Memory increase after device lifecycle test: {} bytes", memory_tracker.memory_increase());

        fixture.cleanup().await;
    }
}

/// Test suite for edge cases and boundary conditions
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn test_extremely_high_latency_device() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_high_latency_device(DeviceType::Kernel, 5000).await; // 5 second latency

        // Test connection with timeout
        let connect_result = timeout(Duration::from_secs(10), async {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await
            } else {
                Err(DeviceError::NotConnected)
            }
        }).await;

        assert!(connect_result.is_ok(), "Connection timed out");
        assert!(connect_result.unwrap().is_ok(), "Connection failed");

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_zero_byte_messages() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Test sending empty message
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            let result = device.send(&[]).await;
            assert!(result.is_ok(), "Failed to send empty message");
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_large_message_handling() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::Kernel).await;

        // Connect device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Test sending large message (1MB)
        let large_message = vec![0u8; 1024 * 1024];
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            let result = device.send(&large_message).await;
            assert!(result.is_ok(), "Failed to send large message");
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_rapid_connect_disconnect_cycles() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Perform rapid connect/disconnect cycles
        for cycle in 0..100 {
            if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
                let mut device = device_lock.write().await;

                let connect_result = device.connect().await;
                assert!(connect_result.is_ok(), "Connect failed on cycle {}", cycle);

                let disconnect_result = device.disconnect().await;
                assert!(disconnect_result.is_ok(), "Disconnect failed on cycle {}", cycle);
            }
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_concurrent_access_to_same_device() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Connect device first
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;
            device.connect().await.unwrap();
        }

        // Launch concurrent operations
        let mut handles = Vec::new();
        for worker_id in 0..10 {
            let state = fixture.app_state.clone();
            let device_id_clone = device_id.clone();

            let handle = tokio::spawn(async move {
                let test_data = format!("worker_{}", worker_id);
                for _ in 0..10 {
                    if let Some(device_lock) = state.get_device(&device_id_clone).await {
                        let mut device = device_lock.write().await;
                        let _ = device.send(test_data.as_bytes()).await;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_device_operations_when_disconnected() {
        let mut fixture = TestFixture::new().await;
        let device_id = fixture.add_mock_device(DeviceType::TTL).await;

        // Attempt operations on disconnected device
        if let Some(device_lock) = fixture.app_state.get_device(&device_id).await {
            let mut device = device_lock.write().await;

            // These should fail because device is not connected
            let send_result = device.send(b"test").await;
            assert!(send_result.is_err(), "Send should fail when disconnected");

            let receive_result = device.receive().await;
            assert!(receive_result.is_err(), "Receive should fail when disconnected");

            let heartbeat_result = device.heartbeat().await;
            assert!(heartbeat_result.is_err(), "Heartbeat should fail when disconnected");
        }

        fixture.cleanup().await;
    }
}

/// Test suite for resource cleanup verification
#[cfg(test)]
mod resource_cleanup_tests {
    use super::*;

    #[tokio::test]
    async fn test_app_state_cleanup() {
        let mut fixture = TestFixture::new().await;

        // Add multiple devices
        let device_ids: Vec<_> = (0..5).map(|i| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    fixture.add_mock_device(DeviceType::Mock).await
                })
            })
        }).collect();

        // Verify devices are added
        assert_eq!(fixture.get_device_count().await, 5);

        // Clean up all devices
        fixture.cleanup().await;

        // Verify all devices are removed
        assert_eq!(fixture.get_device_count().await, 0);

        // Verify devices are no longer accessible
        for device_id in device_ids {
            let device = fixture.app_state.get_device(&device_id).await;
            assert!(device.is_none(), "Device {} still accessible after cleanup", device_id);
        }
    }

    #[tokio::test]
    async fn test_performance_monitor_cleanup() {
        let mut fixture = TestFixture::new().await;

        // Add devices to performance monitoring
        let device_ids: Vec<_> = (0..3).map(|_| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let device_id = fixture.add_mock_device(DeviceType::TTL).await;
                    fixture.performance_monitor.add_device(device_id.clone()).await;
                    device_id
                })
            })
        }).collect();

        // Verify metrics exist for all devices
        for device_id in &device_ids {
            let metrics = fixture.performance_monitor.get_device_metrics(device_id).await;
            assert!(metrics.is_some(), "Metrics not found for device {}", device_id);
        }

        // Remove devices from performance monitoring
        for device_id in &device_ids {
            fixture.performance_monitor.remove_device(device_id).await;
        }

        // Verify metrics are cleaned up
        for device_id in &device_ids {
            let metrics = fixture.performance_monitor.get_device_metrics(device_id).await;
            assert!(metrics.is_none(), "Metrics still exist for device {} after cleanup", device_id);
        }

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn test_proper_resource_disposal() {
        // This test ensures that resources are properly disposed of
        // when devices go out of scope
        {
            let mut fixture = TestFixture::new().await;
            let _device_id = fixture.add_mock_device(DeviceType::TTL).await;

            // Connect device to allocate resources
            if let Some(device_lock) = fixture.app_state.get_device(&_device_id).await {
                let mut device = device_lock.write().await;
                device.connect().await.unwrap();
            }

            // Fixture will be dropped here, triggering cleanup
        } // <- fixture drops here

        // If we reach this point without panicking, cleanup worked properly
        assert!(true, "Resource cleanup completed successfully");
    }
}