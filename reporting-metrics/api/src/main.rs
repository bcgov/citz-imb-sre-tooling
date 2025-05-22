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
