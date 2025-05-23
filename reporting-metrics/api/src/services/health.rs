use actix_web::{HttpResponse, Responder};

// Health check endpoint
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json("OK")
}
