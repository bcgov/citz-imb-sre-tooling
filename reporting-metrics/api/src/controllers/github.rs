use crate::state::AppState;
use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

pub async fn get_github_metrics(
    app_state: web::Data<AppState>,
) -> impl Responder {
    let metrics_cache = app_state.metrics_cache.lock().unwrap();

    // Extract GitHub metrics from all services
    let mut github_metrics = json!({});

    for (service_name, metrics) in metrics_cache.iter() {
        if let Some(ref github_metrics_str) = metrics.github_metrics {
            // Parse JSON from string
            if let Ok(parsed_metrics) = serde_json::from_str(github_metrics_str) {
                github_metrics[service_name] = parsed_metrics;
            }
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

    if let Some(metrics) = metrics_cache.get(&service_name) {
        if let Some(ref github_metrics_str) = metrics.github_metrics {
            // Parse JSON from string
            if let Ok(parsed_metrics) = serde_json::from_str::<serde_json::Value>(github_metrics_str) {
                return HttpResponse::Ok().json(parsed_metrics);
            }
        }
    }

    HttpResponse::NotFound().json(json!({
        "error": format!("GitHub metrics not found for service: {}", service_name)
    }))
}
