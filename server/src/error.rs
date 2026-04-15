use thiserror::Error;
use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication failed")]
    Unauthorized,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Resource not found")]
    NotFound,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limited")]
    RateLimited,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status, error_type) = match self {
            AppError::Unauthorized => (actix_web::http::StatusCode::UNAUTHORIZED, "unauthorized"),
            AppError::InvalidCredentials => (actix_web::http::StatusCode::UNAUTHORIZED, "invalid_credentials"),
            AppError::NotFound => (actix_web::http::StatusCode::NOT_FOUND, "not_found"),
            AppError::Validation(_) => (actix_web::http::StatusCode::BAD_REQUEST, "validation"),
            AppError::Database(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "database"),
            AppError::Internal(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "internal"),
            AppError::Conflict(_) => (actix_web::http::StatusCode::CONFLICT, "conflict"),
            AppError::RateLimited => (actix_web::http::StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
        };

        HttpResponse::build(status).json(ErrorResponse {
            error: error_type.to_string(),
            message: self.to_string(),
        })
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
