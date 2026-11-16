use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::Row;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/notifications", get(list))
        .route("/api/notifications/read-all", post(read_all))
        .route("/api/notifications/{id}/read", post(read_one))
}

pub async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, at, type, crit, read, text_fa, text_en, link FROM notifications
         WHERE (to_user = ?) OR (to_user IS NULL AND to_role = ?)
         ORDER BY at DESC LIMIT 60",
    )
    .bind(&user.id)
    .bind(&user.role)
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "at": r.get::<String, _>("at"),
                "type": r.get::<String, _>("type"),
                "crit": r.get::<i64, _>("crit") == 1,
                "read": r.get::<i64, _>("read") == 1,
                "text": { "fa": r.get::<String, _>("text_fa"), "en": r.get::<String, _>("text_en") },
                "link": r.get::<String, _>("link"),
            })
        })
        .collect();
    let unread = items.iter().filter(|n| n["read"] == json!(false)).count();
    Ok(Json(json!({ "items": items, "unread": unread })))
}

pub async fn read_all(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    sqlx::query(
        "UPDATE notifications SET read = 1
         WHERE (to_user = ?) OR (to_user IS NULL AND to_role = ?)",
    )
    .bind(&user.id)
    .bind(&user.role)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn read_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    // Scoped update: a user can only mark their own notifications.
    sqlx::query(
        "UPDATE notifications SET read = 1
         WHERE id = ? AND ((to_user = ?) OR (to_user IS NULL AND to_role = ?))",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&user.role)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "ok": true })))
}
