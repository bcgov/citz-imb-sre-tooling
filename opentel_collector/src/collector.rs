//! Main sidecar collector implementation

use crate::config::Config;
use crate::telemetry::{LogEntry, TraceSpan};
use crate::log_parser::{LogParser, LogParserFactory};
use crate::buffer::{TelemetryBuffer, is_high_priority_log, is_high_priority_span};
use crate::transport::{HttpTransport, EnhancedTransport};
use crate::errors::{CollectorError, Result};

use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::time::{interval, Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug, instrument};
use uuid::Uuid;

/// Main sidecar collector orchestrating log collection and transmission
pub struct SidecarCollector {
    config: Config,
    parser: Box<dyn LogParser>,
    buffer: Arc<TelemetryBuffer>,
    transport: Arc<EnhancedTransport>,
    collector_id: String,
    file_states: Arc<RwLock<Vec<FileState>>>,
}

/// File tracking state for log tailing
#[derive(Debug, Clone)]
struct FileState {
    path: String,
    last_position: u64,
    last_modified: Option<std::time::SystemTime>,
    inode: Option<u64>,
}

impl SidecarCollector {
    /// Create a new sidecar collector
    pub fn new(config: Config) -> Result<Self> {
        config.validate().map_err(CollectorError::Config)?;

        // Create log parser
        let parser = LogParserFactory::create_parser(
            "composite",
            config.enable_trace_correlation,
        );

        // Create buffer
        let buffer = Arc::new(TelemetryBuffer::new(
            config.max_buffer_size,
            config.batch_size,
        ));

        // Create transport
        let http_transport = HttpTransport::new(
            config.gateway_url.clone(),
            config.http_timeout,
            config.max_retries,
            config.retry_backoff_ms,
        )?;
        let transport = Arc::new(EnhancedTransport::new(http_transport));

        // Initialize file states
        let file_states = Arc::new(RwLock::new(
            config.log_paths.iter()
                .map(|path| FileState {
                    path: path.clone(),
                    last_position: 0,
                    last_modified: None,
                    inode: None,
                })
                .collect()
        ));

        Ok(Self {
            config,
            parser,
            buffer,
            transport,
            collector_id: Uuid::new_v4().to_string(),
            file_states,
        })
    }

