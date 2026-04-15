use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{AppError, Result};
use crate::auth::Claims;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AiSearchQuery {
    pub q: String,
    pub tags: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct IngestNote {
    pub title: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub auto_tag: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AiSearchResult {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub score: f32,
}

/// AI-optimized search endpoint
/// Returns structured JSON optimized for AI consumption
pub async fn search(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    query: web::Query<AiSearchQuery>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let limit = query.limit.unwrap_or(5).min(20);

    let results: Vec<(Uuid, String, String)> = sqlx::query_as(
        r#"
        SELECT id, title, left(content, 300)
        FROM notes
        WHERE user_id = $1 
          AND (visibility = 'shared' OR key_id = $2)
          AND to_tsvector('english', title || ' ' || content) @@ plainto_tsquery('english', $3)
        ORDER BY ts_rank(to_tsvector('english', title || ' ' || content), plainto_tsquery('english', $3)) DESC
        LIMIT $4
        "#
    )
    .bind(&user_id)
    .bind(&claims.key_id)
    .bind(&query.q)
    .bind(limit)
    .fetch_all(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let response: Vec<AiSearchResult> = results.iter().map(|(id, title, snippet)| {
        AiSearchResult {
            id: id.to_string(),
            title: title.clone(),
            snippet: snippet.clone(),
            tags: vec![], // TODO: load tags
            score: 0.9,   // TODO: use actual rank
        }
    }).collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "results": response,
        "query": query.q,
        "count": response.len()
    })))
}

/// AI bulk ingest endpoint
/// Allows AI to write notes from conversations
pub async fn ingest(
    state: web::Data<AppState>,
    claims: web::ReqData<Claims>,
    body: web::Json<IngestNote>,
) -> Result<HttpResponse> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let key_id = claims.key_id.as_ref().map(|k| Uuid::parse_str(k)).transpose()
        .map_err(|_| AppError::Internal("Invalid key_id in token".to_string()))?;

    if body.title.is_empty() || body.content.is_empty() {
        return Err(AppError::Validation("title and content required".to_string()));
    }

    let note_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO notes (user_id, key_id, title, content, visibility)
           VALUES ($1, $2, $3, $4, 'private') RETURNING id"#
    )
    .bind(&user_id)
    .bind(&key_id)
    .bind(&body.title)
    .bind(&body.content)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": note_id.0.to_string(),
        "status": "ingested"
    })))
}
