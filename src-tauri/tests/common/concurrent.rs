//! Concurrent test operations without 'static requirements
//!
//! This module provides utilities for running concurrent test operations
//! without requiring closures to have 'static bounds.

use super::harness::{TestError, TestResult};
use std::future::Future;
use std::time::{Duration, Instant};

/// Result of concurrent operations with error tracking
#[derive(Debug)]
pub struct ConcurrentResult<T> {
    /// Results from all workers (Ok or Err for each)
    pub results: Vec<Result<T, TestError>>,

    /// Total duration of the concurrent operation
    pub duration: Duration,
}

impl<T> ConcurrentResult<T> {
    /// Check if all operations succeeded
    pub fn all_ok(&self) -> bool {
        self.results.iter().all(|r| r.is_ok())
    }

    /// Count the number of errors
    pub fn error_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_err()).count()
    }

    /// Count the number of successes
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_ok()).count()
    }

    /// Get all errors
    pub fn errors(&self) -> Vec<&TestError> {
        self.results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .collect()
    }

    /// Unwrap all results, panicking if any failed
    ///
    /// Use this when you expect all operations to succeed
    pub fn unwrap_all(self) -> Vec<T> {
        self.results
            .into_iter()
            .map(|r| r.expect("Expected all concurrent operations to succeed"))
            .collect()
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        (self.success_count() as f64 / self.results.len() as f64) * 100.0
    }

    /// Get all successful results
    pub fn successes(&self) -> Vec<&T> {
        self.results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .collect()
    }
}

impl<T> ConcurrentResult<Vec<T>> {
    /// Flatten results from workers that each return Vec<T>
    pub fn flatten(self) -> ConcurrentResult<T> {
        let duration = self.duration;
        let results: Vec<Result<T, TestError>> = self
            .results
            .into_iter()
            .flat_map(|r| match r {
                Ok(vec) => vec.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
            .collect();

        ConcurrentResult { results, duration }
    }
}

/// Run a load test with the given closure
///
/// This version does NOT require 'static bounds on the closure,
/// making it much easier to use with test fixtures.
///
/// # Example
/// ```ignore
/// let results = run_load_test(
///     5,   // workers
///     100, // ops per worker
///     |worker_id, op_id| async move {
///         // Your operation here
///         Ok(Duration::from_millis(1))
///     },
/// ).await;
///
/// assert!(results.all_ok());
/// ```
pub async fn run_load_test<F, Fut>(
    worker_count: usize,
    ops_per_worker: usize,
    operation: F,
) -> ConcurrentResult<Vec<Duration>>
where
    F: Fn(usize, usize) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = TestResult<Duration>> + Send + 'static,
{
    let start = Instant::now();
    let mut handles = Vec::with_capacity(worker_count);

    for worker_id in 0..worker_count {
        let op = operation.clone();
        let handle = tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(ops_per_worker);
            for op_id in 0..ops_per_worker {
                match op(worker_id, op_id).await {
                    Ok(latency) => latencies.push(latency),
                    Err(e) => {
                        return Err(TestError::TaskFailed(format!(
                            "Worker {} op {} failed: {}",
                            worker_id, op_id, e
                        )))
                    }
                }
            }
            Ok(latencies)
        });
        handles.push(handle);
    }

    let mut results = Vec::with_capacity(worker_count);
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(Err(TestError::TaskFailed(format!(
                "Task join error: {}",
                e
            )))),
        }
    }

    ConcurrentResult {
        results,
        duration: start.elapsed(),
    }
}

/// Run concurrent operations and collect all results
///
/// Unlike run_load_test, this is more flexible about the return type.
pub async fn run_concurrent<F, Fut, T>(tasks: usize, operation: F) -> ConcurrentResult<T>
where
    F: Fn(usize) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = TestResult<T>> + Send + 'static,
    T: Send + 'static,
{
    let start = Instant::now();
    let mut handles = Vec::with_capacity(tasks);

    for task_id in 0..tasks {
        let op = operation.clone();
        let handle = tokio::spawn(async move {
            op(task_id)
                .await
                .map_err(|e| TestError::TaskFailed(format!("Task {} failed: {}", task_id, e)))
        });
        handles.push(handle);
    }

    let mut results = Vec::with_capacity(tasks);
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(Err(TestError::TaskFailed(format!(
                "Task join error: {}",
                e
            )))),
        }
    }

    ConcurrentResult {
        results,
        duration: start.elapsed(),
    }
}

