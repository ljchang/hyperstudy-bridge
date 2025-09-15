use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{interval, timeout};

/// Stress test configuration
struct StressConfig {
    duration_secs: u64,
    max_connections: usize,
    messages_per_second: u64,
    message_size_bytes: usize,
    concurrent_devices: usize,
}

impl Default for StressConfig {
    fn default() -> Self {
        Self {
            duration_secs: 30,
            max_connections: 100,
            messages_per_second: 1000,
            message_size_bytes: 1024,
            concurrent_devices: 10,
        }
    }
}

/// Mock components for stress testing
mod stress_components {
    use super::*;

    pub struct StressTestDevice {
        id: String,
        message_count: AtomicU64,
        error_count: AtomicU64,
        is_connected: AtomicBool,
        latency_sum: AtomicU64,
        latency_count: AtomicU64,
    }

    impl StressTestDevice {
        pub fn new(id: String) -> Self {
            Self {
                id,
                message_count: AtomicU64::new(0),
                error_count: AtomicU64::new(0),
                is_connected: AtomicBool::new(false),
                latency_sum: AtomicU64::new(0),
                latency_count: AtomicU64::new(0),
            }
        }

        pub async fn connect(&self) -> Result<(), String> {
            // Simulate connection time
            tokio::time::sleep(Duration::from_millis(10 + rand::random::<u64>() % 50)).await;

            if rand::random::<f64>() < 0.95 { // 95% success rate
                self.is_connected.store(true, Ordering::Relaxed);
                Ok(())
            } else {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                Err(format!("Failed to connect to device {}", self.id))
            }
        }

        pub async fn send_message(&self, _data: &[u8]) -> Result<Duration, String> {
            if !self.is_connected.load(Ordering::Relaxed) {
                return Err("Device not connected".to_string());
            }

            let start = Instant::now();

            // Simulate processing time with some variation
            let base_delay = Duration::from_micros(100 + rand::random::<u64>() % 500);
            tokio::time::sleep(base_delay).await;

            let elapsed = start.elapsed();

            // Simulate occasional failures
            if rand::random::<f64>() < 0.98 { // 98% success rate
                self.message_count.fetch_add(1, Ordering::Relaxed);
                self.latency_sum.fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
                self.latency_count.fetch_add(1, Ordering::Relaxed);
                Ok(elapsed)
            } else {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                Err("Message send failed".to_string())
            }
        }

        pub fn disconnect(&self) {
            self.is_connected.store(false, Ordering::Relaxed);
        }

