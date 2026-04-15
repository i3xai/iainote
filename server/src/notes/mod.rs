use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use crate::error::{AppError, Result};
use crate::auth::{verify_token, Claims};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub tag: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNote {
    pub title: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNote {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub visibility: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct NoteResponse {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub visibility: String,
    pub version: i32,
    pub key_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = (page - 1) * limit;

    let notes = sqlx::query_as::<_, (Uuid, String, String, String, i32, Option<Uuid>, chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT n.id, n.title, n.content, n.visibility, n.version, n.key_id, n.created_at, n.updated_at
        FROM notes n
        WHERE n.user_id = $1
        ORDER BY n.updated_at DESC
        LIMIT $2 OFFSET $3
        "#
    )
    .bind(&user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let response: Vec<NoteResponse> = Vec::new();
    // TODO: Implement tag loading and response mapping

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "notes": response,
        "page": page,
        "limit": limit
    })))
}

pub async fn create(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreateNote>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let key_id = claims.key_id.as_ref().map(|k| Uuid::parse_str(k)).transpose()
        .map_err(|_| AppError::Internal("Invalid key_id in token".to_string()))?;

    if body.title.is_empty() || body.content.is_empty() {
        return Err(AppError::Validation("title and content required".to_string()));
    }

    let visibility = body.visibility.as_deref().unwrap_or("private");
    if !["private", "shared"].contains(&visibility) {
        return Err(AppError::Validation("visibility must be 'private' or 'shared'".to_string()));
    }

    let note_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO notes (user_id, key_id, title, content, visibility)
           VALUES ($1, $2, $3, $4, $5) RETURNING id"#
    )
    .bind(&user_id)
    .bind(&key_id)
    .bind(&body.title)
    .bind(&body.content)
    .bind(visibility)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": note_id.0.to_string(),
        "title": body.title,
        "version": 1,
        "created_at": chrono::Utc::now().to_rfc3339()
    })))
}

pub async fn get(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let note_id: Uuid = path.into_inner().parse().map_err(|_| AppError::NotFound)?;

    let note: Option<(Uuid, String, String, String, i32, Option<Uuid>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, title, content, visibility, version, key_id, created_at, updated_at FROM notes WHERE id = $1 AND user_id = $2"
    )
    .bind(&note_id)
    .bind(&user_id)
    .fetch_optional(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match note {
        Some((id, title, content, visibility, version, key_id, created_at, updated_at)) => {
            Ok(HttpResponse::Ok().json(NoteResponse {
                id: id.to_string(),
                title,
                content,
                tags: vec![], // TODO: load tags
                visibility,
                version,
                key_id: key_id.map(|k| k.to_string()),
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
            }))
        }
        None => Err(AppError::NotFound)
    }
}

pub async fn update(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
    body: web::Json<UpdateNote>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let note_id: Uuid = path.into_inner().parse().map_err(|_| AppError::NotFound)?;

    // Optimistic locking
    if let Some(expected_version) = body.version {
        let current: Option<(i32,)> = sqlx::query_as(
            "SELECT version FROM notes WHERE id = $1 AND user_id = $2"
        )
        .bind(&note_id)
        .bind(&user_id)
        .fetch_optional(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if let Some((current_version,)) = current {
            if current_version != expected_version {
                return Err(AppError::Conflict("Version mismatch, note was modified".to_string()));
            }
        }
    }

    let title = body.title.as_ref().map(|s| s.as_str()).unwrap_or("");
    let content = body.content.as_ref().map(|s| s.as_str()).unwrap_or("");

    let result = sqlx::query(
        r#"UPDATE notes SET 
           title = CASE WHEN $3::text <> '' THEN $3 ELSE title END,
           content = CASE WHEN $4::text <> '' THEN $4 ELSE content END,
           visibility = COALESCE($5, visibility),
           version = version + 1,
           updated_at = NOW()
           WHERE id = $1 AND user_id = $2"#
    )
    .bind(&note_id)
    .bind(&user_id)
    .bind(title)
    .bind(content)
    .bind(&body.visibility)
    .execute(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "updated",
        "id": note_id.to_string()
    })))
}

pub async fn delete(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let note_id: Uuid = path.into_inner().parse().map_err(|_| AppError::NotFound)?;

    sqlx::query("DELETE FROM notes WHERE id = $1 AND user_id = $2")
        .bind(&note_id)
        .bind(&user_id)
        .execute(state.db.as_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "deleted"})))
}

pub async fn search(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<SearchQuery>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let limit = query.limit.unwrap_or(20).min(50);

    let results: Vec<(Uuid, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, title, left(content, 200), updated_at
        FROM notes
        WHERE user_id = $1 
          AND to_tsvector('english', title || ' ' || content) @@ plainto_tsquery('english', $2)
        ORDER BY ts_rank(to_tsvector('english', title || ' ' || content), plainto_tsquery('english', $2)) DESC
        LIMIT $3
        "#
    )
    .bind(&user_id)
    .bind(&query.q)
    .bind(limit)
    .fetch_all(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "results": results.iter().map(|(id, title, snippet, updated)| {
            serde_json::json!({
                "id": id.to_string(),
                "title": title,
                "snippet": snippet,
                "updated_at": updated.to_rfc3339()
            })
        }).collect::<Vec<_>>(),
        "total": results.len(),
        "query": query.q
    })))
}
