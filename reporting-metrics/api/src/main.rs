use actix_web::{web, App, HttpServer};
use log::info;
use reqwest::Client as HttpClient;
use std::collections::HashMap;
use std::env;
use std::sync::Mutex;
use std::time::Duration;

mod models;
mod state;
mod services;
mod controllers;

use state::AppState;
use services::health::health_check;
use services::monitoring::metrics_collector;
use controllers::service::{register_service, list_services, remove_service};
use controllers::metrics::{get_all_metrics, get_service_metrics};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if it exists
    dotenv::dotenv().ok();

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
