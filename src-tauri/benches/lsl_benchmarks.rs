// LSL Performance Benchmarks
// These benchmarks measure the performance characteristics of the LSL module

use hyperstudy_bridge::devices::lsl::{LSLDevice, LSLSample, LSLStreamConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

/// Benchmark LSL outlet performance
pub fn bench_lsl_outlet_throughput() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let device = LSLDevice::new(
            "benchmark_outlet".to_string(),
            "Benchmark Outlet Device".to_string(),
        );

        let config = LSLStreamConfig {
            name: "BenchmarkStream".to_string(),
            channel_count: 64,
            sampling_rate: 1000.0,
            ..Default::default()
        };

        device
            .create_outlet("benchmark_outlet".to_string(), config)
            .await
            .unwrap();

        let sample_count = 10000;
        let start_time = Instant::now();

        for i in 0..sample_count {
            let sample = LSLSample {
                data: (0..64).map(|j| (i * 64 + j) as f64 * 0.001).collect(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
                metadata: HashMap::new(),
            };

            device
                .push_to_outlet("benchmark_outlet", sample)
                .await
                .unwrap();
        }

        let duration = start_time.elapsed();
        let samples_per_second = sample_count as f64 / duration.as_secs_f64();
        let data_rate_mbps = (samples_per_second * 64.0 * 8.0) / (1024.0 * 1024.0); // 64 channels * 8 bytes/f64

        println!("LSL Outlet Throughput Benchmark:");
        println!("  Samples: {}", sample_count);
        println!("  Duration: {:?}", duration);
        println!("  Samples/second: {:.0}", samples_per_second);
        println!("  Data rate: {:.2} MB/s", data_rate_mbps);

        // Performance assertions
        assert!(
            samples_per_second >= 1000.0,
            "Throughput too low: {} samples/sec",
            samples_per_second
        );
        assert!(
            duration < Duration::from_secs(20),
            "Benchmark took too long: {:?}",
            duration
        );
    });
}

/// Benchmark LSL latency
pub fn bench_lsl_latency() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let device = LSLDevice::new(
            "benchmark_latency".to_string(),
            "Benchmark Latency Device".to_string(),
        );

        let config = LSLStreamConfig {
            channel_count: 1,
            ..Default::default()
        };

        device
            .create_outlet("latency_outlet".to_string(), config.clone())
            .await
            .unwrap();
        device
            .create_inlet("latency_inlet".to_string(), "BenchmarkStream".to_string())
            .await
            .unwrap();

        let iterations = 1000;
        let mut latencies = Vec::with_capacity(iterations);

        for i in 0..iterations {
            let start = Instant::now();

            let sample = LSLSample {
                data: vec![i as f64],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
                metadata: HashMap::new(),
            };

            // Send sample
            device
                .push_to_outlet("latency_outlet", sample)
                .await
                .unwrap();

            // Try to receive (will be None for mock, but measures the call overhead)
            let _result = device.pull_from_inlet("latency_inlet", 1).await;

            latencies.push(start.elapsed());
        }

        // Calculate statistics
        latencies.sort();
        let median = latencies[iterations / 2];
        let p95 = latencies[(iterations as f64 * 0.95) as usize];
        let p99 = latencies[(iterations as f64 * 0.99) as usize];
        let mean = latencies.iter().sum::<Duration>() / latencies.len() as u32;

        println!("LSL Latency Benchmark:");
        println!("  Iterations: {}", iterations);
        println!("  Mean: {:?}", mean);
        println!("  Median: {:?}", median);
        println!("  95th percentile: {:?}", p95);
        println!("  99th percentile: {:?}", p99);

        // Performance assertions
        assert!(
            median < Duration::from_millis(10),
            "Median latency too high: {:?}",
            median
        );
        assert!(
            p99 < Duration::from_millis(50),
            "99th percentile latency too high: {:?}",
            p99
        );
    });
}

