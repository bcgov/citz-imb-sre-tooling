use actix_web::{get, post, web, App, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};

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