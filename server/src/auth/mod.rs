use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use sha2::{Sha256, Digest};
use hex;
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use chrono::{Utc, Duration};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // user_id
    pub key_id: Option<String>,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user_id: String,
    pub email: String,
    pub keys: Vec<KeyInfo>,
}

#[derive(Debug, Serialize)]
pub struct KeyInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

fn hash_password(password: &str) -> Result<String> {
    let salt = rand::random::<[u8; 16]>();
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password, &salt)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

fn create_jwt(user_id: &Uuid, key_id: Option<&Uuid>, secret: &str) -> Result<String> {
    let now = Utc::now();
    let expiry = now + Duration::hours(24 * 7);

    let claims = Claims {
        sub: user_id.to_string(),
        key_id: key_id.map(|k| k.to_string()),
        exp: expiry.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|e| AppError::Internal(e.to_string()))
}

pub fn extract_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

pub async fn verify_token(req: &HttpRequest, state: &AppState) -> Result<Claims> {
    let token = extract_token(req).ok_or(AppError::Unauthorized)?;

    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    ).map_err(|_| AppError::Unauthorized)?;

    Ok(token_data.claims)
}

pub async fn register(
    state: web::Data<AppState>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse> {
    let email = body.email.to_lowercase().trim().to_string();
    let password_hash = hash_password(&body.password)?;

    let user_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO users (email, password_hash) VALUES ($1, $2) 
           ON CONFLICT (email) DO UPDATE SET updated_at = NOW()
           RETURNING id"#
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "user_id": user_id.0.to_string(),
        "email": email
    })))
}

pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse> {
    let email = body.email.to_lowercase().trim().to_string();

    let user: (Uuid, String) = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE email = $1"
    )
    .bind(&email)
    .fetch_optional(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or(AppError::InvalidCredentials)?;

    if !verify_password(&body.password, &user.1)? {
        return Err(AppError::InvalidCredentials);
    }

    let token = create_jwt(&user.0, None, &state.config.jwt_secret)?;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: 24 * 7 * 3600,
    }))
}

pub async fn create_key(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateKeyRequest>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    // Generate API key
    let raw_key = format!("ia_sk_{}", hex::encode(rand::random::<[u8; 16]>()));
    let key_hash = hash_api_key(&raw_key);

    let key_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO api_keys (user_id, name, key_hash) VALUES ($1, $2, $3)
           RETURNING id"#
    )
    .bind(&user_id)
    .bind(&body.name)
    .bind(&key_hash)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": key_id.0.to_string(),
        "name": body.name,
        "key": raw_key,  // Only returned once!
        "created_at": chrono::Utc::now().to_rfc3339()
    })))
}

pub async fn list_keys(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let keys: Vec<(Uuid, String, chrono::DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT id, name, created_at, revoked FROM api_keys WHERE user_id = $1"
    )
    .bind(&user_id)
    .fetch_all(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "keys": keys.iter().map(|(id, name, created, revoked)| {
            serde_json::json!({
                "id": id.to_string(),
                "name": name,
                "created_at": created.to_rfc3339(),
                "revoked": revoked
            })
        }).collect::<Vec<_>>()
    })))
}

pub async fn delete_key(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let key_id: Uuid = path.into_inner().parse().map_err(|_| AppError::InvalidCredentials)?;

    sqlx::query("UPDATE api_keys SET revoked = true WHERE id = $1 AND user_id = $2")
        .bind(&key_id)
        .bind(&user_id)
        .execute(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "ok"})))
}

pub async fn merge_keys(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let source_key_id: Uuid = path.into_inner().parse().map_err(|_| AppError::InvalidCredentials)?;
    let target_key_id: Uuid = body["target_key_id"].as_str()
        .ok_or_else(|| AppError::Validation("target_key_id required".to_string()))?
        .parse().map_err(|_| AppError::Validation("Invalid target_key_id".to_string()))?;

    // Verify both keys belong to user
    sqlx::query("UPDATE notes SET key_id = $1 WHERE key_id = $2 AND user_id = $3")
        .bind(&target_key_id)
        .bind(&source_key_id)
        .bind(&user_id)
        .execute(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "merged"})))
}

pub async fn transfer_key(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let source_key_id: Uuid = path.into_inner().parse().map_err(|_| AppError::InvalidCredentials)?;
    let target_key_id: Uuid = body["target_key_id"].as_str()
        .ok_or_else(|| AppError::Validation("target_key_id required".to_string()))?
        .parse().map_err(|_| AppError::Validation("Invalid target_key_id".to_string()))?;

    sqlx::query("UPDATE notes SET key_id = $1 WHERE key_id = $2 AND user_id = $3")
        .bind(&target_key_id)
        .bind(&source_key_id)
        .bind(&user_id)
        .execute(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "transferred"})))
}
