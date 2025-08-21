//! Configuration management for the sidecar collector

use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Name of the service being monitored
    pub service_name: String,

    /// Kubernetes pod name
    pub pod_name: String,

    /// Kubernetes namespace
    pub namespace: String,

    /// URL of the telemetry gateway
    pub gateway_url: String,

    /// Path to application log files
    pub log_paths: Vec<String>,

    /// Batch size for telemetry data
    pub batch_size: usize,

    /// Flush interval for buffered data
    pub flush_interval: Duration,

    /// Maximum retry attempts for failed transmissions
    pub max_retries: u32,

    /// Retry backoff multiplier
    pub retry_backoff_ms: u64,

    /// Maximum buffer size in memory
    pub max_buffer_size: usize,

    /// HTTP timeout for gateway requests
    pub http_timeout: Duration,

    /// Enable structured log parsing
    pub parse_structured_logs: bool,

    /// Enable trace correlation
    pub enable_trace_correlation: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            service_name: "unknown-service".to_string(),
            pod_name: "unknown-pod".to_string(),
            namespace: "default".to_string(),
            gateway_url: "http://telemetry-gateway:9090".to_string(),
            log_paths: vec!["/var/log/app/application.log".to_string()],
            batch_size: 100,
            flush_interval: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff_ms: 1000,
            max_buffer_size: 10000,
            http_timeout: Duration::from_secs(10),
            parse_structured_logs: true,
            enable_trace_correlation: true,
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Config::default();

        if let Ok(service_name) = env::var("SERVICE_NAME") {
            config.service_name = service_name;
        }

        if let Ok(pod_name) = env::var("POD_NAME") {
            config.pod_name = pod_name;
        }

        if let Ok(namespace) = env::var("NAMESPACE") {
            config.namespace = namespace;
        }

        if let Ok(gateway_url) = env::var("GATEWAY_URL") {
            config.gateway_url = gateway_url;
        }

        if let Ok(log_paths) = env::var("LOG_PATHS") {
            config.log_paths = log_paths
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }

        if let Ok(batch_size) = env::var("BATCH_SIZE") {
            if let Ok(size) = batch_size.parse() {
                config.batch_size = size;
            }
        }

        if let Ok(flush_interval) = env::var("FLUSH_INTERVAL_SECONDS") {
            if let Ok(seconds) = flush_interval.parse::<u64>() {
                config.flush_interval = Duration::from_secs(seconds);
            }
        }

        if let Ok(max_retries) = env::var("MAX_RETRIES") {
            if let Ok(retries) = max_retries.parse() {
                config.max_retries = retries;
            }
        }

        if let Ok(backoff) = env::var("RETRY_BACKOFF_MS") {
            if let Ok(ms) = backoff.parse() {
                config.retry_backoff_ms = ms;
            }
        }

        if let Ok(buffer_size) = env::var("MAX_BUFFER_SIZE") {
            if let Ok(size) = buffer_size.parse() {
                config.max_buffer_size = size;
            }
        }

        if let Ok(timeout) = env::var("HTTP_TIMEOUT_SECONDS") {
            if let Ok(seconds) = timeout.parse::<u64>() {
                config.http_timeout = Duration::from_secs(seconds);
            }
        }

        if let Ok(parse_structured) = env::var("PARSE_STRUCTURED_LOGS") {
            config.parse_structured_logs = parse_structured.to_lowercase() == "true";
        }

        if let Ok(enable_tracing) = env::var("ENABLE_TRACE_CORRELATION") {
            config.enable_trace_correlation = enable_tracing.to_lowercase() == "true";
        }

        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.service_name.is_empty() {
            return Err("service_name cannot be empty".to_string());
        }

        if self.pod_name.is_empty() {
            return Err("pod_name cannot be empty".to_string());
        }

        if self.namespace.is_empty() {
            return Err("namespace cannot be empty".to_string());
        }

        if self.gateway_url.is_empty() {
            return Err("gateway_url cannot be empty".to_string());
        }

        if self.log_paths.is_empty() {
            return Err("at least one log path must be specified".to_string());
        }

        if self.batch_size == 0 {
            return Err("batch_size must be greater than 0".to_string());
        }

        if self.max_buffer_size == 0 {
            return Err("max_buffer_size must be greater than 0".to_string());
        }

        Ok(())
    }
}
