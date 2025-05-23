use actix_web::{web, HttpResponse, Responder};
use log::info;

use crate::state::AppState;
use crate::models::service::{ServiceMetrics};

// Get metrics for a specific service
pub async fn get_service_metrics(
    data: web::Data<AppState>,
    service_name: web::Path<String>,
) -> impl Responder {
    let cache = data.metrics_cache.lock().unwrap();
    let name = service_name.into_inner();
    info!("Request for metrics of service: {}", name);

    if let Some(metrics) = cache.get(&name) {
        HttpResponse::Ok().json(metrics.clone())
    } else {
        HttpResponse::NotFound().json("Service not found or metrics not yet collected")
    }
}

// Get all services with their latest metrics
pub async fn get_all_metrics(data: web::Data<AppState>) -> impl Responder {
    info!("Request for all service metrics");
    let cache = data.metrics_cache.lock().unwrap();
    let metrics: Vec<ServiceMetrics> = cache.values().cloned().collect();
    info!("Returning metrics for {} services", metrics.len());

    HttpResponse::Ok().json(metrics)
}
