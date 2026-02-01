//! Time-series storage for LSL stream data.
//!
//! This module provides:
//! - Stream metadata storage
//! - Batch sample insertion for high-frequency data
//! - Time-range queries for data export

use super::StorageResult;
use sqlx::SqlitePool;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// LSL stream metadata as stored in the database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct StreamMetadata {
    pub uid: String,
    pub session_id: String,
    pub name: String,
    pub stream_type: String,
    pub channel_count: i32,
    pub sample_rate: f64,
    pub channel_format: String,
    pub source_id: Option<String>,
    pub hostname: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

/// A single LSL sample with packed channel data.
#[derive(Debug, Clone)]
pub struct LslSample {
    pub stream_uid: String,
    pub timestamp: f64,
    pub channel_data: Vec<u8>, // Packed binary data
}

impl LslSample {
    /// Create a new sample from float32 channel values.
    pub fn from_f32(stream_uid: String, timestamp: f64, channels: &[f32]) -> Self {
        let mut channel_data = Vec::with_capacity(channels.len() * 4);
        for &value in channels {
            channel_data.extend_from_slice(&value.to_le_bytes());
        }
        Self {
            stream_uid,
            timestamp,
            channel_data,
        }
    }

    /// Create a new sample from float64 channel values.
    pub fn from_f64(stream_uid: String, timestamp: f64, channels: &[f64]) -> Self {
        let mut channel_data = Vec::with_capacity(channels.len() * 8);
        for &value in channels {
            channel_data.extend_from_slice(&value.to_le_bytes());
        }
        Self {
            stream_uid,
            timestamp,
            channel_data,
        }
    }

    /// Unpack channel data as float32 values.
    pub fn to_f32(&self) -> Vec<f32> {
        self.channel_data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    /// Unpack channel data as float64 values.
    pub fn to_f64(&self) -> Vec<f64> {
        self.channel_data
            .chunks_exact(8)
            .map(|chunk| {
                f64::from_le_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
                ])
            })
            .collect()
    }
}

/// A stored sample with database ID.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct StoredSample {
    pub id: i64,
    pub session_id: String,
    pub stream_uid: String,
    pub timestamp: f64,
    #[serde(with = "base64_bytes")]
    pub channel_data: Vec<u8>,
}

/// Serde helper for serializing Vec<u8> as base64
mod base64_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::Engine;
        let s = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}

/// Configuration for the sample batcher.
#[derive(Debug, Clone)]
pub struct SampleBatcherConfig {
    /// Number of samples to accumulate before flushing.
    pub batch_size: usize,
    /// Maximum time between flushes.
    pub flush_interval: Duration,
}

impl Default for SampleBatcherConfig {
    fn default() -> Self {
        Self {
            batch_size: 500,
            flush_interval: Duration::from_secs(10),
        }
    }
}

/// Batches LSL samples for efficient database writes.
pub struct SampleBatcher {
    buffer: RwLock<Vec<(String, LslSample)>>, // (session_id, sample)
    config: SampleBatcherConfig,
    last_flush: RwLock<Instant>,
    pool: SqlitePool,
}

impl SampleBatcher {
    /// Create a new sample batcher.
    pub fn new(pool: SqlitePool, config: SampleBatcherConfig) -> Self {
        Self {
            buffer: RwLock::new(Vec::with_capacity(config.batch_size)),
            config,
            last_flush: RwLock::new(Instant::now()),
            pool,
        }
    }

    /// Add a sample to the batch.
    pub async fn add(&self, session_id: String, sample: LslSample) {
        let should_flush = {
            let mut buffer = self.buffer.write().await;
            buffer.push((session_id, sample));

            let last_flush = *self.last_flush.read().await;
            buffer.len() >= self.config.batch_size
                || last_flush.elapsed() >= self.config.flush_interval
        };

        if should_flush {
            self.flush().await;
        }
    }

    /// Add multiple samples to the batch.
    pub async fn add_batch(&self, session_id: String, samples: Vec<LslSample>) {
        if samples.is_empty() {
            return;
        }

        let should_flush = {
            let mut buffer = self.buffer.write().await;
            buffer.extend(samples.into_iter().map(|s| (session_id.clone(), s)));

            let last_flush = *self.last_flush.read().await;
            buffer.len() >= self.config.batch_size
                || last_flush.elapsed() >= self.config.flush_interval
        };

        if should_flush {
            self.flush().await;
        }
    }

