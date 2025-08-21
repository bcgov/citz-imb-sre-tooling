//! HTTP transport layer for sending telemetry data to the gateway

use crate::telemetry::TelemetryBatch;
use crate::errors::{CollectorError, Result};
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{debug, warn, error, info};

/// HTTP transport for telemetry data
#[derive(Debug, Clone)]
pub struct HttpTransport {
    client: Client,
    gateway_url: String,
    timeout: Duration,
    max_retries: u32,
    retry_backoff_ms: u64,
}

impl HttpTransport {
    /// Create a new HTTP transport
    pub fn new(
        gateway_url: String,
        http_timeout: Duration,
        max_retries: u32,
        retry_backoff_ms: u64,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(http_timeout)
            .user_agent(format!("opentel_collector/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(CollectorError::Http)?;

        Ok(Self {
            client,
            gateway_url,
            timeout: http_timeout,
            max_retries,
            retry_backoff_ms,
        })
    }

    /// Send a telemetry batch to the gateway
    pub async fn send_batch(&self, batch: TelemetryBatch) -> Result<()> {
        let url = format!("{}/v1/telemetry", self.gateway_url);

        debug!(
            "Sending batch {} with {} logs and {} spans to {}",
            batch.metadata.batch_id,
            batch.logs.len(),
            batch.spans.len(),
            url
        );

        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= self.max_retries {
            match self.send_batch_attempt(&url, &batch).await {
                Ok(_) => {
                    info!(
                        "Successfully sent batch {} (attempt {})",
                        batch.metadata.batch_id,
                        attempt + 1
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    attempt += 1;

                    if attempt <= self.max_retries {
                        let backoff_ms = self.retry_backoff_ms * (2_u64.pow(attempt - 1));
                        warn!(
                            "Failed to send batch {} (attempt {}), retrying in {}ms: {}",
                            batch.metadata.batch_id,
                            attempt,
                            backoff_ms,
                            last_error.as_ref().unwrap()
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        let final_error = last_error.unwrap_or(CollectorError::Other(
            "All retry attempts failed".to_string()
        ));

        error!(
            "Failed to send batch {} after {} attempts: {}",
            batch.metadata.batch_id,
            self.max_retries + 1,
            final_error
        );

        Err(final_error)
    }

    /// Single attempt to send a batch
    async fn send_batch_attempt(&self, url: &str, batch: &TelemetryBatch) -> Result<()> {
        let response = timeout(
            self.timeout,
            self.client.post(url).json(batch).send()
        ).await
        .map_err(|_| CollectorError::Transport("Request timeout".to_string()))?
        .map_err(CollectorError::Http)?;

        self.handle_response(response, &batch.metadata.batch_id).await
    }

    /// Handle the HTTP response from the gateway
    async fn handle_response(&self, response: Response, batch_id: &str) -> Result<()> {
        let status = response.status();

        if status.is_success() {
            debug!("Batch {} accepted by gateway", batch_id);
            return Ok(());
        }

        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

        let error_message = match status.as_u16() {
            400 => format!("Bad request for batch {}: {}", batch_id, error_body),
            401 => format!("Unauthorized for batch {}: {}", batch_id, error_body),
            403 => format!("Forbidden for batch {}: {}", batch_id, error_body),
            404 => format!("Gateway endpoint not found for batch {}: {}", batch_id, error_body),
            413 => format!("Batch {} too large: {}", batch_id, error_body),
            429 => format!("Rate limited for batch {}: {}", batch_id, error_body),
            500..=599 => format!("Gateway server error for batch {}: {}", batch_id, error_body),
            _ => format!("Unexpected response {} for batch {}: {}", status, batch_id, error_body),
        };

        Err(CollectorError::Transport(error_message))
    }

    /// Health check the gateway endpoint
    pub async fn health_check(&self) -> Result<GatewayHealth> {
        let url = format!("{}/health", self.gateway_url);

        debug!("Performing health check against {}", url);

        let response = timeout(
            self.timeout,
            self.client.get(&url).send()
        ).await
        .map_err(|_| CollectorError::Transport("Health check timeout".to_string()))?
        .map_err(CollectorError::Http)?;

        if !response.status().is_success() {
            return Err(CollectorError::Transport(format!(
                "Health check failed with status: {}",
                response.status()
            )));
        }

        let health_data: Value = response.json().await.map_err(CollectorError::Http)?;

        Ok(GatewayHealth {
            status: health_data["status"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            service: health_data["service"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            version: health_data["version"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        })
    }

    /// Test connectivity to the gateway
    pub async fn test_connectivity(&self) -> bool {
        match self.health_check().await {
            Ok(health) => {
                info!(
                    "Gateway connectivity test successful: {} v{} - {}",
                    health.service, health.version, health.status
                );
                true
            }
            Err(e) => {
                warn!("Gateway connectivity test failed: {}", e);
                false
            }
        }
    }

    /// Get transport statistics
    pub fn stats(&self) -> TransportStats {
        TransportStats {
            gateway_url: self.gateway_url.clone(),
            timeout_ms: self.timeout.as_millis() as u64,
            max_retries: self.max_retries,
            retry_backoff_ms: self.retry_backoff_ms,
        }
    }
}

/// Gateway health information
#[derive(Debug, Clone)]
pub struct GatewayHealth {
    pub status: String,
    pub service: String,
    pub version: String,
}

/// Transport statistics
#[derive(Debug, Clone)]
pub struct TransportStats {
    pub gateway_url: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
}

/// Batch transport with enhanced error handling and metrics
#[derive(Debug)]
pub struct EnhancedTransport {
    transport: HttpTransport,
    metrics: TransportMetrics,
}

impl EnhancedTransport {
    pub fn new(transport: HttpTransport) -> Self {
        Self {
            transport,
            metrics: TransportMetrics::new(),
        }
    }

    /// Send a batch with metrics tracking
    pub async fn send_batch(&self, batch: TelemetryBatch) -> Result<()> {
        let start_time = std::time::Instant::now();
        self.metrics.increment_attempts().await;

        match self.transport.send_batch(batch).await {
            Ok(()) => {
                let duration = start_time.elapsed();
                self.metrics.record_success(duration).await;
                Ok(())
            }
            Err(e) => {
                let duration = start_time.elapsed();
                self.metrics.record_failure(duration).await;
                Err(e)
            }
        }
    }

    /// Get transport metrics
    pub async fn metrics(&self) -> TransportMetricsSnapshot {
        self.metrics.snapshot().await
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        self.metrics.reset().await;
    }
}

/// Transport metrics tracking
#[derive(Debug)]
struct TransportMetrics {
    attempts: tokio::sync::RwLock<u64>,
    successes: tokio::sync::RwLock<u64>,
    failures: tokio::sync::RwLock<u64>,
    total_duration: tokio::sync::RwLock<Duration>,
    min_duration: tokio::sync::RwLock<Option<Duration>>,
    max_duration: tokio::sync::RwLock<Option<Duration>>,
}

impl TransportMetrics {
    fn new() -> Self {
        Self {
            attempts: tokio::sync::RwLock::new(0),
            successes: tokio::sync::RwLock::new(0),
            failures: tokio::sync::RwLock::new(0),
            total_duration: tokio::sync::RwLock::new(Duration::ZERO),
            min_duration: tokio::sync::RwLock::new(None),
            max_duration: tokio::sync::RwLock::new(None),
        }
    }

    async fn increment_attempts(&self) {
        let mut attempts = self.attempts.write().await;
        *attempts += 1;
    }

    async fn record_success(&self, duration: Duration) {
        let mut successes = self.successes.write().await;
        *successes += 1;
        drop(successes);

        self.update_duration_stats(duration).await;
    }

    async fn record_failure(&self, duration: Duration) {
        let mut failures = self.failures.write().await;
        *failures += 1;
        drop(failures);

        self.update_duration_stats(duration).await;
    }

    async fn update_duration_stats(&self, duration: Duration) {
        let mut total = self.total_duration.write().await;
        *total += duration;
        drop(total);

        let mut min = self.min_duration.write().await;
        *min = Some(min.map_or(duration, |m| m.min(duration)));
        drop(min);

        let mut max = self.max_duration.write().await;
        *max = Some(max.map_or(duration, |m| m.max(duration)));
    }

    async fn snapshot(&self) -> TransportMetricsSnapshot {
        let attempts = *self.attempts.read().await;
        let successes = *self.successes.read().await;
        let failures = *self.failures.read().await;
        let total_duration = *self.total_duration.read().await;
        let min_duration = *self.min_duration.read().await;
        let max_duration = *self.max_duration.read().await;

        let success_rate = if attempts > 0 {
            (successes as f64 / attempts as f64) * 100.0
        } else {
            0.0
        };

        let avg_duration = if attempts > 0 {
            total_duration / attempts as u32
        } else {
            Duration::ZERO
        };

        TransportMetricsSnapshot {
            attempts,
            successes,
            failures,
            success_rate,
            avg_duration_ms: avg_duration.as_millis() as u64,
            min_duration_ms: min_duration.map(|d| d.as_millis() as u64),
            max_duration_ms: max_duration.map(|d| d.as_millis() as u64),
        }
    }

    async fn reset(&self) {
        *self.attempts.write().await = 0;
        *self.successes.write().await = 0;
        *self.failures.write().await = 0;
        *self.total_duration.write().await = Duration::ZERO;
        *self.min_duration.write().await = None;
        *self.max_duration.write().await = None;
    }
}

/// Snapshot of transport metrics
#[derive(Debug, Clone)]
pub struct TransportMetricsSnapshot {
    pub attempts: u64,
    pub successes: u64,
    pub failures: u64,
    pub success_rate: f64,
    pub avg_duration_ms: u64,
    pub min_duration_ms: Option<u64>,
    pub max_duration_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{LogEntry, LogLevel, BatchMetadata};

    #[test]
    fn test_transport_creation() {
        let transport = HttpTransport::new(
            "http://localhost:8080".to_string(),
            Duration::from_secs(10),
            3,
            1000,
        );

        assert!(transport.is_ok());
        let transport = transport.unwrap();
        assert_eq!(transport.gateway_url, "http://localhost:8080");
        assert_eq!(transport.max_retries, 3);
    }

    #[tokio::test]
    async fn test_transport_metrics() {
        let transport = HttpTransport::new(
            "http://localhost:8080".to_string(),
            Duration::from_secs(1),
            0, // No retries for test
            1000,
        ).unwrap();

        let enhanced = EnhancedTransport::new(transport);

        // Test metrics initialization
        let metrics = enhanced.metrics().await;
        assert_eq!(metrics.attempts, 0);
        assert_eq!(metrics.successes, 0);
        assert_eq!(metrics.failures, 0);
        assert_eq!(metrics.success_rate, 0.0);
    }

    #[test]
    fn test_gateway_health_parsing() {
        // This would be a more comprehensive test with a mock HTTP server
        // For now, just test the structure
        let health = GatewayHealth {
            status: "healthy".to_string(),
            service: "telemetry-gateway".to_string(),
            version: "1.0.0".to_string(),
        };

        assert_eq!(health.status, "healthy");
        assert_eq!(health.service, "telemetry-gateway");
        assert_eq!(health.version, "1.0.0");
    }
}
