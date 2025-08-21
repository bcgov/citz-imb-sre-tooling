//! Log parsing utilities for various log formats

use crate::telemetry::{LogEntry, LogLevel, TraceSpan, SpanStatus, generate_trace_id, generate_span_id};
use crate::errors::{CollectorError, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Trait for parsing log lines into structured telemetry data
pub trait LogParser: Send + Sync {
    fn parse_log(&self, line: &str, service_name: &str, pod_name: &str, namespace: &str) -> Result<Option<LogEntry>>;
    fn parse_span(&self, line: &str, service_name: &str) -> Result<Option<TraceSpan>>;
}

/// JSON log parser for structured logs
pub struct JsonLogParser {
    trace_correlation: bool,
}

impl JsonLogParser {
    pub fn new(trace_correlation: bool) -> Self {
        Self { trace_correlation }
    }
}

impl LogParser for JsonLogParser {
    fn parse_log(&self, line: &str, service_name: &str, pod_name: &str, namespace: &str) -> Result<Option<LogEntry>> {
        let json: Value = serde_json::from_str(line)?;

        let timestamp = json["timestamp"]
            .as_u64()
            .or_else(|| json["@timestamp"].as_u64())
            .or_else(|| json["time"].as_u64())
            .unwrap_or_else(|| crate::telemetry::current_timestamp());

        let level = json["level"]
            .as_str()
            .or_else(|| json["severity"].as_str())
            .or_else(|| json["log_level"].as_str())
            .unwrap_or("INFO");

        let message = json["message"]
            .as_str()
            .or_else(|| json["msg"].as_str())
            .or_else(|| json["text"].as_str())
            .unwrap_or("")
            .to_string();

        if message.is_empty() {
            return Ok(None);
        }

        let mut log_entry = LogEntry {
            timestamp,
            level: LogLevel::from(level),
            message,
            service_name: service_name.to_string(),
            pod_name: pod_name.to_string(),
            namespace: namespace.to_string(),
            trace_id: None,
            span_id: None,
            attributes: HashMap::new(),
        };

        if self.trace_correlation {
            if let Some(trace_id) = json["trace_id"]
                .as_str()
                .or_else(|| json["traceId"].as_str())
                .or_else(|| json["trace-id"].as_str()) {
                log_entry.trace_id = Some(trace_id.to_string());
            }

            if let Some(span_id) = json["span_id"]
                .as_str()
                .or_else(|| json["spanId"].as_str())
                .or_else(|| json["span-id"].as_str()) {
                log_entry.span_id = Some(span_id.to_string());
            }
        }

        if let Some(attributes) = json["attributes"].as_object() {
            for (key, value) in attributes {
                if let Some(str_value) = value.as_str() {
                    log_entry.attributes.insert(key.clone(), str_value.to_string());
                }
            }
        }

        for field in ["user_id", "request_id", "session_id", "correlation_id"] {
            if let Some(value) = json[field].as_str() {
                log_entry.attributes.insert(field.to_string(), value.to_string());
            }
        }

        Ok(Some(log_entry))
    }

    fn parse_span(&self, line: &str, service_name: &str) -> Result<Option<TraceSpan>> {
        let json: Value = serde_json::from_str(line)?;

        // Only parse if this looks like a span/trace log
        if !json.get("span_id").is_some() && !json.get("spanId").is_some() {
            return Ok(None);
        }

        let trace_id = json["trace_id"]
            .as_str()
            .or_else(|| json["traceId"].as_str())
            .unwrap_or_else(|| &generate_trace_id())
            .to_string();

        let span_id = json["span_id"]
            .as_str()
            .or_else(|| json["spanId"].as_str())
            .unwrap_or_else(|| &generate_span_id())
            .to_string();

        let operation_name = json["operation"]
            .as_str()
            .or_else(|| json["operation_name"].as_str())
            .or_else(|| json["method"].as_str())
            .unwrap_or("unknown")
            .to_string();

        let start_time = json["start_time"]
            .as_u64()
            .or_else(|| json["startTime"].as_u64())
            .unwrap_or_else(|| crate::telemetry::current_timestamp());

        let end_time = json["end_time"]
            .as_u64()
            .or_else(|| json["endTime"].as_u64())
            .unwrap_or(start_time);

        let duration_ms = json["duration_ms"]
            .as_u64()
            .or_else(|| json["duration"].as_u64())
            .unwrap_or_else(|| end_time.saturating_sub(start_time) * 1000);

        let status = json["status"]
            .as_str()
            .or_else(|| json["span_status"].as_str())
            .unwrap_or("OK");

        let mut span = TraceSpan {
            trace_id,
            span_id,
            parent_span_id: json["parent_span_id"]
                .as_str()
                .or_else(|| json["parentSpanId"].as_str())
                .map(String::from),
            operation_name,
            start_time,
            end_time,
            duration_ms,
            status: SpanStatus::from(status),
            service_name: service_name.to_string(),
            tags: HashMap::new(),
        };

        if let Some(tags) = json["tags"].as_object() {
            for (key, value) in tags {
                if let Some(str_value) = value.as_str() {
                    span.tags.insert(key.clone(), str_value.to_string());
                }
            }
        }

        Ok(Some(span))
    }
}

/// Regex-based log parser for unstructured logs
pub struct RegexLogParser {
    patterns: Vec<LogPattern>,
    trace_correlation: bool,
}

struct LogPattern {
    regex: Regex,
    level_group: usize,
    message_group: usize,
    timestamp_group: Option<usize>,
    trace_id_group: Option<usize>,
    span_id_group: Option<usize>,
}

impl RegexLogParser {
    pub fn new(trace_correlation: bool) -> Self {
        Self {
            patterns: Self::default_patterns(),
            trace_correlation,
        }
    }

    pub fn with_custom_patterns(patterns: Vec<LogPattern>) -> Self {
        Self {
            patterns,
            trace_correlation: true,
        }
    }

    fn default_patterns() -> Vec<LogPattern> {
        static PATTERNS: OnceLock<Vec<LogPattern>> = OnceLock::new();
        PATTERNS.get_or_init(|| {
            vec![
                // Common application log format: [2023-12-01T10:30:45Z] INFO: Message
                LogPattern {
                    regex: Regex::new(r"^\[([^\]]+)\]\s+(\w+):\s+(.+)$").unwrap(),
                    level_group: 2,
                    message_group: 3,
                    timestamp_group: Some(1),
                    trace_id_group: None,
                    span_id_group: None,
                },
                // Nginx access log style: 2023/12/01 10:30:45 [error] Message
                LogPattern {
                    regex: Regex::new(r"^(\d{4}/\d{2}/\d{2}\s+\d{2}:\d{2}:\d{2})\s+\[(\w+)\]\s+(.+)$").unwrap(),
                    level_group: 2,
                    message_group: 3,
                    timestamp_group: Some(1),
                    trace_id_group: None,
                    span_id_group: None,
                },
                // Java/Spring Boot style: 2023-12-01 10:30:45.123 ERROR [trace-id,span-id] --- Message
                LogPattern {
                    regex: Regex::new(r"^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3})\s+(\w+)\s+\[([^,]+),([^\]]+)\]\s+---\s+(.+)$").unwrap(),
                    level_group: 2,
                    message_group: 5,
                    timestamp_group: Some(1),
                    trace_id_group: Some(3),
                    span_id_group: Some(4),
                },
                // Simple format: ERROR: Message
                LogPattern {
                    regex: Regex::new(r"^(\w+):\s+(.+)$").unwrap(),
                    level_group: 1,
                    message_group: 2,
                    timestamp_group: None,
                    trace_id_group: None,
                    span_id_group: None,
                },
                // Python logging: ERROR:module.name:Message
                LogPattern {
                    regex: Regex::new(r"^(\w+):[\w\.]+:(.+)$").unwrap(),
                    level_group: 1,
                    message_group: 2,
                    timestamp_group: None,
                    trace_id_group: None,
                    span_id_group: None,
                },
            ]
        }).clone()
    }
}

impl LogParser for RegexLogParser {
    fn parse_log(&self, line: &str, service_name: &str, pod_name: &str, namespace: &str) -> Result<Option<LogEntry>> {
        for pattern in &self.patterns {
            if let Some(captures) = pattern.regex.captures(line) {
                let level = captures.get(pattern.level_group)
                    .map(|m| m.as_str())
                    .unwrap_or("INFO");

                let message = captures.get(pattern.message_group)
                    .map(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();

                if message.is_empty() {
                    continue;
                }

                let timestamp = if let Some(ts_group) = pattern.timestamp_group {
                    captures.get(ts_group)
                        .and_then(|m| parse_timestamp(m.as_str()))
                        .unwrap_or_else(|| crate::telemetry::current_timestamp())
                } else {
                    crate::telemetry::current_timestamp()
                };

                let mut log_entry = LogEntry {
                    timestamp,
                    level: LogLevel::from(level),
                    message,
                    service_name: service_name.to_string(),
                    pod_name: pod_name.to_string(),
                    namespace: namespace.to_string(),
                    trace_id: None,
                    span_id: None,
                    attributes: HashMap::new(),
                };

                // Extract trace context if available and enabled
                if self.trace_correlation {
                    if let Some(trace_group) = pattern.trace_id_group {
                        if let Some(trace_id) = captures.get(trace_group) {
                            log_entry.trace_id = Some(trace_id.as_str().to_string());
                        }
                    }

                    if let Some(span_group) = pattern.span_id_group {
                        if let Some(span_id) = captures.get(span_group) {
                            log_entry.span_id = Some(span_id.as_str().to_string());
                        }
                    }
                }

                return Ok(Some(log_entry));
            }
        }

        Ok(Some(LogEntry {
            timestamp: crate::telemetry::current_timestamp(),
            level: LogLevel::Info,
            message: line.to_string(),
            service_name: service_name.to_string(),
            pod_name: pod_name.to_string(),
            namespace: namespace.to_string(),
            trace_id: None,
            span_id: None,
            attributes: HashMap::new(),
        }))
    }

    fn parse_span(&self, _line: &str, _service_name: &str) -> Result<Option<TraceSpan>> {
        // Regex parser doesn't extract spans from unstructured logs
        Ok(None)
    }
}

/// Combined parser that tries multiple parsing strategies
pub struct CompositeLogParser {
    json_parser: JsonLogParser,
    regex_parser: RegexLogParser,
}

impl CompositeLogParser {
    pub fn new(trace_correlation: bool) -> Self {
        Self {
            json_parser: JsonLogParser::new(trace_correlation),
            regex_parser: RegexLogParser::new(trace_correlation),
        }
    }
}

impl LogParser for CompositeLogParser {
    fn parse_log(&self, line: &str, service_name: &str, pod_name: &str, namespace: &str) -> Result<Option<LogEntry>> {
        // Try JSON parsing first
        if line.trim().starts_with('{') {
            match self.json_parser.parse_log(line, service_name, pod_name, namespace) {
                Ok(Some(log)) => return Ok(Some(log)),
                Ok(None) => {},
                Err(_) => {},
            }
        }

        // Fall back to regex parsing
        self.regex_parser.parse_log(line, service_name, pod_name, namespace)
    }

    fn parse_span(&self, line: &str, service_name: &str) -> Result<Option<TraceSpan>> {
        if line.trim().starts_with('{') {
            self.json_parser.parse_span(line, service_name)
        } else {
            Ok(None)
        }
    }
}

/// Parse various timestamp formats
fn parse_timestamp(ts_str: &str) -> Option<u64> {
    use chrono::{DateTime, NaiveDateTime};

    // Try different timestamp formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S%.fZ",      // ISO 8601 with timezone
        "%Y-%m-%dT%H:%M:%SZ",         // ISO 8601 simple
        "%Y-%m-%d %H:%M:%S%.f",       // SQL timestamp with fractional
        "%Y-%m-%d %H:%M:%S",          // SQL timestamp
        "%Y/%m/%d %H:%M:%S",          // Alternative format
        "%d/%b/%Y:%H:%M:%S %z",       // Apache log format
    ];

    for format in &formats {
        if let Ok(dt) = DateTime::parse_from_str(ts_str, format) {
            return Some(dt.timestamp() as u64);
        }
        if let Ok(dt) = NaiveDateTime::parse_from_str(ts_str, format) {
            return Some(dt.timestamp() as u64);
        }
    }

    // Try parsing as Unix timestamp
    if let Ok(timestamp) = ts_str.parse::<u64>() {
        return Some(timestamp);
    }

    None
}

/// Factory for creating log parsers
pub struct LogParserFactory;

impl LogParserFactory {
    pub fn create_parser(
        format: &str,
        trace_correlation: bool,
    ) -> Box<dyn LogParser> {
        match format.to_lowercase().as_str() {
            "json" => Box::new(JsonLogParser::new(trace_correlation)),
            "regex" => Box::new(RegexLogParser::new(trace_correlation)),
            "composite" | "auto" => Box::new(CompositeLogParser::new(trace_correlation)),
            _ => Box::new(CompositeLogParser::new(trace_correlation)), // Default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_log_parsing() {
        let parser = JsonLogParser::new(true);
        let log_line = r#"{"timestamp": 1701234567, "level": "ERROR", "message": "Test error", "trace_id": "abc123", "span_id": "def456"}"#;

        let result = parser.parse_log(log_line, "test-service", "test-pod", "test-ns").unwrap();
        assert!(result.is_some());

        let log_entry = result.unwrap();
        assert_eq!(log_entry.level, LogLevel::Error);
        assert_eq!(log_entry.message, "Test error");
        assert_eq!(log_entry.trace_id, Some("abc123".to_string()));
        assert_eq!(log_entry.span_id, Some("def456".to_string()));
    }

    #[test]
    fn test_regex_log_parsing() {
        let parser = RegexLogParser::new(false);
        let log_line = "[2023-12-01T10:30:45Z] ERROR: Database connection failed";

        let result = parser.parse_log(log_line, "test-service", "test-pod", "test-ns").unwrap();
        assert!(result.is_some());

        let log_entry = result.unwrap();
        assert_eq!(log_entry.level, LogLevel::Error);
        assert_eq!(log_entry.message, "Database connection failed");
    }

    #[test]
    fn test_composite_parser_json() {
        let parser = CompositeLogParser::new(true);
        let log_line = r#"{"level": "INFO", "message": "Test message"}"#;

        let result = parser.parse_log(log_line, "test-service", "test-pod", "test-ns").unwrap();
        assert!(result.is_some());

        let log_entry = result.unwrap();
        assert_eq!(log_entry.level, LogLevel::Info);
        assert_eq!(log_entry.message, "Test message");
    }

    #[test]
    fn test_composite_parser_regex() {
        let parser = CompositeLogParser::new(false);
        let log_line = "ERROR: Something went wrong";

        let result = parser.parse_log(log_line, "test-service", "test-pod", "test-ns").unwrap();
        assert!(result.is_some());

        let log_entry = result.unwrap();
        assert_eq!(log_entry.level, LogLevel::Error);
        assert_eq!(log_entry.message, "Something went wrong");
    }

    #[test]
    fn test_timestamp_parsing() {
        assert!(parse_timestamp("2025-01-01T10:30:45Z").is_some());
        assert!(parse_timestamp("2025-01-01 10:30:45").is_some());
        assert!(parse_timestamp("1701234567").is_some());
        assert!(parse_timestamp("invalid").is_none());
    }

    #[test]
    fn test_span_parsing() {
        let parser = JsonLogParser::new(true);
        let span_line = r#"{"trace_id": "abc123", "span_id": "def456", "operation": "database_query", "duration_ms": 150, "status": "OK"}"#;

        let result = parser.parse_span(span_line, "test-service").unwrap();
        assert!(result.is_some());

        let span = result.unwrap();
        assert_eq!(span.trace_id, "abc123");
        assert_eq!(span.span_id, "def456");
        assert_eq!(span.operation_name, "database_query");
        assert_eq!(span.duration_ms, 150);
        assert_eq!(span.status, SpanStatus::Ok);
    }
}
