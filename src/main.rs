mod config;
mod controllers;
mod handlers;
mod models;
mod routes;
mod services;

use actix_web::{App, HttpServer, Responder, get, web};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            // This connects the config function from route.rs
            .configure(routes::config)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
