use crate::state::AppState;
use actix_web::{web, HttpResponse, Responder};
use serde_json::json;
use std::collections::HashMap;

pub async fn get_github_metrics(
    app_state: web::Data<AppState>,
) -> impl Responder {
    let metrics_cache = app_state.metrics_cache.lock().unwrap();

    // Extract GitHub metrics from all services
    let mut github_metrics = json!({});
    for (service_name, metrics) in metrics_cache.iter() {
        if let Some(github_data) = metrics.get("github_metrics") {
            github_metrics.insert(service_name, github_data);
        }
    }

    HttpResponse::Ok().json(github_metrics)
}

pub async fn get_service_github_metrics(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let service_name = path.into_inner();
    let metrics_cache = app_state.metrics_cache.lock().unwrap();

    if let Some(service_metrics) = metrics_cache.get(&service_name) {
        if let Some(github_metrics) = service_metrics.get("github_metrics") {
            return HttpResponse::Ok().json(github_metrics);
        }
    }

    HttpResponse::NotFound().json(json!({
        "error": format!("GitHub metrics not found for service: {}", service_name)
    }))
}
