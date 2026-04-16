// Simple auth helper — validates JWT and returns Claims
// Usage: let claims = auth::validate_request(&req, &state.jwt_secret)?;

use actix_web::{HttpRequest, HttpResponse, http::header::AUTHORIZATION};
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::auth::Claims;

pub fn extract_bearer(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

pub fn validate_request(req: &HttpRequest, jwt_secret: &str) -> Result<Claims, HttpResponse> {
    let token = extract_bearer(req).ok_or_else(|| {
        HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "unauthorized", "message": "Missing Authorization header"}))
    })?;

    let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());
    let validation = Validation::default();

    decode::<Claims>(&token, &decoding_key, &validation)
        .map(|t| t.claims)
        .map_err(|_| {
            HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "unauthorized", "message": "Invalid or expired token"}))
        })
}
