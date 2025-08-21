//! In-memory buffering for telemetry data

use crate::telemetry::{LogEntry, TraceSpan, TelemetryBatch};
use crate::errors::{CollectorError, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Thread-safe buffer for telemetry data
#[derive(Debug)]
pub struct TelemetryBuffer {
    logs: Arc<RwLock<VecDeque<LogEntry>>>,
    spans: Arc<RwLock<VecDeque<TraceSpan>>>,
    max_size: usize,
    batch_size: usize,
}

impl TelemetryBuffer {
    /// Create a new telemetry buffer
    pub fn new(max_size: usize, batch_size: usize) -> Self {
        Self {
            logs: Arc::new(RwLock::new(VecDeque::new())),
            spans: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
            batch_size,
        }
    }

    /// Add a log entry to the buffer
    pub async fn add_log(&self, log_entry: LogEntry) -> Result<()> {
        let mut logs = self.logs.write().await;

        if logs.len() >= self.max_size {
            logs.pop_front();
            warn!("Log buffer overflow, dropping oldest entry");
        }

        logs.push_back(log_entry);
        debug!("Added log entry to buffer, current size: {}", logs.len());

        Ok(())
    }

    /// Add a trace span to the buffer
    pub async fn add_span(&self, span: TraceSpan) -> Result<()> {
        let mut spans = self.spans.write().await;

        if spans.len() >= self.max_size {
            spans.pop_front();
            warn!("Span buffer overflow, dropping oldest entry");
        }

        spans.push_back(span);
        debug!("Added span to buffer, current size: {}", spans.len());

        Ok(())
    }

    /// Drain a batch of telemetry data from the buffer
    pub async fn drain_batch(
        &self,
        collector_id: String,
        source_pod: String,
        source_namespace: String,
    ) -> Result<Option<TelemetryBatch>> {
        let (logs, spans) = {
            let mut log_buffer = self.logs.write().await;
            let mut span_buffer = self.spans.write().await;

            let log_count = std::cmp::min(self.batch_size, log_buffer.len());
            let span_count = std::cmp::min(self.batch_size, span_buffer.len());

            if log_count == 0 && span_count == 0 {
                return Ok(None);
            }

            let logs: Vec<LogEntry> = log_buffer.drain(..log_count).collect();
            let spans: Vec<TraceSpan> = span_buffer.drain(..span_count).collect();

            (logs, spans)
        };

        debug!(
            "Drained batch: {} logs, {} spans",
            logs.len(),
            spans.len()
        );

        Ok(Some(TelemetryBatch::new(
            logs,
            spans,
            collector_id,
            source_pod,
            source_namespace,
        )))
    }

    /// Get the current buffer sizes
    pub async fn sizes(&self) -> (usize, usize) {
        let logs = self.logs.read().await;
        let spans = self.spans.read().await;
        (logs.len(), spans.len())
    }

    /// Check if the buffer has data ready for batching
    pub async fn has_data(&self) -> bool {
        let (log_count, span_count) = self.sizes().await;
        log_count > 0 || span_count > 0
    }

    /// Check if the buffer should be flushed (has enough data or is getting full)
    pub async fn should_flush(&self) -> bool {
        let (log_count, span_count) = self.sizes().await;

        log_count >= self.batch_size
            || span_count >= self.batch_size
            || log_count >= (self.max_size * 3 / 4)
            || span_count >= (self.max_size * 3 / 4)
    }

    /// Force flush all buffered data
    pub async fn flush_all(
        &self,
        collector_id: String,
        source_pod: String,
        source_namespace: String,
    ) -> Result<Vec<TelemetryBatch>> {
        let mut batches = Vec::new();

        while let Some(batch) = self.drain_batch(
            collector_id.clone(),
            source_pod.clone(),
            source_namespace.clone(),
        ).await? {
            batches.push(batch);
        }

        debug!("Flushed {} batches from buffer", batches.len());
        Ok(batches)
    }

    /// Clear all buffered data
    pub async fn clear(&self) {
        let mut logs = self.logs.write().await;
        let mut spans = self.spans.write().await;

        logs.clear();
        spans.clear();

        debug!("Cleared all buffered data");
    }

    /// Get buffer utilization as a percentage
    pub async fn utilization(&self) -> f64 {
        let (log_count, span_count) = self.sizes().await;
        let total_used = log_count + span_count;
        let total_capacity = self.max_size * 2; // logs + spans

        (total_used as f64 / total_capacity as f64) * 100.0
    }
}

/// Configuration for buffer behavior
#[derive(Debug, Clone)]
pub struct BufferConfig {
    pub max_size: usize,
    pub batch_size: usize,
    pub flush_threshold: f64,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            batch_size: 100,
            flush_threshold: 75.0,
        }
    }
}

/// A more advanced buffer with priority handling
#[derive(Debug)]
pub struct PriorityTelemetryBuffer {
    high_priority: TelemetryBuffer,
    normal_priority: TelemetryBuffer,
    config: BufferConfig,
}

impl PriorityTelemetryBuffer {
    pub fn new(config: BufferConfig) -> Self {
        Self {
            high_priority: TelemetryBuffer::new(
                config.max_size / 4,
                config.batch_size / 2,
            ),
            normal_priority: TelemetryBuffer::new(
                config.max_size * 3 / 4,
                config.batch_size,
            ),
            config,
        }
    }

    /// Add a log entry with priority
    pub async fn add_log(&self, log_entry: LogEntry, high_priority: bool) -> Result<()> {
        if high_priority {
            self.high_priority.add_log(log_entry).await
        } else {
            self.normal_priority.add_log(log_entry).await
        }
    }

