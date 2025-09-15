use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use hyperstudy_bridge::*;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

// Mock modules for benchmarking (these would normally be imported from the main crate)
mod mock_bridge {
    use super::*;

    pub struct MockTtlDevice {
        latency_us: u64,
        pulse_count: u64,
    }

    impl MockTtlDevice {
        pub fn new(latency_us: u64) -> Self {
            Self {
                latency_us,
                pulse_count: 0,
            }
        }

        pub async fn send_pulse(&mut self) -> Duration {
            let start = Instant::now();

            // Simulate device processing time
            tokio::time::sleep(Duration::from_micros(self.latency_us)).await;

            self.pulse_count += 1;
            start.elapsed()
        }

        pub fn get_pulse_count(&self) -> u64 {
            self.pulse_count
        }
    }

    pub struct MockWebSocketServer {
        message_count: u64,
    }

    impl MockWebSocketServer {
        pub fn new() -> Self {
            Self { message_count: 0 }
        }

        pub async fn handle_message(&mut self, _message: &[u8]) -> Duration {
            let start = Instant::now();

            // Simulate message processing
            self.message_count += 1;

            // Simulate routing and response time
            tokio::task::yield_now().await;

            start.elapsed()
        }

        pub fn get_message_count(&self) -> u64 {
            self.message_count
        }
    }

    pub struct MockDataStream {
        pub sample_rate_hz: u32,
        pub channels: usize,
    }

    impl MockDataStream {
        pub fn new(sample_rate_hz: u32, channels: usize) -> Self {
            Self { sample_rate_hz, channels }
        }

        pub async fn generate_sample(&self) -> Vec<f64> {
            // Simulate data generation time
            tokio::task::yield_now().await;

            // Generate mock data
            (0..self.channels).map(|_| rand::random::<f64>()).collect()
        }
    }
}

use mock_bridge::*;

/// Benchmark TTL pulse latency - Critical requirement: <1ms
fn bench_ttl_pulse_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("TTL Pulse Latency");
    group.measurement_time(Duration::from_secs(10));

    // Test different simulated hardware latencies
    for latency_us in [100, 500, 800, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("pulse_latency_us", latency_us),
            latency_us,
            |b, &latency_us| {
                b.to_async(&rt).iter(|| async {
                    let mut device = MockTtlDevice::new(latency_us);
                    let latency = device.send_pulse().await;
                    black_box(latency)
                });
            }
        );
    }

    group.finish();
}

/// Benchmark WebSocket message throughput - Target: >1000 msg/sec
fn bench_websocket_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("WebSocket Throughput");
    group.measurement_time(Duration::from_secs(5));

    // Test different message sizes
    for msg_size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::new("message_throughput_bytes", msg_size),
            msg_size,
            |b, &msg_size| {
                b.to_async(&rt).iter(|| async {
                    let mut server = MockWebSocketServer::new();
                    let message = vec![0u8; msg_size];

                    let start = Instant::now();
                    for _ in 0..100 {
                        server.handle_message(&message).await;
                    }
                    let elapsed = start.elapsed();

                    black_box(elapsed)
                });
            }
        );
    }

    group.finish();
}

/// Benchmark concurrent device operations
fn bench_concurrent_devices(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("Concurrent Device Operations");
    group.measurement_time(Duration::from_secs(8));

    // Test with different numbers of concurrent devices
    for device_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_devices", device_count),
            device_count,
            |b, &device_count| {
                b.to_async(&rt).iter(|| async {
                    let mut devices = Vec::new();
                    for i in 0..device_count {
                        devices.push(MockTtlDevice::new(500 + (i as u64 * 100)));
                    }

                    let start = Instant::now();

                    // Send pulses concurrently
                    let tasks: Vec<_> = devices.into_iter().map(|mut device| {
                        tokio::spawn(async move {
                            for _ in 0..10 {
                                device.send_pulse().await;
                            }
                            device.get_pulse_count()
                        })
                    }).collect();

                    let results: Vec<_> = futures_util::future::join_all(tasks).await;
                    let elapsed = start.elapsed();

                    let total_pulses: u64 = results.into_iter().map(|r| r.unwrap()).sum();
                    black_box((elapsed, total_pulses))
                });
            }
        );
    }

    group.finish();
}