/// Measure throughput of an operation over a duration
///
/// Returns (operation_count, operations_per_second)
pub async fn measure_throughput<F, Fut>(operation: F, duration: Duration) -> (u64, f64)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    let start = Instant::now();
    let mut count = 0u64;

    while start.elapsed() < duration {
        operation().await;
        count += 1;
    }

    let actual_duration = start.elapsed().as_secs_f64();
    let throughput = count as f64 / actual_duration;

    (count, throughput)
}

/// Measure throughput with error counting
///
/// Returns (success_count, error_count, operations_per_second)
pub async fn measure_throughput_with_errors<F, Fut, E>(
    operation: F,
    duration: Duration,
) -> (u64, u64, f64)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    let start = Instant::now();
    let mut success_count = 0u64;
    let mut error_count = 0u64;

    while start.elapsed() < duration {
        match operation().await {
            Ok(()) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let actual_duration = start.elapsed().as_secs_f64();
    let throughput = (success_count + error_count) as f64 / actual_duration;

    (success_count, error_count, throughput)
}

/// Statistics from a collection of latencies
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

impl LatencyStats {
    /// Calculate statistics from a collection of latencies
    pub fn from_latencies(latencies: &[Duration]) -> Option<Self> {
        if latencies.is_empty() {
            return None;
        }

        let mut sorted: Vec<Duration> = latencies.to_vec();
        sorted.sort();

        let count = sorted.len();
        let min = sorted[0];
        let max = sorted[count - 1];
        let avg = sorted.iter().sum::<Duration>() / count as u32;

        let percentile = |p: f64| -> Duration {
            let idx = ((count as f64 * p) as usize).min(count - 1);
            sorted[idx]
        };

        Some(Self {
            count,
            min,
            max,
            avg,
            p50: percentile(0.50),
            p95: percentile(0.95),
            p99: percentile(0.99),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_concurrent_all_succeed() {
        let result = run_concurrent(5, |task_id| async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(task_id)
        })
        .await;

        assert!(result.all_ok());
        assert_eq!(result.success_count(), 5);
        assert_eq!(result.error_count(), 0);
    }

    #[tokio::test]
    async fn test_run_concurrent_some_fail() {
        let result = run_concurrent(5, |task_id| async move {
            if task_id % 2 == 0 {
                Ok(task_id)
            } else {
                Err(TestError::Assertion("odd task".to_string()))
            }
        })
        .await;

        assert!(!result.all_ok());
        assert_eq!(result.success_count(), 3); // 0, 2, 4
        assert_eq!(result.error_count(), 2); // 1, 3
    }

    #[tokio::test]
    async fn test_run_load_test() {
        let result = run_load_test(3, 10, |_worker_id, _op_id| async move {
            Ok(Duration::from_micros(100))
        })
        .await;

        assert!(result.all_ok());
        let flattened = result.flatten();
        assert_eq!(flattened.success_count(), 30); // 3 workers * 10 ops
    }

    #[tokio::test]
    async fn test_measure_throughput() {
        let (count, throughput) = measure_throughput(
            || async {
                tokio::time::sleep(Duration::from_millis(1)).await;
            },
            Duration::from_millis(100),
        )
        .await;

        assert!(count > 0);
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_latency_stats() {
        let latencies: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();

        let stats = LatencyStats::from_latencies(&latencies).unwrap();

        assert_eq!(stats.count, 100);
        assert_eq!(stats.min, Duration::from_millis(1));
        assert_eq!(stats.max, Duration::from_millis(100));
        assert!(stats.p50 >= Duration::from_millis(49));
        assert!(stats.p95 >= Duration::from_millis(94));
    }

    #[test]
    fn test_concurrent_result_success_rate() {
        let result: ConcurrentResult<i32> = ConcurrentResult {
            results: vec![Ok(1), Ok(2), Err(TestError::Assertion("test".into()))],
            duration: Duration::from_secs(1),
        };

        assert!((result.success_rate() - 66.67).abs() < 1.0);
    }
}
