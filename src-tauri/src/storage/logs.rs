//! Log storage operations with batching for efficient writes.
//!
//! This module provides:
//! - `LogBatcher`: Accumulates logs and flushes them to the database in batches
//! - Query functions with filtering, pagination, and search

use super::StorageResult;
use crate::logging::LogEntry;
use sqlx::SqlitePool;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error};

/// Configuration for the log batcher.
#[derive(Debug, Clone)]
pub struct LogBatcherConfig {
    /// Number of logs to accumulate before flushing.
    pub batch_size: usize,
    /// Maximum time between flushes.
    pub flush_interval: Duration,
}

impl Default for LogBatcherConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            flush_interval: Duration::from_secs(5),
        }
    }
}

/// Batches log entries for efficient database writes.
///
/// Logs are accumulated in memory and periodically flushed to the database,
/// reducing the overhead of individual insert operations.
pub struct LogBatcher {
    buffer: RwLock<Vec<LogEntry>>,
    config: LogBatcherConfig,
    last_flush: RwLock<Instant>,
    pool: SqlitePool,
}

impl LogBatcher {
    /// Create a new log batcher with the given configuration.
    pub fn new(pool: SqlitePool, config: LogBatcherConfig) -> Self {
        Self {
            buffer: RwLock::new(Vec::with_capacity(config.batch_size)),
            config,
            last_flush: RwLock::new(Instant::now()),
            pool,
        }
    }

    /// Add a log entry to the batch.
    ///
    /// May trigger a flush if the batch is full or the flush interval has elapsed.
    pub async fn add(&self, entry: LogEntry) {
        let should_flush = {
            let mut buffer = self.buffer.write().await;
            buffer.push(entry);

            let last_flush = *self.last_flush.read().await;
            buffer.len() >= self.config.batch_size
                || last_flush.elapsed() >= self.config.flush_interval
        };

        if should_flush {
            self.flush().await;
        }
    }

    /// Add multiple log entries to the batch.
    pub async fn add_batch(&self, entries: Vec<LogEntry>) {
        if entries.is_empty() {
            return;
        }

        let should_flush = {
            let mut buffer = self.buffer.write().await;
            buffer.extend(entries);

            let last_flush = *self.last_flush.read().await;
            buffer.len() >= self.config.batch_size
                || last_flush.elapsed() >= self.config.flush_interval
        };

        if should_flush {
            self.flush().await;
        }
    }

    /// Force flush all buffered logs to the database.
    pub async fn flush(&self) {
        let entries = {
            let mut buffer = self.buffer.write().await;
            if buffer.is_empty() {
                return;
            }
            std::mem::take(&mut *buffer)
        };

        let count = entries.len();

        // Session ID will be set during insert if storage is available

        if let Err(e) = self.insert_batch(&entries, None).await {
            error!("Failed to flush logs to database: {}", e);
            // Re-add entries to buffer on failure (with size limit to prevent memory issues)
            let mut buffer = self.buffer.write().await;
            let remaining_capacity = self.config.batch_size.saturating_sub(buffer.len());
            buffer.extend(entries.into_iter().take(remaining_capacity));
        } else {
            debug!("Flushed {} logs to database", count);
            *self.last_flush.write().await = Instant::now();
        }
    }

    /// Insert a batch of log entries into the database.
    async fn insert_batch(
        &self,
        entries: &[LogEntry],
        session_id: Option<&str>,
    ) -> StorageResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        // Use a transaction for batch insert
        let mut tx = self.pool.begin().await?;

