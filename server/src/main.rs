use actix_web::{web, App, HttpServer, HttpResponse, middleware};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod auth;
mod auth_middleware;
mod notes;
mod tags;
mod ai;
mod db;
mod error;
mod config;

pub use error::{AppError, Result};
pub use config::Config as AppConfig;

#[derive(Debug, Clone)]
pub struct AppState {
    pub jwt_secret: String,
    pub db: Arc<db::DbPool>,
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "iainote-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "error": "not_found",
        "message": "The requested resource was not found"
    }))
}

fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health check
        .route("/health", web::get().to(health))
        // Auth routes
        .service(
            web::scope("/api/v1/auth")
                .route("/register", web::post().to(auth::register))
                .route("/login", web::post().to(auth::login))
                .route("/keys", web::post().to(auth::create_key))
                .route("/keys", web::get().to(auth::list_keys))
                .route("/keys/{id}", web::delete().to(auth::delete_key))
        )
        // Notes routes (protected)
        .service(
            web::scope("/api/v1/notes")
                
                .route("", web::get().to(notes::list))
                .route("", web::post().to(notes::create))
                .route("/{id}", web::get().to(notes::get))
                .route("/{id}", web::put().to(notes::update))
                .route("/{id}", web::delete().to(notes::delete))
                .route("/search", web::get().to(notes::search))
        )
        // Tags routes (protected)
        .service(
            web::scope("/api/v1/tags")
                
                .route("", web::get().to(tags::list))
                .route("", web::post().to(tags::create))
                .route("/{id}", web::delete().to(tags::delete))
        )
        // AI routes (protected)
        .service(
            web::scope("/api/v1/ai")
                
                .route("/search", web::get().to(ai::search))
                .route("/ingest", web::post().to(ai::ingest))
        )
        // Key management (protected)
        .service(
            web::scope("/api/v1/keys")
                
                .route("/{id}/merge", web::post().to(auth::merge_keys))
                .route("/{id}/transfer", web::post().to(auth::transfer_key))
        );
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Load config
    let config = AppConfig::from_env().expect("Failed to load configuration");

    // Initialize database
    let db_pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    tracing::info!("Starting iainote API server on {}", config.server_addr);

    HttpServer::new(move || {
        let cors = actix_cors::Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .app_data(web::Data::new(AppState {
                jwt_secret: config.jwt_secret.clone(),
                db: Arc::new(db_pool.clone()),
            }))
            .app_data(web::JsonConfig::default())
            .app_data(web::QueryConfig::default())
            .app_data(web::PathConfig::default())
            .configure(configure_routes)
            .default_service(web::route().to(not_found))
    })
    .bind(&config.server_addr)?
    .run()
    .await
}