    /// Force flush all buffered samples to the database.
    pub async fn flush(&self) {
        let samples = {
            let mut buffer = self.buffer.write().await;
            if buffer.is_empty() {
                return;
            }
            std::mem::take(&mut *buffer)
        };

        let count = samples.len();

        if let Err(e) = self.insert_batch(&samples).await {
            error!("Failed to flush samples to database: {}", e);
            // On failure, we don't re-add samples to prevent memory issues
            // LSL data can be regenerated, so data loss is acceptable here
        } else {
            debug!("Flushed {} samples to database", count);
            *self.last_flush.write().await = Instant::now();
        }
    }

    /// Insert a batch of samples into the database.
    async fn insert_batch(&self, samples: &[(String, LslSample)]) -> StorageResult<()> {
        if samples.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for (session_id, sample) in samples {
            sqlx::query(
                "INSERT INTO lsl_samples (session_id, stream_uid, timestamp, channel_data)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(session_id)
            .bind(&sample.stream_uid)
            .bind(sample.timestamp)
            .bind(&sample.channel_data)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get the number of samples currently buffered.
    pub async fn buffered_count(&self) -> usize {
        self.buffer.read().await.len()
    }
}

/// Register a new LSL stream in the database.
pub async fn register_stream(
    pool: &SqlitePool,
    session_id: &str,
    uid: &str,
    name: &str,
    stream_type: &str,
    channel_count: i32,
    sample_rate: f64,
    channel_format: &str,
    source_id: Option<&str>,
    hostname: Option<&str>,
    metadata: Option<&str>,
) -> StorageResult<()> {
    let created_at = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR REPLACE INTO lsl_streams
         (uid, session_id, name, stream_type, channel_count, sample_rate,
          channel_format, source_id, hostname, metadata, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uid)
    .bind(session_id)
    .bind(name)
    .bind(stream_type)
    .bind(channel_count)
    .bind(sample_rate)
    .bind(channel_format)
    .bind(source_id)
    .bind(hostname)
    .bind(metadata)
    .bind(&created_at)
    .execute(pool)
    .await?;

    info!("Registered LSL stream: {} ({})", name, uid);
    Ok(())
}

/// Get metadata for a specific stream.
pub async fn get_stream(pool: &SqlitePool, uid: &str) -> StorageResult<Option<StreamMetadata>> {
    let stream = sqlx::query_as::<_, StreamMetadata>(
        "SELECT uid, session_id, name, stream_type, channel_count, sample_rate,
                channel_format, source_id, hostname, metadata, created_at
         FROM lsl_streams WHERE uid = ?",
    )
    .bind(uid)
    .fetch_optional(pool)
    .await?;

    Ok(stream)
}

/// List all streams for a session.
pub async fn list_streams(
    pool: &SqlitePool,
    session_id: &str,
) -> StorageResult<Vec<StreamMetadata>> {
    let streams = sqlx::query_as::<_, StreamMetadata>(
        "SELECT uid, session_id, name, stream_type, channel_count, sample_rate,
                channel_format, source_id, hostname, metadata, created_at
         FROM lsl_streams WHERE session_id = ? ORDER BY created_at",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(streams)
}

/// Query options for time-series data.
#[derive(Debug, Clone, Default)]
pub struct SampleQueryOptions {
    /// Stream UID to query.
    pub stream_uid: String,
    /// Session ID to query.
    pub session_id: String,
    /// Start timestamp (inclusive).
    pub from_timestamp: Option<f64>,
    /// End timestamp (inclusive).
    pub to_timestamp: Option<f64>,
    /// Maximum number of samples to return.
    pub limit: Option<i64>,
    /// Number of samples to skip.
    pub offset: Option<i64>,
}

/// Result of a sample query.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SampleQueryResult {
    pub samples: Vec<StoredSample>,
    pub total_count: i64,
    pub has_more: bool,
}

/// Query samples from the database.
///
/// Limits are capped to prevent memory exhaustion when loading large binary blobs.
pub async fn query_samples(
    pool: &SqlitePool,
    options: SampleQueryOptions,
) -> StorageResult<SampleQueryResult> {
    // Cap at 10,000 samples to prevent memory exhaustion
    // Each sample contains binary channel data (e.g., 64 channels * 8 bytes = 512 bytes)
    // 10,000 samples * 512 bytes = ~5MB max per query
    let limit = options.limit.unwrap_or(1000).clamp(1, 10000);
    let offset = options.offset.unwrap_or(0).max(0);

    let mut conditions = vec!["session_id = ?".to_string(), "stream_uid = ?".to_string()];

    if options.from_timestamp.is_some() {
        conditions.push("timestamp >= ?".to_string());
    }
    if options.to_timestamp.is_some() {
        conditions.push("timestamp <= ?".to_string());
    }

    let where_clause = conditions.join(" AND ");

    // Get total count
    let count_query = format!("SELECT COUNT(*) FROM lsl_samples WHERE {}", where_clause);

    let mut count_builder = sqlx::query_as::<_, (i64,)>(&count_query)
        .bind(&options.session_id)
        .bind(&options.stream_uid);

    if let Some(from_ts) = options.from_timestamp {
        count_builder = count_builder.bind(from_ts);
    }
    if let Some(to_ts) = options.to_timestamp {
        count_builder = count_builder.bind(to_ts);
    }

    let total_count = count_builder.fetch_one(pool).await?.0;

    // Get samples
    let query = format!(
        "SELECT id, session_id, stream_uid, timestamp, channel_data
         FROM lsl_samples WHERE {} ORDER BY timestamp LIMIT ? OFFSET ?",
        where_clause
    );

    let mut builder = sqlx::query_as::<_, StoredSample>(&query)
        .bind(&options.session_id)
        .bind(&options.stream_uid);

    if let Some(from_ts) = options.from_timestamp {
        builder = builder.bind(from_ts);
    }
    if let Some(to_ts) = options.to_timestamp {
        builder = builder.bind(to_ts);
    }

    let samples = builder.bind(limit).bind(offset).fetch_all(pool).await?;

    let has_more = offset + (samples.len() as i64) < total_count;

    Ok(SampleQueryResult {
        samples,
        total_count,
        has_more,
    })
}

/// Get sample count for a stream.
pub async fn get_sample_count(
    pool: &SqlitePool,
    session_id: &str,
    stream_uid: &str,
) -> StorageResult<i64> {
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM lsl_samples WHERE session_id = ? AND stream_uid = ?")
            .bind(session_id)
            .bind(stream_uid)
            .fetch_one(pool)
            .await?;

    Ok(count.0)
}

/// Get the time range of samples for a stream.
pub async fn get_time_range(
    pool: &SqlitePool,
    session_id: &str,
    stream_uid: &str,
) -> StorageResult<Option<(f64, f64)>> {
    let result: Option<(f64, f64)> = sqlx::query_as(
        "SELECT MIN(timestamp), MAX(timestamp) FROM lsl_samples
         WHERE session_id = ? AND stream_uid = ?",
    )
    .bind(session_id)
    .bind(stream_uid)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Insert samples directly (bypassing the batcher).
pub async fn insert_samples_batch(
    pool: &SqlitePool,
    session_id: &str,
    samples: &[LslSample],
) -> StorageResult<()> {
    if samples.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    for sample in samples {
        sqlx::query(
            "INSERT INTO lsl_samples (session_id, stream_uid, timestamp, channel_data)
             VALUES (?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(&sample.stream_uid)
        .bind(sample.timestamp)
        .bind(&sample.channel_data)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        crate::storage::schema::run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_register_and_get_stream() {
        let pool = create_test_pool().await;

        // Create a session first
        sqlx::query("INSERT INTO sessions (id, started_at) VALUES (?, ?)")
            .bind("test-session")
            .bind("2024-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();

        register_stream(
            &pool,
            "test-session",
            "stream-001",
            "EEG",
            "EEG",
            16,
            256.0,
            "float32",
            Some("device-001"),
            Some("localhost"),
            None,
        )
        .await
        .unwrap();

        let stream = get_stream(&pool, "stream-001").await.unwrap().unwrap();
        assert_eq!(stream.name, "EEG");
        assert_eq!(stream.channel_count, 16);
        assert_eq!(stream.sample_rate, 256.0);
    }

    #[tokio::test]
    async fn test_sample_packing() {
        let channels = vec![1.0f32, 2.0, 3.0, 4.0];
        let sample = LslSample::from_f32("stream-001".to_string(), 1234.5, &channels);

        let unpacked = sample.to_f32();
        assert_eq!(unpacked, channels);
    }

    #[tokio::test]
    async fn test_sample_packing_f64() {
        let channels = vec![1.0f64, 2.0, 3.0, 4.0];
        let sample = LslSample::from_f64("stream-001".to_string(), 1234.5, &channels);

        let unpacked = sample.to_f64();
        assert_eq!(unpacked, channels);
    }

    #[tokio::test]
    async fn test_insert_and_query_samples() {
        let pool = create_test_pool().await;

        // Create session and stream
        sqlx::query("INSERT INTO sessions (id, started_at) VALUES (?, ?)")
            .bind("test-session")
            .bind("2024-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();

        register_stream(
            &pool,
            "test-session",
            "stream-001",
            "Test",
            "Test",
            4,
            100.0,
            "float32",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Insert samples
        let samples: Vec<LslSample> = (0..10)
            .map(|i| LslSample::from_f32("stream-001".to_string(), i as f64 * 0.01, &[i as f32; 4]))
            .collect();

        insert_samples_batch(&pool, "test-session", &samples)
            .await
            .unwrap();

        // Query samples
        let result = query_samples(
            &pool,
            SampleQueryOptions {
                session_id: "test-session".to_string(),
                stream_uid: "stream-001".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.total_count, 10);
        assert_eq!(result.samples.len(), 10);
    }

    #[tokio::test]
    async fn test_time_range_query() {
        let pool = create_test_pool().await;

        sqlx::query("INSERT INTO sessions (id, started_at) VALUES (?, ?)")
            .bind("test-session")
            .bind("2024-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();

        register_stream(
            &pool,
            "test-session",
            "stream-001",
            "Test",
            "Test",
            4,
            100.0,
            "float32",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Insert samples with specific timestamps
        let samples: Vec<LslSample> = (0..100)
            .map(|i| LslSample::from_f32("stream-001".to_string(), i as f64, &[1.0; 4]))
            .collect();

        insert_samples_batch(&pool, "test-session", &samples)
            .await
            .unwrap();

        // Query specific time range
        let result = query_samples(
            &pool,
            SampleQueryOptions {
                session_id: "test-session".to_string(),
                stream_uid: "stream-001".to_string(),
                from_timestamp: Some(25.0),
                to_timestamp: Some(75.0),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.total_count, 51); // 25 to 75 inclusive
    }

    #[tokio::test]
    async fn test_sample_batcher() {
        let pool = create_test_pool().await;

        sqlx::query("INSERT INTO sessions (id, started_at) VALUES (?, ?)")
            .bind("test-session")
            .bind("2024-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();

        register_stream(
            &pool,
            "test-session",
            "stream-001",
            "Test",
            "Test",
            4,
            100.0,
            "float32",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let config = SampleBatcherConfig {
            batch_size: 10,
            flush_interval: Duration::from_secs(60),
        };
        let batcher = SampleBatcher::new(pool.clone(), config);

        // Add samples below threshold
        for i in 0..9 {
            let sample = LslSample::from_f32("stream-001".to_string(), i as f64, &[1.0; 4]);
            batcher.add("test-session".to_string(), sample).await;
        }

        assert_eq!(batcher.buffered_count().await, 9);

        // Add one more to trigger flush
        let sample = LslSample::from_f32("stream-001".to_string(), 9.0, &[1.0; 4]);
        batcher.add("test-session".to_string(), sample).await;

        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(batcher.buffered_count().await, 0);

        // Verify samples in database
        let count = get_sample_count(&pool, "test-session", "stream-001")
            .await
            .unwrap();
        assert_eq!(count, 10);
    }
}