        for entry in entries {
            sqlx::query(
                "INSERT INTO logs (session_id, timestamp, level, message, device, source)
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(session_id)
            .bind(&entry.timestamp)
            .bind(&entry.level)
            .bind(&entry.message)
            .bind(&entry.device)
            .bind(&entry.source)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get the number of entries currently buffered.
    pub async fn buffered_count(&self) -> usize {
        self.buffer.read().await.len()
    }
}

/// Options for querying logs.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LogQueryOptions {
    /// Maximum number of logs to return.
    pub limit: Option<i64>,
    /// Number of logs to skip (for pagination).
    pub offset: Option<i64>,
    /// Filter by log level (exact match).
    pub level: Option<String>,
    /// Filter by device (exact match).
    pub device: Option<String>,
    /// Search term to match against message (case-insensitive LIKE).
    pub search: Option<String>,
    /// Filter logs after this timestamp (ISO 8601).
    pub from_timestamp: Option<String>,
    /// Filter logs before this timestamp (ISO 8601).
    pub to_timestamp: Option<String>,
    /// Filter by session ID.
    pub session_id: Option<String>,
}

/// Result of a paginated log query.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogQueryResult {
    /// The log entries.
    pub logs: Vec<StoredLogEntry>,
    /// Total count of matching logs (before pagination).
    pub total_count: i64,
    /// Whether there are more logs available.
    pub has_more: bool,
}

/// A log entry as stored in the database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct StoredLogEntry {
    pub id: i64,
    pub session_id: Option<String>,
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub device: Option<String>,
    pub source: String,
}

impl From<StoredLogEntry> for LogEntry {
    fn from(stored: StoredLogEntry) -> Self {
        LogEntry {
            timestamp: stored.timestamp,
            level: stored.level,
            message: stored.message,
            device: stored.device,
            source: stored.source,
        }
    }
}

