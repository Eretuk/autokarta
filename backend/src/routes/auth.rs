use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{self, CurrentUser, SESSION_COOKIE},
    error::{AppError, AppResult},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me).patch(update_me))
        .route("/password", post(change_password))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(sqlx::FromRow)]
struct LoginRow {
    id: Uuid,
    password_hash: String,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<CurrentUser>)> {
    let key = req.username.trim().to_lowercase();
    state.limiter.check(&key)?;

    let row = sqlx::query_as::<_, LoginRow>(
        "SELECT id, password_hash FROM users WHERE lower(username) = $1 AND is_active",
    )
    .bind(&key)
    .fetch_optional(&state.db)
    .await?;

    let ok =
        auth::verify_password(req.password, row.as_ref().map(|r| r.password_hash.clone())).await;
    let Some(row) = row.filter(|_| ok) else {
        state.limiter.record_failure(&key);
        return Err(AppError::InvalidCredentials);
    };
    state.limiter.reset(&key);

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let token = auth::create_session(&state, row.id, user_agent).await?;
    let user = load_user(&state, row.id).await?;

    Ok((
        jar.add(auth::session_cookie(&state.settings, token)),
        Json(user),
    ))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<(CookieJar, StatusCode)> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        auth::delete_session(&state, cookie.value()).await?;
    }
    Ok((jar.remove(auth::removal_cookie()), StatusCode::NO_CONTENT))
}

async fn me(user: CurrentUser) -> Json<CurrentUser> {
    Json(user)
}

#[derive(Deserialize)]
struct UpdateMe {
    display_name: Option<String>,
    locale: Option<String>,
}

async fn update_me(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<UpdateMe>,
) -> AppResult<Json<CurrentUser>> {
    let display_name = req
        .display_name
        .as_deref()
        .map(validate::display_name)
        .transpose()
        .map_err(AppError::Validation)?;
    let locale = req
        .locale
        .as_deref()
        .map(validate::locale)
        .transpose()
        .map_err(AppError::Validation)?;

    sqlx::query(
        "UPDATE users SET display_name = COALESCE($2, display_name),
                          locale = COALESCE($3, locale),
                          updated_at = now()
          WHERE id = $1",
    )
    .bind(user.id)
    .bind(display_name)
    .bind(locale)
    .execute(&state.db)
    .await?;

    Ok(Json(load_user(&state, user.id).await?))
}

#[derive(Deserialize)]
struct ChangePassword {
    current_password: String,
    new_password: String,
}

/// Changing your own password signs out every other device.
async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    user: CurrentUser,
    Json(req): Json<ChangePassword>,
) -> AppResult<StatusCode> {
    validate::password(&req.new_password).map_err(AppError::Validation)?;

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    if !auth::verify_password(req.current_password, Some(hash)).await {
        return Err(AppError::Validation("wrong_current_password"));
    }

    let new_hash = auth::hash_password(req.new_password).await?;
    let current_token = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .unwrap_or_default();

    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
        .bind(user.id)
        .bind(new_hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM sessions WHERE user_id = $1 AND token_hash <> sha256($2::text::bytea)",
    )
    .bind(user.id)
    .bind(current_token)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn load_user(state: &AppState, id: Uuid) -> AppResult<CurrentUser> {
    sqlx::query_as::<_, CurrentUser>(
        "SELECT id, username, display_name, role, locale FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}
