use std::collections::HashMap;
use std::sync::Mutex;
use reqwest::Client as HttpClient;

use crate::models::{ServiceConfig, ServiceMetrics};

// App state
pub struct AppState {
   pub metrics_cache: Mutex<HashMap<String, ServiceMetrics>>,
   pub services: Mutex<Vec<ServiceConfig>>,
   pub http_client: HttpClient,
}