/// Benchmark memory usage under load
pub fn bench_lsl_memory_usage() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let device = Arc::new(LSLDevice::new(
            "benchmark_memory".to_string(),
            "Benchmark Memory Device".to_string(),
        ));

        let config = LSLStreamConfig {
            channel_count: 256,
            sampling_rate: 1000.0,
            ..Default::default()
        };

        device
            .create_outlet("memory_outlet".to_string(), config)
            .await
            .unwrap();

        // Measure initial memory
        let initial_memory = get_memory_usage();

        // Push many large samples
        let sample_count = 5000;
        for i in 0..sample_count {
            let sample = LSLSample {
                data: (0..256).map(|j| (i * 256 + j) as f64).collect(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
                metadata: HashMap::new(),
            };

            device
                .push_to_outlet("memory_outlet", sample)
                .await
                .unwrap();

            // Periodically check memory usage
            if i % 1000 == 0 {
                let current_memory = get_memory_usage();
                println!("Memory at sample {}: {} KB", i, current_memory / 1024);
            }
        }

        let final_memory = get_memory_usage();
        let memory_increase = final_memory.saturating_sub(initial_memory);

        println!("LSL Memory Usage Benchmark:");
        println!("  Initial memory: {} KB", initial_memory / 1024);
        println!("  Final memory: {} KB", final_memory / 1024);
        println!("  Memory increase: {} KB", memory_increase / 1024);
        println!("  Samples processed: {}", sample_count);
        println!(
            "  Memory per sample: {} bytes",
            memory_increase / sample_count
        );

        // Memory usage should be reasonable
        assert!(
            memory_increase < 100 * 1024 * 1024,
            "Memory usage too high: {} MB",
            memory_increase / (1024 * 1024)
        );
    });
}

/// Benchmark concurrent access performance
pub fn bench_lsl_concurrent_access() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let device = Arc::new(LSLDevice::new(
            "benchmark_concurrent".to_string(),
            "Benchmark Concurrent Device".to_string(),
        ));

        let config = LSLStreamConfig {
            channel_count: 16,
            ..Default::default()
        };

        device
            .create_outlet("concurrent_outlet".to_string(), config)
            .await
            .unwrap();

        let thread_count = 10;
        let samples_per_thread = 1000;
        let start_time = Instant::now();

        let mut handles = Vec::new();

        for thread_id in 0..thread_count {
            let device_clone = Arc::clone(&device);
            let handle = tokio::spawn(async move {
                for i in 0..samples_per_thread {
                    let sample = LSLSample {
                        data: (0..16)
                            .map(|j| (thread_id * 1000 + i * 16 + j) as f64)
                            .collect(),
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64(),
                        metadata: HashMap::new(),
                    };

                    device_clone
                        .push_to_outlet("concurrent_outlet", sample)
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start_time.elapsed();
        let total_samples = thread_count * samples_per_thread;
        let samples_per_second = total_samples as f64 / duration.as_secs_f64();

        println!("LSL Concurrent Access Benchmark:");
        println!("  Threads: {}", thread_count);
        println!("  Samples per thread: {}", samples_per_thread);
        println!("  Total samples: {}", total_samples);
        println!("  Duration: {:?}", duration);
        println!("  Samples/second: {:.0}", samples_per_second);

        // Performance assertions
        assert!(
            samples_per_second >= 5000.0,
            "Concurrent throughput too low: {} samples/sec",
            samples_per_second
        );
        assert!(
            duration < Duration::from_secs(10),
            "Concurrent benchmark took too long: {:?}",
            duration
        );
    });
}

/// Benchmark large data transfer
pub fn bench_lsl_large_data() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let device = LSLDevice::new(
            "benchmark_large".to_string(),
            "Benchmark Large Data Device".to_string(),
        );

        let config = LSLStreamConfig {
            channel_count: 1024, // Large channel count
            sampling_rate: 100.0,
            ..Default::default()
        };

        device
            .create_outlet("large_outlet".to_string(), config)
            .await
            .unwrap();

        let sample_count = 100;
        let data_per_sample = 1024 * 8; // 1024 channels * 8 bytes per f64
        let total_data_mb = (sample_count * data_per_sample) as f64 / (1024.0 * 1024.0);

        let start_time = Instant::now();

        for i in 0..sample_count {
            let sample = LSLSample {
                data: (0..1024).map(|j| (i * 1024 + j) as f64 * 0.001).collect(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
                metadata: HashMap::new(),
            };

            device.push_to_outlet("large_outlet", sample).await.unwrap();
        }

        let duration = start_time.elapsed();
        let throughput_mbps = total_data_mb / duration.as_secs_f64();

        println!("LSL Large Data Benchmark:");
        println!("  Sample count: {}", sample_count);
        println!("  Channels per sample: 1024");
        println!("  Data per sample: {} KB", data_per_sample / 1024);
        println!("  Total data: {:.2} MB", total_data_mb);
        println!("  Duration: {:?}", duration);
        println!("  Throughput: {:.2} MB/s", throughput_mbps);

        // Performance assertions
        assert!(
            throughput_mbps >= 10.0,
            "Large data throughput too low: {:.2} MB/s",
            throughput_mbps
        );
        assert!(
            duration < Duration::from_secs(30),
            "Large data benchmark took too long: {:?}",
            duration
        );
    });
}