        pub fn get_stats(&self) -> DeviceStats {
            let latency_count = self.latency_count.load(Ordering::Relaxed);
            let avg_latency = if latency_count > 0 {
                self.latency_sum.load(Ordering::Relaxed) / latency_count
            } else {
                0
            };

            DeviceStats {
                message_count: self.message_count.load(Ordering::Relaxed),
                error_count: self.error_count.load(Ordering::Relaxed),
                avg_latency_ns: avg_latency,
                is_connected: self.is_connected.load(Ordering::Relaxed),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct DeviceStats {
        pub message_count: u64,
        pub error_count: u64,
        pub avg_latency_ns: u64,
        pub is_connected: bool,
    }

    pub struct StressTestServer {
        connection_count: AtomicU64,
        message_count: AtomicU64,
        error_count: AtomicU64,
        max_connections: usize,
    }

    impl StressTestServer {
        pub fn new(max_connections: usize) -> Self {
            Self {
                connection_count: AtomicU64::new(0),
                message_count: AtomicU64::new(0),
                error_count: AtomicU64::new(0),
                max_connections,
            }
        }

        pub async fn handle_connection(&self) -> Result<(), String> {
            let current = self.connection_count.load(Ordering::Relaxed);
            if current >= self.max_connections as u64 {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err("Max connections exceeded".to_string());
            }

            self.connection_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        pub async fn handle_message(&self, _data: &[u8]) -> Result<(), String> {
            // Simulate message processing
            tokio::task::yield_now().await;

            if rand::random::<f64>() < 0.999 { // 99.9% success rate
                self.message_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            } else {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                Err("Message processing failed".to_string())
            }
        }

        pub fn close_connection(&self) {
            if self.connection_count.load(Ordering::Relaxed) > 0 {
                self.connection_count.fetch_sub(1, Ordering::Relaxed);
            }
        }

        pub fn get_stats(&self) -> ServerStats {
            ServerStats {
                connection_count: self.connection_count.load(Ordering::Relaxed),
                message_count: self.message_count.load(Ordering::Relaxed),
                error_count: self.error_count.load(Ordering::Relaxed),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct ServerStats {
        pub connection_count: u64,
        pub message_count: u64,
        pub error_count: u64,
    }
}

use stress_components::*;

/// Stress test: High message throughput (1000+ messages/second)
fn stress_test_high_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("Stress Test High Throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10); // Fewer samples for stress tests

    for msg_per_sec in [1000, 2000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("messages_per_second", msg_per_sec),
            msg_per_sec,
            |b, &msg_per_sec| {
                b.to_async(&rt).iter(|| async {
                    let device = StressTestDevice::new("stress-device".to_string());
                    let _ = device.connect().await;

                    let message = vec![0u8; 1024];
                    let duration = Duration::from_secs(5);
                    let start_time = Instant::now();

                    let mut interval_timer = interval(Duration::from_nanos(1_000_000_000 / msg_per_sec));

                    let mut successful_messages = 0;
                    let mut failed_messages = 0;

                    while start_time.elapsed() < duration {
                        interval_timer.tick().await;

                        match device.send_message(&message).await {
                            Ok(_) => successful_messages += 1,
                            Err(_) => failed_messages += 1,
                        }
                    }

                    device.disconnect();
                    black_box((successful_messages, failed_messages))
                });
            }
        );
    }

    group.finish();
}

/// Stress test: Many concurrent WebSocket connections
fn stress_test_concurrent_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("Stress Test Concurrent Connections");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    for connection_count in [50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_connections", connection_count),
            connection_count,
            |b, &connection_count| {
                b.to_async(&rt).iter(|| async {
                    let server = Arc::new(StressTestServer::new(connection_count * 2));
                    let semaphore = Arc::new(Semaphore::new(connection_count));

                    let tasks: Vec<_> = (0..connection_count).map(|i| {
                        let server = Arc::clone(&server);
                        let semaphore = Arc::clone(&semaphore);

                        tokio::spawn(async move {
                            let _permit = semaphore.acquire().await.unwrap();

                            // Connect
                            if let Err(_) = server.handle_connection().await {
                                return 0;
                            }

                            let message = vec![0u8; 512];
                            let mut message_count = 0;

                            // Send messages for 3 seconds
                            let start = Instant::now();
                            while start.elapsed() < Duration::from_secs(3) {
                                if server.handle_message(&message).await.is_ok() {
                                    message_count += 1;
                                }

                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }

                            server.close_connection();
                            message_count
                        })
                    }).collect();

                    let results: Vec<_> = futures_util::future::join_all(tasks).await;
                    let total_messages: i32 = results.into_iter().map(|r| r.unwrap()).sum();

                    let final_stats = server.get_stats();
                    black_box((total_messages, final_stats))
                });
            }
        );
    }

    group.finish();
}

/// Stress test: Long-running stability test
fn stress_test_long_running_stability(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("long_running_stability", |b| {
        b.to_async(&rt).iter(|| async {
            let device_count = 5;
            let devices: Vec<_> = (0..device_count)
                .map(|i| Arc::new(StressTestDevice::new(format!("device-{}", i))))
                .collect();

            // Connect all devices
            let connect_tasks: Vec<_> = devices.iter().map(|device| {
                let device = Arc::clone(device);
                tokio::spawn(async move {
                    device.connect().await
                })
            }).collect();

            let _connect_results: Vec<_> = futures_util::future::join_all(connect_tasks).await;

            // Run for 10 seconds (reduced from hours for benchmark)
            let test_duration = Duration::from_secs(10);
            let start_time = Instant::now();

            let tasks: Vec<_> = devices.iter().map(|device| {
                let device = Arc::clone(device);
                tokio::spawn(async move {
                    let message = vec![0u8; 256];
                    let mut local_message_count = 0;

                    while start_time.elapsed() < test_duration {
                        match device.send_message(&message).await {
                            Ok(_) => local_message_count += 1,
                            Err(_) => {}
                        }

                        // Random delay between messages
                        tokio::time::sleep(Duration::from_millis(1 + rand::random::<u64>() % 10)).await;
                    }

                    local_message_count
                })
            }).collect();

            let results: Vec<_> = futures_util::future::join_all(tasks).await;
            let total_messages: u64 = results.into_iter().map(|r| r.unwrap()).sum();

            // Collect final statistics
            let device_stats: Vec<_> = devices.iter().map(|d| d.get_stats()).collect();

            // Disconnect all devices
            for device in &devices {
                device.disconnect();
            }

            black_box((total_messages, device_stats))
        });
    });
}

/// Stress test: Memory leak detection under load
fn stress_test_memory_leak_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_leak_detection", |b| {
        b.to_async(&rt).iter(|| async {
            let iterations = 1000;
            let mut peak_memory_usage = 0usize;

            for batch in 0..iterations {
                // Create temporary objects that should be cleaned up
                let devices: Vec<_> = (0..10)
                    .map(|i| StressTestDevice::new(format!("temp-device-{}-{}", batch, i)))
                    .collect();

                let server = StressTestServer::new(100);

                // Perform operations
                for device in &devices {
                    let _ = device.connect().await;
                    let message = vec![0u8; 1024];

                    for _ in 0..10 {
                        let _ = device.send_message(&message).await;
                    }

                    device.disconnect();
                }

                // Simulate memory usage tracking (in real implementation, would use actual memory profiling)
                let estimated_memory = (batch + 1) * 1024; // Simplified estimation
                if estimated_memory > peak_memory_usage {
                    peak_memory_usage = estimated_memory;
                }

                // Force potential cleanup
                tokio::task::yield_now().await;

                // Every 100 iterations, check for excessive memory growth
                if batch % 100 == 0 && batch > 0 {
                    // In real implementation, check actual memory usage
                    // For now, just simulate memory measurement
                    let current_memory = peak_memory_usage;
                    black_box(current_memory);
                }
            }

            black_box(peak_memory_usage)
        });
    });
}

/// Stress test: Resource exhaustion scenarios
fn stress_test_resource_exhaustion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("Resource Exhaustion");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    // Test file descriptor exhaustion simulation
    group.bench_function("file_descriptor_exhaustion", |b| {
        b.to_async(&rt).iter(|| async {
            let max_connections = 1000;
            let server = StressTestServer::new(max_connections);

            let mut successful_connections = 0;
            let mut failed_connections = 0;

            // Try to create many connections rapidly
            for i in 0..max_connections * 2 {
                match server.handle_connection().await {
                    Ok(_) => {
                        successful_connections += 1;

                        // Simulate some work
                        let message = vec![0u8; 64];
                        let _ = server.handle_message(&message).await;

                        // Occasionally close connections to prevent total exhaustion
                        if i % 100 == 0 {
                            for _ in 0..10 {
                                server.close_connection();
                            }
                        }
                    },
                    Err(_) => failed_connections += 1,
                }

                if i % 50 == 0 {
                    tokio::task::yield_now().await;
                }
            }

            black_box((successful_connections, failed_connections))
        });
    });

