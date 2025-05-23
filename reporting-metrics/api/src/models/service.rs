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
   pub github_metrics: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
   pub name: String,
   pub url: String,
   pub github_repo: Option<String>,
}

impl Default for ServiceMetrics {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            status: "unknown".to_string(),
            response_time_ms: 0,
            uptime_percentage: 0.0,
            availability_history: Vec::new(),
            last_checked: Utc::now(),
            github_metrics: None,
        }
    }
}
