use actix_web::{web, HttpResponse, Responder};
use log::info;

use crate::models::service::{ServiceConfig};
use crate::state::AppState;

// Register a new service to monitor
pub async fn register_service(
    data: web::Data<AppState>,
    service: web::Json<ServiceConfig>,
) -> impl Responder {
    info!("Request to register service: {}", service.name);
    let mut services = data.services.lock().unwrap();

    // Check if service already exists
    for existing in services.iter() {
        if existing.name == service.name {
            info!("Request to register service: {}", service.name);
            return HttpResponse::BadRequest().json("Service with this name already exists");
        }
    }

    services.push(service.into_inner());

    HttpResponse::Ok().json("Service registered successfully")
}

// List all registered services
pub async fn list_services(data: web::Data<AppState>) -> impl Responder {
    info!("Request to list all services");
    let services = data.services.lock().unwrap();
    info!("Returning list of {} services", services.len());

    HttpResponse::Ok().json(services.clone())
}

// Remove a service from monitoring
pub async fn remove_service(
    data: web::Data<AppState>,
    service_name: web::Path<String>,
) -> impl Responder {
    let name = service_name.into_inner();
    info!("Request to remove service: {}", name);

    // Remove from services list
    {
        let mut services = data.services.lock().unwrap();
        let original_len = services.len();
        services.retain(|s| s.name != name);

        if services.len() == original_len {
            info!("Service not found for removal: {}", name);
            return HttpResponse::NotFound().json("Service not found");
        }
    }

    // Remove from metrics cache
    {
        let mut cache = data.metrics_cache.lock().unwrap();
        cache.remove(&name);
    }

    info!("Service '{}' removed successfully", name);
    HttpResponse::Ok().json("Service removed successfully")
}