    // Test message queue backlog
    group.bench_function("message_queue_backlog", |b| {
        b.to_async(&rt).iter(|| async {
            let (tx, mut rx) = mpsc::channel(100);
            let message_count = Arc::new(AtomicU64::new(0));

            // Producer task - sends messages rapidly
            let producer_count = Arc::clone(&message_count);
            let producer = tokio::spawn(async move {
                let message = vec![0u8; 1024];

                for i in 0..5000 {
                    match timeout(Duration::from_millis(1), tx.send((i, message.clone()))).await {
                        Ok(Ok(_)) => producer_count.fetch_add(1, Ordering::Relaxed),
                        _ => break, // Channel full or timeout
                    };
                }
            });

            // Consumer task - processes messages slowly
            let consumer_count = Arc::new(AtomicU64::new(0));
            let consumer_count_clone = Arc::clone(&consumer_count);
            let consumer = tokio::spawn(async move {
                let mut processed = 0;

                while let Some((_id, data)) = rx.recv().await {
                    // Simulate slow processing
                    tokio::time::sleep(Duration::from_micros(100)).await;
                    black_box(data);
                    processed += 1;
                    consumer_count_clone.store(processed, Ordering::Relaxed);

                    if processed >= 1000 {
                        break;
                    }
                }
            });

            // Run for limited time
            let timeout_duration = Duration::from_secs(3);
            let _ = timeout(timeout_duration, producer).await;
            let _ = timeout(timeout_duration, consumer).await;

            let sent = message_count.load(Ordering::Relaxed);
            let processed = consumer_count.load(Ordering::Relaxed);

            black_box((sent, processed, sent.saturating_sub(processed)))
        });
    });