/// Query logs from the database with optional filtering and pagination.
///
/// Uses parameterized queries to prevent SQL injection attacks.
/// All filter parameters are properly bound as query parameters.
pub async fn query_logs(pool: &SqlitePool, options: LogQueryOptions) -> StorageResult<LogQueryResult> {
    let limit = options.limit.unwrap_or(100).clamp(1, 10000);
    let offset = options.offset.unwrap_or(0).max(0);

    // Prepare search pattern if provided (escape SQL LIKE wildcards)
    let search_pattern = options.search.as_ref().map(|s| {
        let escaped = s
            .replace('\\', "\\\\")  // Escape backslash first
            .replace('%', "\\%")     // Escape percent
            .replace('_', "\\_");    // Escape underscore
        format!("%{}%", escaped)
    });

    // Build query with optional filters using COALESCE/NULL pattern
    // This approach uses parameterized queries throughout
    let count_query = r#"
        SELECT COUNT(*) FROM logs
        WHERE (? IS NULL OR level = ?)
          AND (? IS NULL OR device = ?)
          AND (? IS NULL OR message LIKE ? ESCAPE '\')
          AND (? IS NULL OR timestamp >= ?)
          AND (? IS NULL OR timestamp <= ?)
          AND (? IS NULL OR session_id = ?)
    "#;

    let total_count: (i64,) = sqlx::query_as(count_query)
        // level filter
        .bind(options.level.as_ref())
        .bind(options.level.as_ref())
        // device filter
        .bind(options.device.as_ref())
        .bind(options.device.as_ref())
        // search filter
        .bind(search_pattern.as_ref())
        .bind(search_pattern.as_ref())
        // from_timestamp filter
        .bind(options.from_timestamp.as_ref())
        .bind(options.from_timestamp.as_ref())
        // to_timestamp filter
        .bind(options.to_timestamp.as_ref())
        .bind(options.to_timestamp.as_ref())
        // session_id filter
        .bind(options.session_id.as_ref())
        .bind(options.session_id.as_ref())
        .fetch_one(pool)
        .await?;

    // Get paginated results (newest first)
    let select_query = r#"
        SELECT id, session_id, timestamp, level, message, device, source
        FROM logs
        WHERE (? IS NULL OR level = ?)
          AND (? IS NULL OR device = ?)
          AND (? IS NULL OR message LIKE ? ESCAPE '\')
          AND (? IS NULL OR timestamp >= ?)
          AND (? IS NULL OR timestamp <= ?)
          AND (? IS NULL OR session_id = ?)
        ORDER BY timestamp DESC
        LIMIT ? OFFSET ?
    "#;

    let logs: Vec<StoredLogEntry> = sqlx::query_as(select_query)
        // level filter
        .bind(options.level.as_ref())
        .bind(options.level.as_ref())
        // device filter
        .bind(options.device.as_ref())
        .bind(options.device.as_ref())
        // search filter
        .bind(search_pattern.as_ref())
        .bind(search_pattern.as_ref())
        // from_timestamp filter
        .bind(options.from_timestamp.as_ref())
        .bind(options.from_timestamp.as_ref())
        // to_timestamp filter
        .bind(options.to_timestamp.as_ref())
        .bind(options.to_timestamp.as_ref())
        // session_id filter
        .bind(options.session_id.as_ref())
        .bind(options.session_id.as_ref())
        // pagination
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let has_more = offset + (logs.len() as i64) < total_count.0;

    Ok(LogQueryResult {
        logs,
        total_count: total_count.0,
        has_more,
    })
}

/// Get the most recent logs (for real-time display).
pub async fn get_recent_logs(pool: &SqlitePool, limit: i64) -> StorageResult<Vec<StoredLogEntry>> {
    let logs = sqlx::query_as::<_, StoredLogEntry>(
        "SELECT id, session_id, timestamp, level, message, device, source
         FROM logs ORDER BY timestamp DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(logs)
}

/// Get log count by level (for dashboard statistics).
pub async fn get_log_counts_by_level(pool: &SqlitePool) -> StorageResult<Vec<(String, i64)>> {
    let counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT level, COUNT(*) as count FROM logs GROUP BY level"
    )
    .fetch_all(pool)
    .await?;

    Ok(counts)
}

/// Insert a single log entry directly (bypassing the batcher).
pub async fn insert_log(pool: &SqlitePool, entry: &LogEntry, session_id: Option<&str>) -> StorageResult<i64> {
    let result = sqlx::query(
        "INSERT INTO logs (session_id, timestamp, level, message, device, source)
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(&entry.timestamp)
    .bind(&entry.level)
    .bind(&entry.message)
    .bind(&entry.device)
    .bind(&entry.source)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Batch insert logs directly (bypassing the batcher).
pub async fn insert_logs_batch(
    pool: &SqlitePool,
    entries: &[LogEntry],
    session_id: Option<&str>,
) -> StorageResult<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    for entry in entries {
        sqlx::query(
            "INSERT INTO logs (session_id, timestamp, level, message, device, source)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(session_id)
        .bind(&entry.timestamp)
        .bind(&entry.level)
        .bind(&entry.message)
        .bind(&entry.device)
        .bind(&entry.source)
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

        // Run migrations
        crate::storage::schema::run_migrations(&pool).await.unwrap();

        pool
    }

    fn create_test_entry(level: &str, message: &str) -> LogEntry {
        LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message: message.to_string(),
            device: None,
            source: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_query_logs() {
        let pool = create_test_pool().await;

        // Insert some logs
        let entries = vec![
            create_test_entry("info", "Test message 1"),
            create_test_entry("warn", "Test warning"),
            create_test_entry("error", "Test error"),
        ];

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Query all logs
        let result = query_logs(&pool, LogQueryOptions::default()).await.unwrap();
        assert_eq!(result.total_count, 3);
        assert_eq!(result.logs.len(), 3);
    }

    #[tokio::test]
    async fn test_filter_by_level() {
        let pool = create_test_pool().await;

        let entries = vec![
            create_test_entry("info", "Info message"),
            create_test_entry("warn", "Warning message"),
            create_test_entry("error", "Error message"),
        ];

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Filter by level
        let result = query_logs(&pool, LogQueryOptions {
            level: Some("error".to_string()),
            ..Default::default()
        }).await.unwrap();

        assert_eq!(result.total_count, 1);
        assert_eq!(result.logs[0].level, "error");
    }

    #[tokio::test]
    async fn test_search_logs() {
        let pool = create_test_pool().await;

        let entries = vec![
            create_test_entry("info", "Connection established"),
            create_test_entry("info", "Data received"),
            create_test_entry("error", "Connection failed"),
        ];

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Search for "Connection"
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("Connection".to_string()),
            ..Default::default()
        }).await.unwrap();

        assert_eq!(result.total_count, 2);
    }

    #[tokio::test]
    async fn test_pagination() {
        let pool = create_test_pool().await;

        // Insert 10 logs
        let entries: Vec<LogEntry> = (0..10)
            .map(|i| create_test_entry("info", &format!("Message {}", i)))
            .collect();

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Query with limit and offset
        let result = query_logs(&pool, LogQueryOptions {
            limit: Some(3),
            offset: Some(2),
            ..Default::default()
        }).await.unwrap();

        assert_eq!(result.total_count, 10);
        assert_eq!(result.logs.len(), 3);
        assert!(result.has_more);
    }

    #[tokio::test]
    async fn test_log_batcher() {
        let pool = create_test_pool().await;
        let config = LogBatcherConfig {
            batch_size: 5,
            flush_interval: Duration::from_secs(60),
        };
        let batcher = LogBatcher::new(pool.clone(), config);

        // Add 4 entries (below threshold)
        for i in 0..4 {
            batcher.add(create_test_entry("info", &format!("Msg {}", i))).await;
        }

        assert_eq!(batcher.buffered_count().await, 4);

        // Add one more to trigger flush
        batcher.add(create_test_entry("info", "Msg 4")).await;

        // Give flush a moment to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Buffer should be empty after flush
        assert_eq!(batcher.buffered_count().await, 0);

        // Verify logs are in database
        let result = query_logs(&pool, LogQueryOptions::default()).await.unwrap();
        assert_eq!(result.total_count, 5);
    }

    #[tokio::test]
    async fn test_get_recent_logs() {
        let pool = create_test_pool().await;

        let entries: Vec<LogEntry> = (0..10)
            .map(|i| create_test_entry("info", &format!("Message {}", i)))
            .collect();

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        let recent = get_recent_logs(&pool, 5).await.unwrap();
        assert_eq!(recent.len(), 5);
    }

    #[tokio::test]
    async fn test_log_counts_by_level() {
        let pool = create_test_pool().await;

        let entries = vec![
            create_test_entry("info", "Info 1"),
            create_test_entry("info", "Info 2"),
            create_test_entry("warn", "Warning"),
            create_test_entry("error", "Error"),
        ];

        insert_logs_batch(&pool, &entries, None).await.unwrap();

        let counts = get_log_counts_by_level(&pool).await.unwrap();
        let counts_map: std::collections::HashMap<_, _> = counts.into_iter().collect();

        assert_eq!(counts_map.get("info"), Some(&2));
        assert_eq!(counts_map.get("warn"), Some(&1));
        assert_eq!(counts_map.get("error"), Some(&1));
    }

    #[tokio::test]
    async fn test_sql_injection_prevention() {
        let pool = create_test_pool().await;

        // Insert a test entry
        let entries = vec![create_test_entry("info", "Normal message")];
        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Attempt SQL injection through various parameters
        // These should NOT cause errors or return unexpected results

        // SQL injection attempt in level filter
        let result = query_logs(&pool, LogQueryOptions {
            level: Some("info' OR '1'='1".to_string()),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total_count, 0); // Should find nothing, not all entries

        // SQL injection attempt in search
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("'; DROP TABLE logs; --".to_string()),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total_count, 0); // Should find nothing

        // Verify table still exists
        let result = query_logs(&pool, LogQueryOptions::default()).await.unwrap();
        assert_eq!(result.total_count, 1); // Original entry still exists

        // SQL injection attempt with wildcards in search
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("%".to_string()),
            ..Default::default()
        }).await.unwrap();
        // Should treat % literally, not as wildcard (escaped)
        assert_eq!(result.total_count, 0);

        // Verify normal search still works
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("Normal".to_string()),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total_count, 1);
    }

    #[tokio::test]
    async fn test_search_with_special_characters() {
        let pool = create_test_pool().await;

        // Insert entries with special characters
        let entries = vec![
            create_test_entry("info", "Message with 100% success"),
            create_test_entry("info", "Query like SELECT * FROM table"),
            create_test_entry("info", "Path: C:\\Users\\test"),
        ];
        insert_logs_batch(&pool, &entries, None).await.unwrap();

        // Search for literal %
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("100%".to_string()),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total_count, 1);

        // Search for backslash
        let result = query_logs(&pool, LogQueryOptions {
            search: Some("C:\\".to_string()),
            ..Default::default()
        }).await.unwrap();
        assert_eq!(result.total_count, 1);
    }
}
