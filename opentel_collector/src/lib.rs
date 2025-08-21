//! OpenTelemetry Sidecar Collector Library
//!
//! This library provides components for collecting logs and traces from applications
//! and forwarding them to a telemetry gateway service.

pub mod config;
pub mod collector;
pub mod log_parser;
pub mod telemetry;
pub mod transport;
pub mod buffer;
pub mod errors;

pub use config::Config;
pub use collector::SidecarCollector;
pub use telemetry::{LogEntry, TraceSpan, TelemetryBatch, BatchMetadata};
pub use errors::{CollectorError, Result};