    /// Add a span with priority
    pub async fn add_span(&self, span: TraceSpan, high_priority: bool) -> Result<()> {
        if high_priority {
            self.high_priority.add_span(span).await
        } else {
            self.normal_priority.add_span(span).await
        }
    }

    /// Drain a batch, prioritizing high-priority data
    pub async fn drain_batch(
        &self,
        collector_id: String,
        source_pod: String,
        source_namespace: String,
    ) -> Result<Option<TelemetryBatch>> {
        if let Some(batch) = self.high_priority.drain_batch(
            collector_id.clone(),
            source_pod.clone(),
            source_namespace.clone(),
        ).await? {
            return Ok(Some(batch));
        }

        self.normal_priority.drain_batch(collector_id, source_pod, source_namespace).await
    }

    /// Check if should flush any buffer
    pub async fn should_flush(&self) -> bool {
        self.high_priority.should_flush().await || self.normal_priority.should_flush().await
    }

    /// Get combined buffer statistics
    pub async fn stats(&self) -> BufferStats {
        let (hp_logs, hp_spans) = self.high_priority.sizes().await;
        let (np_logs, np_spans) = self.normal_priority.sizes().await;

        BufferStats {
            high_priority_logs: hp_logs,
            high_priority_spans: hp_spans,
            normal_priority_logs: np_logs,
            normal_priority_spans: np_spans,
            total_logs: hp_logs + np_logs,
            total_spans: hp_spans + np_spans,
            utilization: self.utilization().await,
        }
    }

    async fn utilization(&self) -> f64 {
        let stats = self.stats().await;
        let total_used = stats.total_logs + stats.total_spans;
        let total_capacity = self.config.max_size * 2;

        (total_used as f64 / total_capacity as f64) * 100.0
    }
}

/// Buffer statistics
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub high_priority_logs: usize,
    pub high_priority_spans: usize,
    pub normal_priority_logs: usize,
    pub normal_priority_spans: usize,
    pub total_logs: usize,
    pub total_spans: usize,
    pub utilization: f64,
}

/// Helper function to determine if a log entry should be high priority
pub fn is_high_priority_log(log_entry: &LogEntry) -> bool {
    use crate::telemetry::LogLevel;

    matches!(log_entry.level, LogLevel::Error | LogLevel::Fatal)
        || log_entry.message.to_lowercase().contains("critical")
        || log_entry.message.to_lowercase().contains("security")
        || log_entry.message.to_lowercase().contains("alert")
}

/// Helper function to determine if a span should be high priority
pub fn is_high_priority_span(span: &TraceSpan) -> bool {
    use crate::telemetry::SpanStatus;

    matches!(span.status, SpanStatus::Error | SpanStatus::Timeout)
        || span.duration_ms > 10000 // Spans longer than 10 seconds
        || span.tags.values().any(|v| {
            v.to_lowercase().contains("error")
                || v.to_lowercase().contains("timeout")
                || v.to_lowercase().contains("critical")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{LogLevel, SpanStatus};

    #[tokio::test]
    async fn test_basic_buffer_operations() {
        let buffer = TelemetryBuffer::new(100, 10);

        let log_entry = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        buffer.add_log(log_entry).await.unwrap();

        let (log_count, span_count) = buffer.sizes().await;
        assert_eq!(log_count, 1);
        assert_eq!(span_count, 0);

        let batch = buffer.drain_batch(
            "collector-1".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        ).await.unwrap();

        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.logs.len(), 1);
        assert_eq!(batch.spans.len(), 0);

        let (log_count, span_count) = buffer.sizes().await;
        assert_eq!(log_count, 0);
        assert_eq!(span_count, 0);
    }

    #[tokio::test]
    async fn test_buffer_overflow() {
        let buffer = TelemetryBuffer::new(2, 10); // Very small buffer

        for i in 0..5 {
            let log_entry = LogEntry::new(
                LogLevel::Info,
                format!("Message {}", i),
                "test-service".to_string(),
                "test-pod".to_string(),
                "test-namespace".to_string(),
            );
            buffer.add_log(log_entry).await.unwrap();
        }

        let (log_count, _) = buffer.sizes().await;
        assert_eq!(log_count, 2); // Should be limited to max_size
    }

    #[tokio::test]
    async fn test_priority_buffer() {
        let config = BufferConfig::default();
        let buffer = PriorityTelemetryBuffer::new(config);

        let normal_log = LogEntry::new(
            LogLevel::Info,
            "Normal message".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        let error_log = LogEntry::new(
            LogLevel::Error,
            "Error message".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        buffer.add_log(normal_log, false).await.unwrap();
        buffer.add_log(error_log, true).await.unwrap();

        let stats = buffer.stats().await;
        assert_eq!(stats.normal_priority_logs, 1);
        assert_eq!(stats.high_priority_logs, 1);

        // High priority should be drained first
        let batch = buffer.drain_batch(
            "collector-1".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        ).await.unwrap();

        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.logs.len(), 1);
        assert_eq!(batch.logs[0].message, "Error message");
    }

    #[test]
    fn test_priority_detection() {
        let error_log = LogEntry::new(
            LogLevel::Error,
            "Database error".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        let info_log = LogEntry::new(
            LogLevel::Info,
            "Normal operation".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        assert!(is_high_priority_log(&error_log));
        assert!(!is_high_priority_log(&info_log));
    }
}
