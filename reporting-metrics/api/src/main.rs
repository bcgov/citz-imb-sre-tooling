use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{DateTime, Utc};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use anyhow::Result;
use reqwest::Client as HttpClient;

// Data models
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ServiceMetrics {
    name: String,
    url: String,
    status: String,
    response_time_ms: u64,
    uptime_percentage: f64,
    availability_history: Vec<bool>,
    last_checked: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceConfig {
    name: String,
    url: String,
}

// App state
struct AppState {
    metrics_cache: Mutex<HashMap<String, ServiceMetrics>>,
    services: Mutex<Vec<ServiceConfig>>,
    http_client: HttpClient,
}

// Register a new service to monitor
async fn register_service(
    data: web::Data<AppState>,
    service: web::Json<ServiceConfig>,
) -> impl Responder {
    let mut services = data.services.lock().unwrap();

    // Check if service already exists
    for existing in services.iter() {
        if existing.name == service.name {
            return HttpResponse::BadRequest().json("Service with this name already exists");
        }
    }

    services.push(service.into_inner());

    HttpResponse::Ok().json("Service registered successfully")
}

// Get metrics for a specific service
async fn get_service_metrics(
    data: web::Data<AppState>,
    service_name: web::Path<String>,
) -> impl Responder {
    let cache = data.metrics_cache.lock().unwrap();
    let name = service_name.into_inner();

    if let Some(metrics) = cache.get(&name) {
        HttpResponse::Ok().json(metrics.clone())
    } else {
        HttpResponse::NotFound().json("Service not found or metrics not yet collected")
    }
}

// Get all services with their latest metrics
async fn get_all_metrics(data: web::Data<AppState>) -> impl Responder {
    let cache = data.metrics_cache.lock().unwrap();
    let metrics: Vec<ServiceMetrics> = cache.values().cloned().collect();

    HttpResponse::Ok().json(metrics)
}

// List all registered services
async fn list_services(data: web::Data<AppState>) -> impl Responder {
    let services = data.services.lock().unwrap();

    HttpResponse::Ok().json(services.clone())
}

// Remove a service from monitoring
async fn remove_service(
    data: web::Data<AppState>,
    service_name: web::Path<String>,
) -> impl Responder {
    let name = service_name.into_inner();

    // Remove from services list
    {
        let mut services = data.services.lock().unwrap();
        let original_len = services.len();
        services.retain(|s| s.name != name);

        if services.len() == original_len {
            return HttpResponse::NotFound().json("Service not found");
        }
    }

    // Remove from metrics cache
    {
        let mut cache = data.metrics_cache.lock().unwrap();
        cache.remove(&name);
    }

    HttpResponse::Ok().json("Service removed successfully")
}

// Health check endpoint
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json("OK")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!("Starting Service Metrics API server at http://{}", bind_addr);

    // Create HTTP client
    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");

    // Create application state
    let app_state = web::Data::new(AppState {
        metrics_cache: Mutex::new(HashMap::new()),
        services: Mutex::new(Vec::new()),
        http_client,
    });

    // Start background metrics collector
    let collector_state = app_state.clone();
    tokio::spawn(async move {
        metrics_collector(collector_state).await;
    });

    HttpServer::new(move || {
    App::new()
        .app_data(app_state.clone())
        .route("/health", web::get().to(health_check))
        .route("/services", web::get().to(list_services))
        .route("/services", web::post().to(register_service))
        .route("/services/{name}", web::delete().to(remove_service))
        .route("/metrics", web::get().to(get_all_metrics))
        .route("/metrics/{name}", web::get().to(get_service_metrics))
    })
    .bind(bind_addr)?
    .run()
    .await
}