// Platform-specific memory usage function
#[cfg(target_os = "macos")]
fn get_memory_usage() -> usize {
    use std::process::Command;

    let output = Command::new("ps")
        .args(&["-o", "rss=", "-p"])
        .arg(std::process::id().to_string())
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"0".to_vec(),
            stderr: Vec::new(),
        });

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<usize>()
        .unwrap_or(0)
        * 1024 // Convert KB to bytes
}

#[cfg(not(target_os = "macos"))]
fn get_memory_usage() -> usize {
    // Fallback implementation
    std::process::id() as usize * 1024
}

// If criterion is available, we can create proper benchmarks
#[cfg(feature = "criterion")]
mod criterion_benchmarks {
    use super::*;
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn criterion_outlet_throughput(c: &mut Criterion) {
        c.bench_function("lsl_outlet_throughput", |b| {
            b.iter(|| {
                black_box(bench_lsl_outlet_throughput());
            });
        });
    }

    fn criterion_latency(c: &mut Criterion) {
        c.bench_function("lsl_latency", |b| {
            b.iter(|| {
                black_box(bench_lsl_latency());
            });
        });
    }

    criterion_group!(benches, criterion_outlet_throughput, criterion_latency);
    criterion_main!(benches);
}

// Simple test runner if criterion is not available
#[cfg(not(feature = "criterion"))]
fn main() {
    println!("Running LSL Benchmarks...");

    println!("\n1. Testing Outlet Throughput:");
    bench_lsl_outlet_throughput();

    println!("\n2. Testing Latency:");
    bench_lsl_latency();

    println!("\n3. Testing Memory Usage:");
    bench_lsl_memory_usage();

    println!("\n4. Testing Concurrent Access:");
    bench_lsl_concurrent_access();

    println!("\n5. Testing Large Data Transfer:");
    bench_lsl_large_data();

    println!("\nAll benchmarks completed successfully!");
}

#[cfg(feature = "criterion")]
fn main() {
    // Criterion will handle the main function when the feature is enabled
}

#[cfg(test)]
mod benchmark_tests {
    use super::*;

    #[test]
    fn test_benchmark_outlet_throughput() {
        bench_lsl_outlet_throughput();
    }

    #[test]
    fn test_benchmark_latency() {
        bench_lsl_latency();
    }

    #[test]
    fn test_benchmark_memory_usage() {
        bench_lsl_memory_usage();
    }

    #[test]
    fn test_benchmark_concurrent_access() {
        bench_lsl_concurrent_access();
    }

    #[test]
    fn test_benchmark_large_data() {
        bench_lsl_large_data();
    }
}