/// Benchmark data streaming performance
fn bench_data_streaming(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("Data Streaming");
    group.measurement_time(Duration::from_secs(5));

    // Test different sample rates
    for sample_rate in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("streaming_hz", sample_rate),
            sample_rate,
            |b, &sample_rate| {
                b.to_async(&rt).iter(|| async {
                    let stream = MockDataStream::new(sample_rate, 8); // 8 channels
                    let (tx, mut rx) = mpsc::channel(100);

                    // Producer task
                    let producer_stream = stream;
                    let producer = tokio::spawn(async move {
                        let interval = Duration::from_nanos(1_000_000_000 / producer_stream.sample_rate_hz as u64);
                        let mut last_time = Instant::now();

                        for _ in 0..100 { // Generate 100 samples
                            let sample = producer_stream.generate_sample().await;
                            if tx.send((last_time, sample)).await.is_err() {
                                break;
                            }

                            let now = Instant::now();
                            let elapsed = now.duration_since(last_time);
                            if elapsed < interval {
                                tokio::time::sleep(interval - elapsed).await;
                            }
                            last_time = Instant::now();
                        }
                    });

                    // Consumer task
                    let consumer = tokio::spawn(async move {
                        let mut sample_count = 0;
                        let start_time = Instant::now();

                        while let Some((_timestamp, sample)) = rx.recv().await {
                            black_box(sample);
                            sample_count += 1;

                            if sample_count >= 100 {
                                break;
                            }
                        }

                        (start_time.elapsed(), sample_count)
                    });

                    let _ = producer.await;
                    let (elapsed, count) = consumer.await.unwrap();

                    black_box((elapsed, count))
                });
            }
        );
    }

    group.finish();
}

/// Benchmark memory usage under load
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_usage_high_load", |b| {
        b.to_async(&rt).iter(|| async {
            let mut devices = Vec::new();
            let mut servers = Vec::new();

            // Create multiple mock devices and servers
            for i in 0..10 {
                devices.push(MockTtlDevice::new(500));
                servers.push(MockWebSocketServer::new());
            }

            // Simulate high load
            let tasks: Vec<_> = (0..10).map(|i| {
                let mut device = MockTtlDevice::new(500 + i * 50);
                let mut server = MockWebSocketServer::new();

                tokio::spawn(async move {
                    let message = vec![0u8; 1024];

                    // Simulate sustained operation
                    for _ in 0..50 {
                        let _ = device.send_pulse().await;
                        let _ = server.handle_message(&message).await;
                        tokio::task::yield_now().await;
                    }

                    (device.get_pulse_count(), server.get_message_count())
                })
            }).collect();

            let results: Vec<_> = futures_util::future::join_all(tasks).await;
            let total_operations: u64 = results.into_iter()
                .map(|r| r.unwrap())
                .map(|(pulses, messages)| pulses + messages)
                .sum();

            black_box(total_operations)
        });
    });
}

/// Benchmark WebSocket connection handling
fn bench_websocket_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("WebSocket Connections");
    group.measurement_time(Duration::from_secs(3));

    // Test handling multiple concurrent connections
    for connection_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_connections", connection_count),
            connection_count,
            |b, &connection_count| {
                b.to_async(&rt).iter(|| async {
                    let servers: Vec<_> = (0..connection_count)
                        .map(|_| MockWebSocketServer::new())
                        .collect();

                    let tasks: Vec<_> = servers.into_iter().map(|mut server| {
                        tokio::spawn(async move {
                            let message = vec![0u8; 256];

                            // Each connection handles 20 messages
                            for _ in 0..20 {
                                server.handle_message(&message).await;
                            }

                            server.get_message_count()
                        })
                    }).collect();

                    let start = Instant::now();
                    let results: Vec<_> = futures_util::future::join_all(tasks).await;
                    let elapsed = start.elapsed();

                    let total_messages: u64 = results.into_iter().map(|r| r.unwrap()).sum();
                    black_box((elapsed, total_messages))
                });
            }
        );
    }

    group.finish();
}

/// Benchmark time synchronization accuracy
fn bench_time_synchronization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("time_synchronization_accuracy", |b| {
        b.to_async(&rt).iter(|| async {
            let device_count = 5;
            let mut devices = Vec::new();

            for i in 0..device_count {
                devices.push(MockTtlDevice::new(200 + i * 100));
            }

            // Simulate synchronized trigger
            let sync_time = Instant::now();
            let tasks: Vec<_> = devices.into_iter().map(|mut device| {
                tokio::spawn(async move {
                    let trigger_time = Instant::now();
                    let latency = device.send_pulse().await;
                    (trigger_time, latency)
                })
            }).collect();

            let results: Vec<_> = futures_util::future::join_all(tasks).await;
            let timestamps: Vec<_> = results.into_iter()
                .map(|r| r.unwrap().0)
                .collect();

            // Calculate synchronization accuracy (max deviation)
            let min_time = timestamps.iter().min().unwrap();
            let max_deviation = timestamps.iter()
                .map(|t| t.duration_since(*min_time))
                .max()
                .unwrap();

            black_box(max_deviation)
        });
    });
}

criterion_group!(
    benches,
    bench_ttl_pulse_latency,
    bench_websocket_throughput,
    bench_concurrent_devices,
    bench_data_streaming,
    bench_memory_usage,
    bench_websocket_connections,
    bench_time_synchronization
);

criterion_main!(benches);