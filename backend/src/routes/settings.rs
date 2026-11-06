//! Business-wide settings: currency and distance unit. These are business
//! decisions, independent of the UI language.

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{CurrentUser, Owner},
    error::{AppError, AppResult},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(get_settings).patch(update))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Settings {
    pub currency: String,
    pub distance_unit: String,
}

pub async fn load(db: &sqlx::PgPool) -> AppResult<Settings> {
    Ok(
        sqlx::query_as::<_, Settings>("SELECT currency, distance_unit FROM business_settings")
            .fetch_one(db)
            .await?,
    )
}

async fn get_settings(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> AppResult<Json<Settings>> {
    Ok(Json(load(&state.db).await?))
}

#[derive(Deserialize)]
struct UpdateSettings {
    currency: Option<String>,
    distance_unit: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    _owner: Owner,
    Json(req): Json<UpdateSettings>,
) -> AppResult<Json<Settings>> {
    let currency = req
        .currency
        .as_deref()
        .map(validate::currency)
        .transpose()
        .map_err(AppError::Validation)?;
    let unit = req
        .distance_unit
        .as_deref()
        .map(|u| validate::one_of(u, validate::DISTANCE_UNITS, "invalid_distance_unit"))
        .transpose()
        .map_err(AppError::Validation)?;

    sqlx::query(
        "UPDATE business_settings SET currency = COALESCE($1, currency),
                                      distance_unit = COALESCE($2, distance_unit),
                                      updated_at = now()",
    )
    .bind(currency)
    .bind(unit)
    .execute(&state.db)
    .await?;

    Ok(Json(load(&state.db).await?))
}
