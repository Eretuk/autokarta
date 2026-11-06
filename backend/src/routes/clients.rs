//! Customers. Personal data: staff only, never on public pages.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::CurrentUser,
    error::{AppError, AppResult},
    routes::like_pattern,
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(search).post(create))
        .route("/{id}", put(update))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Client {
    pub id: Uuid,
    pub name: String,
    pub phone: Option<String>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

async fn search(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<Client>>> {
    let q = query.q.trim();
    // Phones are stored as typed: compare digits only.
    let digits: String = q.chars().filter(char::is_ascii_digit).collect();
    let rows = sqlx::query_as::<_, Client>(
        "SELECT id, name, phone, note FROM clients
          WHERE $1 = ''
             OR lower(name) LIKE $2
             OR ($3 <> '' AND regexp_replace(coalesce(phone, ''), '\\D', '', 'g') LIKE '%' || $3 || '%')
          ORDER BY lower(name)
          LIMIT 20",
    )
    .bind(q)
    .bind(like_pattern(&q.to_lowercase()))
    .bind(digits)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct ClientInput {
    name: String,
    phone: Option<String>,
    note: Option<String>,
}

fn clean(req: &ClientInput) -> AppResult<(String, Option<String>, Option<String>)> {
    Ok((
        validate::client_name(&req.name).map_err(AppError::Validation)?,
        validate::phone(req.phone.as_deref()).map_err(AppError::Validation)?,
        validate::optional_text(req.note.as_deref(), 2000).map_err(AppError::Validation)?,
    ))
}

async fn create(
    State(state): State<AppState>,
    _user: CurrentUser,
    Json(req): Json<ClientInput>,
) -> AppResult<(StatusCode, Json<Client>)> {
    let (name, phone, note) = clean(&req)?;
    let row = sqlx::query_as::<_, Client>(
        "INSERT INTO clients (name, phone, note) VALUES ($1, $2, $3)
         RETURNING id, name, phone, note",
    )
    .bind(name)
    .bind(phone)
    .bind(note)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn update(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ClientInput>,
) -> AppResult<Json<Client>> {
    let (name, phone, note) = clean(&req)?;
    let row = sqlx::query_as::<_, Client>(
        "UPDATE clients SET name = $2, phone = $3, note = $4, updated_at = now()
          WHERE id = $1
          RETURNING id, name, phone, note",
    )
    .bind(id)
    .bind(name)
    .bind(phone)
    .bind(note)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}
