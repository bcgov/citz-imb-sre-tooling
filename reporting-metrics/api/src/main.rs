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

#[derive(Serialize)]
struct HealthResponse {
    status: String
}

#[get("/health")]
async fn health_check() -> impl Responder {
    web::Json(HealthResponse {
        status: "OK".into(),
    })
}

#[derive(Deserialize)]
struct GreetRequest {
    name: String,
}

#[post("/greet")]
async fn greet_user(info: web::Json<GreetRequest>) -> impl Responder {
    HttpResponse::Ok().json(format!("Hello, {}!", info.name))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Server is live at http://0.0.0.0:8080");
    HttpServer::new(|| {
        App::new()
            .service(health_check)
            .service(greet_user)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
