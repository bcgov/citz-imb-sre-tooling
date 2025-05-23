use log::{error, info};
use reqwest::Client as HttpClient;
use std::time::{Duration};
use tokio::time::sleep;
use chrono::Utc;
use actix_web::web;

use crate::models::service::{ServiceConfig, ServiceMetrics};
use crate::state::AppState;

// Check a single service and return its metrics
pub async fn check_service(client: &HttpClient, service: &ServiceConfig) -> ServiceMetrics {
    let start_time = std::time::Instant::now();
    let mut status = "down".to_string();
    let mut response_time_ms = 0;

    match client.get(&service.url).send().await {
        Ok(response) if response.status().is_success() => {
            status = "up".to_string();
            response_time_ms = start_time.elapsed().as_millis() as u64;
        }
        Ok(_) => {
            error!("Service {} returned non-success status", service.name);
        }
        Err(e) => {
            error!("Failed to connect to service {}: {}", service.name, e);
        }
    }

    // Get existing metrics or create new ones
    let metrics = ServiceMetrics {
        name: service.name.clone(),
        url: service.url.clone(),
        status,
        response_time_ms,
        uptime_percentage: 0.0,
        availability_history: Vec::new(),
        last_checked: Utc::now(),
    };

    // Return the updated metrics
    metrics
}

pub async fn metrics_collector(data: web::Data<AppState>) {
    info!("Starting metrics collector background task");

    loop {
        // Collect metrics for all registered services
        {
            let services = data.services.lock().unwrap().clone();

            for service in services {
                let metrics = check_service(&data.http_client, &service).await;

                // Update metrics cache
                let mut cache = data.metrics_cache.lock().unwrap();
                cache.insert(service.name.clone(), metrics);
            }
        }

        // Wait before next collection cycle (e.g., 30 seconds)
        sleep(Duration::from_secs(30)).await;
    }
}

