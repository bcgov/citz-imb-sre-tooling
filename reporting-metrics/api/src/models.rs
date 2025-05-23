use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Data models
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceMetrics {
   pub name: String,
   pub url: String,
   pub status: String,
   pub response_time_ms: u64,
   pub uptime_percentage: f64,
   pub availability_history: Vec<bool>,
   pub last_checked: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
   pub name: String,
   pub url: String,
}

