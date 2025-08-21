//! Telemetry data structures and utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub service_name: String,
    pub pod_name: String,
    pub namespace: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub attributes: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

impl From<&str> for LogLevel {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "TRACE" | "VERBOSE" => LogLevel::Trace,
            "DEBUG" => LogLevel::Debug,
            "INFO" | "INFORMATION" => LogLevel::Info,
            "WARN" | "WARNING" => LogLevel::Warn,
            "ERROR" | "ERR" => LogLevel::Error,
            "FATAL" | "CRITICAL" => LogLevel::Fatal,
            _ => LogLevel::Info, // Default fallback
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub duration_ms: u64,
    pub status: SpanStatus,
    pub service_name: String,
    pub tags: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SpanStatus {
    Ok,
    Error,
    Timeout,
    Cancelled,
}

impl std::fmt::Display for SpanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanStatus::Ok => write!(f, "OK"),
            SpanStatus::Error => write!(f, "ERROR"),
            SpanStatus::Timeout => write!(f, "TIMEOUT"),
            SpanStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

impl From<&str> for SpanStatus {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "OK" | "SUCCESS" | "COMPLETED" => SpanStatus::Ok,
            "ERROR" | "FAILED" | "FAILURE" => SpanStatus::Error,
            "TIMEOUT" | "TIMEDOUT" => SpanStatus::Timeout,
            "CANCELLED" | "CANCELED" | "ABORTED" => SpanStatus::Cancelled,
            _ => SpanStatus::Ok, // Default fallback
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub logs: Vec<LogEntry>,
    pub spans: Vec<TraceSpan>,
    pub metadata: BatchMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchMetadata {
    pub collector_id: String,
    pub batch_id: String,
    pub timestamp: u64,
    pub source_pod: String,
    pub source_namespace: String,
    pub version: String,
}

impl LogEntry {
    pub fn new(
        level: LogLevel,
        message: String,
        service_name: String,
        pod_name: String,
        namespace: String,
    ) -> Self {
        Self {
            timestamp: current_timestamp(),
            level,
            message,
            service_name,
            pod_name,
            namespace,
            trace_id: None,
            span_id: None,
            attributes: HashMap::new(),
        }
    }

    pub fn with_trace_context(mut self, trace_id: String, span_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = Some(span_id);
        self
    }

    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }

    pub fn with_attributes(mut self, attributes: HashMap<String, String>) -> Self {
        self.attributes.extend(attributes);
        self
    }
}

impl TraceSpan {
    pub fn new(
        trace_id: String,
        span_id: String,
        operation_name: String,
        service_name: String,
    ) -> Self {
        let now = current_timestamp();
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
            operation_name,
            start_time: now,
            end_time: now,
            duration_ms: 0,
            status: SpanStatus::Ok,
            service_name,
            tags: HashMap::new(),
        }
    }

    pub fn with_parent(mut self, parent_span_id: String) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }

    pub fn with_status(mut self, status: SpanStatus) -> Self {
        self.status = status;
        self
    }

    pub fn finish(mut self) -> Self {
        self.end_time = current_timestamp();
        self.duration_ms = self.end_time.saturating_sub(self.start_time) * 1000;
        self
    }

    pub fn set_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self.end_time = self.start_time + (duration_ms / 1000);
        self
    }
}

impl TelemetryBatch {
    pub fn new(
        logs: Vec<LogEntry>,
        spans: Vec<TraceSpan>,
        collector_id: String,
        source_pod: String,
        source_namespace: String,
    ) -> Self {
        Self {
            logs,
            spans,
            metadata: BatchMetadata {
                collector_id,
                batch_id: Uuid::new_v4().to_string(),
                timestamp: current_timestamp(),
                source_pod,
                source_namespace,
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.logs.is_empty() && self.spans.is_empty()
    }

    pub fn len(&self) -> usize {
        self.logs.len() + self.spans.len()
    }
}

/// Generate a new trace ID
pub fn generate_trace_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

/// Generate a new span ID
pub fn generate_span_id() -> String {
    format!("{:016x}", rand::random::<u64>())
}

/// Get current timestamp in seconds since Unix epoch
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::from("error"), LogLevel::Error);
        assert_eq!(LogLevel::from("DEBUG"), LogLevel::Debug);
        assert_eq!(LogLevel::from("unknown"), LogLevel::Info);
    }

    #[test]
    fn test_span_status_from_str() {
        assert_eq!(SpanStatus::from("OK"), SpanStatus::Ok);
        assert_eq!(SpanStatus::from("error"), SpanStatus::Error);
        assert_eq!(SpanStatus::from("TIMEOUT"), SpanStatus::Timeout);
        assert_eq!(SpanStatus::from("unknown"), SpanStatus::Ok);
    }

    #[test]
    fn test_log_entry_creation() {
        let log = LogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            "test-service".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        assert_eq!(log.level, LogLevel::Info);
        assert_eq!(log.message, "Test message");
        assert_eq!(log.service_name, "test-service");
        assert!(log.trace_id.is_none());
    }

    #[test]
    fn test_trace_span_creation() {
        let span = TraceSpan::new(
            "trace-123".to_string(),
            "span-456".to_string(),
            "test-operation".to_string(),
            "test-service".to_string(),
        );

        assert_eq!(span.trace_id, "trace-123");
        assert_eq!(span.span_id, "span-456");
        assert_eq!(span.operation_name, "test-operation");
        assert!(span.parent_span_id.is_none());
    }

    #[test]
    fn test_telemetry_batch_creation() {
        let logs = vec![LogEntry::new(
            LogLevel::Info,
            "Test".to_string(),
            "service".to_string(),
            "pod".to_string(),
            "namespace".to_string(),
        )];

        let batch = TelemetryBatch::new(
            logs,
            vec![],
            "collector-1".to_string(),
            "test-pod".to_string(),
            "test-namespace".to_string(),
        );

        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());
        assert_eq!(batch.metadata.source_pod, "test-pod");
    }
}