    group.finish();
}

/// Stress test: Error recovery and resilience
fn stress_test_error_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("error_recovery_resilience", |b| {
        b.to_async(&rt).iter(|| async {
            let device_count = 10;
            let devices: Vec<_> = (0..device_count)
                .map(|i| Arc::new(StressTestDevice::new(format!("resilience-device-{}", i))))
                .collect();

            let mut recovery_count = 0;
            let test_duration = Duration::from_secs(5);
            let start_time = Instant::now();

            // Initial connections
            for device in &devices {
                let _ = device.connect().await;
            }

            while start_time.elapsed() < test_duration {
                // Randomly disconnect some devices (simulate errors)
                for (i, device) in devices.iter().enumerate() {
                    if rand::random::<f64>() < 0.1 { // 10% chance to disconnect
                        device.disconnect();
                    }
                }

                // Attempt to reconnect disconnected devices
                for device in &devices {
                    if !device.get_stats().is_connected {
                        if device.connect().await.is_ok() {
                            recovery_count += 1;
                        }
                    }
                }

                // Send messages to connected devices
                let message = vec![0u8; 512];
                let tasks: Vec<_> = devices.iter().map(|device| {
                    let device = Arc::clone(device);
                    let message = message.clone();
                    tokio::spawn(async move {
                        if device.get_stats().is_connected {
                            device.send_message(&message).await.is_ok()
                        } else {
                            false
                        }
                    })
                }).collect();

                let _results: Vec<_> = futures_util::future::join_all(tasks).await;

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Collect final statistics
            let final_stats: Vec<_> = devices.iter().map(|d| d.get_stats()).collect();
            let total_messages: u64 = final_stats.iter().map(|s| s.message_count).sum();
            let total_errors: u64 = final_stats.iter().map(|s| s.error_count).sum();

            black_box((total_messages, total_errors, recovery_count))
        });
    });
}

/// Stress test: Performance under thermal throttling simulation
fn stress_test_thermal_throttling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("thermal_throttling_simulation", |b| {
        b.to_async(&rt).iter(|| async {
            let device = StressTestDevice::new("thermal-device".to_string());
            let _ = device.connect().await;

            let mut message_count = 0;
            let test_duration = Duration::from_secs(5);
            let start_time = Instant::now();

            while start_time.elapsed() < test_duration {
                let message = vec![0u8; 1024];

                // Simulate thermal throttling by gradually increasing delays
                let elapsed_ratio = start_time.elapsed().as_secs_f64() / test_duration.as_secs_f64();
                let throttle_delay = Duration::from_micros((elapsed_ratio * 1000.0) as u64);

                tokio::time::sleep(throttle_delay).await;

                if device.send_message(&message).await.is_ok() {
                    message_count += 1;
                }

                // Add CPU-intensive work to generate "heat"
                let mut sum = 0u64;
                for i in 0..1000 {
                    sum = sum.wrapping_add(i);
                }
                black_box(sum);
            }

            device.disconnect();
            black_box(message_count)
        });
    });
}

criterion_group!(
    stress_benches,
    stress_test_high_throughput,
    stress_test_concurrent_connections,
    stress_test_long_running_stability,
    stress_test_memory_leak_detection,
    stress_test_resource_exhaustion,
    stress_test_error_recovery,
    stress_test_thermal_throttling
);

criterion_main!(stress_benches);