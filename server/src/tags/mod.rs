use actix_web::{web, HttpResponse};
use serde::Serialize;
use uuid::Uuid;
use crate::error::{AppError, Result};
use crate::auth::Claims;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct TagResponse {
    pub id: String,
    pub name: String,
}

pub async fn list(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let tags: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM tags WHERE user_id = $1 ORDER BY name"
    )
    .bind(&user_id)
    .fetch_all(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tags": tags.iter().map(|(id, name)| TagResponse {
            id: id.to_string(),
            name: name.clone()
        }).collect::<Vec<_>>()
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateTag {
    pub name: String,
}

pub async fn create(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateTag>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let tag_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO tags (user_id, name) VALUES ($1, $2)
           ON CONFLICT DO NOTHING RETURNING id"#
    )
    .bind(&user_id)
    .bind(&body.name)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": tag_id.0.to_string(),
        "name": body.name
    })))
}

pub async fn delete(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let tag_id: Uuid = path.into_inner().parse().map_err(|_| AppError::InvalidCredentials)?;

    sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(&tag_id)
        .bind(&user_id)
        .execute(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "deleted"})))
}
