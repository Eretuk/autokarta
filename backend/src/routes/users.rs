//! Staff account management. Owners only: there is no public sign-up.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    auth::{self, Owner},
    error::{AppError, AppResult},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", patch(update))
        .route("/{id}/password", post(reset_password))
}

#[derive(Serialize, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    display_name: String,
    role: String,
    locale: String,
    is_active: bool,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

const SELECT_USER: &str =
    "SELECT id, username, display_name, role, locale, is_active, created_at FROM users";

async fn list(State(state): State<AppState>, _owner: Owner) -> AppResult<Json<Vec<UserRow>>> {
    let rows = sqlx::query_as::<_, UserRow>(&format!(
        "{SELECT_USER} ORDER BY is_active DESC, lower(display_name)"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CreateUser {
    username: String,
    display_name: String,
    role: String,
    password: String,
    locale: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    _owner: Owner,
    Json(req): Json<CreateUser>,
) -> AppResult<(StatusCode, Json<UserRow>)> {
    let username = validate::username(&req.username).map_err(AppError::Validation)?;
    let display_name = validate::display_name(&req.display_name).map_err(AppError::Validation)?;
    let role = validate::role(&req.role).map_err(AppError::Validation)?;
    let locale =
        validate::locale(req.locale.as_deref().unwrap_or("en")).map_err(AppError::Validation)?;
    validate::password(&req.password).map_err(AppError::Validation)?;

    let hash = auth::hash_password(req.password).await?;
    let row = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, display_name, role, locale, password_hash)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, username, display_name, role, locale, is_active, created_at",
    )
    .bind(username)
    .bind(display_name)
    .bind(role)
    .bind(locale)
    .bind(hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => AppError::Conflict("username_taken"),
        other => other.into(),
    })?;

    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Deserialize)]
struct UpdateUser {
    display_name: Option<String>,
    role: Option<String>,
    is_active: Option<bool>,
}

async fn update(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUser>,
) -> AppResult<Json<UserRow>> {
    let display_name = req
        .display_name
        .as_deref()
        .map(validate::display_name)
        .transpose()
        .map_err(AppError::Validation)?;
    let role = req
        .role
        .as_deref()
        .map(validate::role)
        .transpose()
        .map_err(AppError::Validation)?;

    // Guard against locking yourself out.
    if id == owner.id
        && (req.is_active == Some(false) || role.as_deref().is_some_and(|r| r != "owner"))
    {
        return Err(AppError::Validation("cannot_demote_self"));
    }

    let mut tx = state.db.begin().await?;
    let row = sqlx::query_as::<_, UserRow>(
        "UPDATE users SET display_name = COALESCE($2, display_name),
                          role = COALESCE($3, role),
                          is_active = COALESCE($4, is_active),
                          updated_at = now()
          WHERE id = $1
          RETURNING id, username, display_name, role, locale, is_active, created_at",
    )
    .bind(id)
    .bind(display_name)
    .bind(role)
    .bind(req.is_active)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if !row.is_active {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(Json(row))
}

#[derive(Deserialize)]
struct ResetPassword {
    password: String,
}

/// Sets a new password for a staff member and signs them out everywhere.
/// Also lifts a login lockout: a locked-out user is the usual reason to ask.
async fn reset_password(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<ResetPassword>,
) -> AppResult<StatusCode> {
    // Your own password is changed on the account page, which asks for the
    // current one; a borrowed session must not be enough.
    if id == owner.id {
        return Err(AppError::Validation("cannot_reset_own_password"));
    }
    validate::password(&req.password).map_err(AppError::Validation)?;
    let hash = auth::hash_password(req.password).await?;

    let mut tx = state.db.begin().await?;
    let username: String = sqlx::query_scalar(
        "UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1
         RETURNING lower(username)",
    )
    .bind(id)
    .bind(hash)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state.limiter.reset(&username);

    Ok(StatusCode::NO_CONTENT)
}