    /// Start the collector
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting sidecar collector {} for service: {}",
            self.collector_id, self.config.service_name
        );

        if !self.transport.test_connectivity().await {
            warn!("Gateway connectivity test failed, but continuing anyway");
        }

        for (index, _) in self.config.log_paths.iter().enumerate() {
            let collector = self.clone_for_task();
            tokio::spawn(async move {
                if let Err(e) = collector.monitor_file(index).await {
                    error!("File monitoring task {} failed: {}", index, e);
                }
            });
        }

        let flush_collector = self.clone_for_task();
        tokio::spawn(async move {
            flush_collector.periodic_flush().await;
        });

        let metrics_collector = self.clone_for_task();
        tokio::spawn(async move {
            metrics_collector.report_metrics().await;
        });

        tokio::signal::ctrl_c().await.map_err(|e| {
            CollectorError::Other(format!("Failed to wait for shutdown signal: {}", e))
        })?;

        info!("Shutting down sidecar collector");
        self.shutdown().await?;
        Ok(())
    }

    /// Monitor a specific log file
    #[instrument(skip(self))]
    async fn monitor_file(&self, file_index: usize) -> Result<()> {
        let path = &self.config.log_paths[file_index];
        info!("Starting file monitor for: {}", path);

        let mut check_interval = interval(Duration::from_millis(500));
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        loop {
            check_interval.tick().await;

            match self.check_and_read_file(file_index).await {
                Ok(lines_read) => {
                    consecutive_errors = 0;
                    if lines_read > 0 {
                        debug!("Read {} lines from {}", lines_read, path);
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    if consecutive_errors <= MAX_CONSECUTIVE_ERRORS {
                        warn!("Error reading file {} (attempt {}): {}", path, consecutive_errors, e);
                    }

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        error!(
                            "Too many consecutive errors reading file {}, pausing for 30 seconds",
                            path
                        );
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        consecutive_errors = 0;
                    }
                }
            }
        }
    }

    /// Check file for changes and read new content
    async fn check_and_read_file(&self, file_index: usize) -> Result<usize> {
        let path = {
            let file_states = self.file_states.read().await;
            file_states[file_index].path.clone()
        };

        if !Path::new(&path).exists() {
            return Ok(0);
        }

        let metadata = tokio::fs::metadata(&path).await?;
        let current_size = metadata.len();
        let current_modified = metadata.modified().ok();

        let (should_read, start_position) = {
            let mut file_states = self.file_states.write().await;
            let state = &mut file_states[file_index];

            // Check if file was truncated or rotated
            if current_size < state.last_position {
                debug!("File {} appears to have been truncated or rotated", path);
                state.last_position = 0;
                state.last_modified = current_modified;
                (true, 0)
            }
            // Check if file was modified
            else if state.last_modified != current_modified || current_size > state.last_position {
                (true, state.last_position)
            } else {
                (false, state.last_position)
            }
        };

        if !should_read {
            return Ok(0);
        }

        self.read_file_from_position(&path, file_index, start_position).await
    }

    /// Read file content from a specific position
    async fn read_file_from_position(
        &self,
        path: &str,
        file_index: usize,
        start_position: u64,
    ) -> Result<usize> {
        let mut file = File::open(path).await?;
        file.seek(SeekFrom::Start(start_position)).await?;

        let mut reader = BufReader::new(file);
        let mut lines_read = 0;
        let mut current_position = start_position;

        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                break;
            }

            current_position += bytes_read as u64;
            lines_read += 1;

            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            if line.trim().is_empty() {
                continue;
            }

            self.process_log_line(&line).await?;
        }

        {
            let mut file_states = self.file_states.write().await;
            let state = &mut file_states[file_index];
            state.last_position = current_position;
            state.last_modified = tokio::fs::metadata(path).await?.modified().ok();
        }

        Ok(lines_read)
    }

    /// Process a single log line
    async fn process_log_line(&self, line: &str) -> Result<()> {
        if let Some(log_entry) = self.parser.parse_log(
            line,
            &self.config.service_name,
            &self.config.pod_name,
            &self.config.namespace,
        )? {
            self.buffer.add_log(log_entry).await?;
        }

        if let Some(span) = self.parser.parse_span(line, &self.config.service_name)? {
            self.buffer.add_span(span).await?;
        }

        Ok(())
    }

    /// Periodic flush of buffered data
    async fn periodic_flush(&self) {
        let mut flush_interval = interval(self.config.flush_interval);

        loop {
            flush_interval.tick().await;

            if let Err(e) = self.flush_buffers().await {
                error!("Failed to flush buffers: {}", e);
            }
        }
    }

    /// Flush buffered telemetry data
    async fn flush_buffers(&self) -> Result<()> {
        if !self.buffer.has_data().await {
            return Ok(());
        }

        let batches = self.buffer.flush_all(
            self.collector_id.clone(),
            self.config.pod_name.clone(),
            self.config.namespace.clone(),
        ).await?;

        debug!("Flushing {} batches", batches.len());

        for batch in batches {
            if let Err(e) = self.transport.send_batch(batch).await {
                error!("Failed to send batch: {}", e);
                // TODO: Persistent retry logic
            }
        }

        Ok(())
    }

    /// Report metrics periodically
    async fn report_metrics(&self) {
        let mut metrics_interval = interval(Duration::from_secs(60));

        loop {
            metrics_interval.tick().await;

            let (log_count, span_count) = self.buffer.sizes().await;
            let utilization = self.buffer.utilization().await;
            let transport_metrics = self.transport.metrics().await;

            info!(
                "Collector metrics - Buffered: {} logs, {} spans ({:.1}% utilization), Transport: {:.1}% success rate, {} attempts",
                log_count,
                span_count,
                utilization,
                transport_metrics.success_rate,
                transport_metrics.attempts
            );
        }
    }

    /// Graceful shutdown
    async fn shutdown(&self) -> Result<()> {
        info!("Performing graceful shutdown");

        self.flush_buffers().await?;

        // Report final metrics
        let transport_metrics = self.transport.metrics().await;
        info!(
            "Final transport metrics - Success rate: {:.1}%, Total attempts: {}, Avg duration: {}ms",
            transport_metrics.success_rate,
            transport_metrics.attempts,
            transport_metrics.avg_duration_ms
        );

        info!("Sidecar collector shutdown complete");
        Ok(())
    }

    /// Create a clone suitable for async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            parser: LogParserFactory::create_parser(
                "composite",
                self.config.enable_trace_correlation,
            ),
            buffer: Arc::clone(&self.buffer),
            transport: Arc::clone(&self.transport),
            collector_id: self.collector_id.clone(),
            file_states: Arc::clone(&self.file_states),
        }
    }

    /// Get collector statistics
    pub async fn stats(&self) -> CollectorStats {
        let (buffered_logs, buffered_spans) = self.buffer.sizes().await;
        let buffer_utilization = self.buffer.utilization().await;
        let transport_metrics = self.transport.metrics().await;

        CollectorStats {
            collector_id: self.collector_id.clone(),
            service_name: self.config.service_name.clone(),
            pod_name: self.config.pod_name.clone(),
            namespace: self.config.namespace.clone(),
            buffered_logs,
            buffered_spans,
            buffer_utilization,
            transport_success_rate: transport_metrics.success_rate,
            transport_attempts: transport_metrics.attempts,
            avg_transport_duration_ms: transport_metrics.avg_duration_ms,
        }
    }
}

/// Collector statistics
#[derive(Debug, Clone)]
pub struct CollectorStats {
    pub collector_id: String,
    pub service_name: String,
    pub pod_name: String,
    pub namespace: String,
    pub buffered_logs: usize,
    pub buffered_spans: usize,
    pub buffer_utilization: f64,
    pub transport_success_rate: f64,
    pub transport_attempts: u64,
    pub avg_transport_duration_ms: u64,
}
