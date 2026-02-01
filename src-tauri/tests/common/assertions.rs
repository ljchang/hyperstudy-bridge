//! Assertion helpers for integration tests
//!
//! Provides structured assertions with context that return TestResult
//! instead of panicking, enabling better error propagation.

use super::harness::{TestError, TestHarness, TestResult};
use hyperstudy_bridge::devices::DeviceStatus;
use std::future::Future;
use std::time::{Duration, Instant};

/// Collection of assertion helpers
pub struct Assertions;

impl Assertions {
    /// Assert that a device has the expected status
    ///
    /// # Example
    /// ```ignore
    /// Assertions::assert_device_status(&harness, &device_id, DeviceStatus::Connected, "after connect").await?;
    /// ```
    pub async fn assert_device_status(
        harness: &TestHarness,
        device_id: &str,
        expected: DeviceStatus,
        context: &str,
    ) -> TestResult<()> {
        let actual = harness.get_device_status(device_id).await?;
        if actual != expected {
            return Err(TestError::Assertion(format!(
                "Device {} status mismatch {}: expected {:?}, got {:?}",
                device_id, context, expected, actual
            )));
        }
        Ok(())
    }

    /// Assert that latency is under a threshold
    ///
    /// # Example
    /// ```ignore
    /// Assertions::assert_latency(latency, 1.0, "TTL pulse")?;
    /// ```
    pub fn assert_latency(latency: Duration, threshold_ms: f64, context: &str) -> TestResult<()> {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        if latency_ms > threshold_ms {
            return Err(TestError::Assertion(format!(
                "{}: latency {:.3}ms exceeds threshold {:.3}ms",
                context, latency_ms, threshold_ms
            )));
        }
        Ok(())
    }

    /// Assert that TTL latency is under 1ms (per performance requirements)
    pub fn assert_ttl_latency(latency: Duration, context: &str) -> TestResult<()> {
        Self::assert_latency(latency, 1.0, &format!("TTL {}", context))
    }

    /// Assert that throughput meets minimum requirement
    ///
    /// # Example
    /// ```ignore
    /// Assertions::assert_throughput(throughput, 1000.0, "message processing")?;
    /// ```
    pub fn assert_throughput(throughput: f64, minimum: f64, context: &str) -> TestResult<()> {
        if throughput < minimum {
            return Err(TestError::Assertion(format!(
                "{}: throughput {:.0} ops/sec below minimum {:.0} ops/sec",
                context, throughput, minimum
            )));
        }
        Ok(())
    }

    /// Assert that message throughput meets the 1000 msg/sec requirement
    pub fn assert_message_throughput(throughput: f64, context: &str) -> TestResult<()> {
        Self::assert_throughput(
            throughput,
            1000.0,
            &format!("Message throughput {}", context),
        )
    }

    /// Wait for a synchronous condition with timeout
    ///
    /// Returns Ok(()) if condition becomes true, Err(Timeout) otherwise.
    pub async fn wait_for<F>(mut condition: F, timeout: Duration, context: &str) -> TestResult<()>
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(10);

        while start.elapsed() < timeout {
            if condition() {
                return Ok(());
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(TestError::Timeout(format!(
            "{}: condition not met within {:?}",
            context, timeout
        )))
    }

    /// Wait for an async condition with timeout
    ///
    /// Returns Ok(()) if condition becomes true, Err(Timeout) otherwise.
    pub async fn wait_for_async<F, Fut>(
        mut condition: F,
        timeout: Duration,
        context: &str,
    ) -> TestResult<()>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = bool>,
    {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(10);

        while start.elapsed() < timeout {
            if condition().await {
                return Ok(());
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(TestError::Timeout(format!(
            "{}: condition not met within {:?}",
            context, timeout
        )))
    }

    /// Wait for device to reach expected status
    pub async fn wait_for_device_status(
        harness: &TestHarness,
        device_id: &str,
        expected: DeviceStatus,
        timeout: Duration,
    ) -> TestResult<()> {
        let app_state = harness.app_state.clone();
        let device_id_owned = device_id.to_string();

        Self::wait_for_async(
            || {
                let state = app_state.clone();
                let id = device_id_owned.clone();
                async move { state.get_device_status(&id).await == Some(expected) }
            },
            timeout,
            &format!("device {} to become {:?}", device_id, expected),
        )
        .await
    }

    /// Assert a value is within expected range
    pub fn assert_in_range<T: PartialOrd + std::fmt::Debug>(
        value: T,
        min: T,
        max: T,
        context: &str,
    ) -> TestResult<()> {
        if value < min || value > max {
            return Err(TestError::Assertion(format!(
                "{}: value {:?} not in range [{:?}, {:?}]",
                context, value, min, max
            )));
        }
        Ok(())
    }

    /// Assert approximate equality for floating point values
    pub fn assert_approx_eq(
        actual: f64,
        expected: f64,
        tolerance: f64,
        context: &str,
    ) -> TestResult<()> {
        let diff = (actual - expected).abs();
        if diff > tolerance {
            return Err(TestError::Assertion(format!(
                "{}: {} != {} (tolerance: {}, diff: {})",
                context, actual, expected, tolerance, diff
            )));
        }
        Ok(())
    }

    /// Assert that a duration is within tolerance of expected
    pub fn assert_duration_approx(
        actual: Duration,
        expected: Duration,
        tolerance: Duration,
        context: &str,
    ) -> TestResult<()> {
        let diff = if actual > expected {
            actual - expected
        } else {
            expected - actual
        };

        if diff > tolerance {
            return Err(TestError::Assertion(format!(
                "{}: {:?} not within {:?} of {:?}",
                context, actual, tolerance, expected
            )));
        }
        Ok(())
    }

    /// Assert that memory increase is within threshold
    pub fn assert_no_memory_leak(
        increase_bytes: u64,
        threshold_mb: u64,
        context: &str,
    ) -> TestResult<()> {
        let increase_mb = increase_bytes / 1024 / 1024;
        if increase_mb > threshold_mb {
            return Err(TestError::Assertion(format!(
                "{}: memory increase {}MB exceeds threshold {}MB",
                context, increase_mb, threshold_mb
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_latency_pass() {
        let latency = Duration::from_micros(500);
        assert!(Assertions::assert_latency(latency, 1.0, "test").is_ok());
    }

    #[test]
    fn test_assert_latency_fail() {
        let latency = Duration::from_millis(5);
        assert!(Assertions::assert_latency(latency, 1.0, "test").is_err());
    }

    #[test]
    fn test_assert_throughput_pass() {
        assert!(Assertions::assert_throughput(1500.0, 1000.0, "test").is_ok());
    }

    #[test]
    fn test_assert_throughput_fail() {
        assert!(Assertions::assert_throughput(500.0, 1000.0, "test").is_err());
    }

    #[test]
    fn test_assert_in_range() {
        assert!(Assertions::assert_in_range(5, 1, 10, "test").is_ok());
        assert!(Assertions::assert_in_range(0, 1, 10, "test").is_err());
        assert!(Assertions::assert_in_range(15, 1, 10, "test").is_err());
    }

    #[test]
    fn test_assert_approx_eq() {
        assert!(Assertions::assert_approx_eq(1.0, 1.01, 0.1, "test").is_ok());
        assert!(Assertions::assert_approx_eq(1.0, 2.0, 0.1, "test").is_err());
    }

    #[tokio::test]
    async fn test_wait_for_immediate() {
        let result = Assertions::wait_for(|| true, Duration::from_millis(100), "immediate").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_timeout() {
        let result = Assertions::wait_for(|| false, Duration::from_millis(50), "never true").await;
        assert!(result.is_err());
    }
}
