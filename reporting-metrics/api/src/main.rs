use actix_web::{get, post, web, App, HttpServer, Responder, HttpResponse};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    Client,
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

// Data models
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProjectMetrics {
    name: String,
    uptime_seconds: u64,
    pod_count: usize,
    pod_status: HashMap<String, usize>, // Running, Pending, Failed, etc.
    last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectRequest {
    name: String,
}

// App state
struct AppState {
    metrics_cache: Mutex<HashMap<String, ProjectMetrics>>,
    kube_client: Client,
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
