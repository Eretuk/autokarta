//! Business-wide settings: currency, distance unit and the default parts
//! markup. These are business decisions, independent of the UI language.

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{CurrentUser, Owner},
    error::{AppError, AppResult},
    routes::deserialize_some,
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
    /// Default markup on the net purchase price for suggested sale prices;
    /// `null`: no suggestions. Owners only (absent for mechanics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup_percent: Option<Option<i16>>,
}

pub async fn load(db: &sqlx::PgPool) -> AppResult<Settings> {
    let (currency, distance_unit, markup): (String, String, Option<i16>) =
        sqlx::query_as("SELECT currency, distance_unit, markup_percent FROM business_settings")
            .fetch_one(db)
            .await?;
    Ok(Settings {
        currency,
        distance_unit,
        markup_percent: Some(markup),
    })
}

async fn for_user(db: &sqlx::PgPool, user: &CurrentUser) -> AppResult<Settings> {
    let mut s = load(db).await?;
    if !user.is_owner() {
        s.markup_percent = None;
    }
    Ok(s)
}

async fn get_settings(
    State(state): State<AppState>,
    user: CurrentUser,
) -> AppResult<Json<Settings>> {
    Ok(Json(for_user(&state.db, &user).await?))
}

#[derive(Deserialize)]
struct UpdateSettings {
    currency: Option<String>,
    distance_unit: Option<String>,
    /// Absent: unchanged. null: no suggested prices.
    #[serde(default, deserialize_with = "deserialize_some")]
    markup_percent: Option<Option<i64>>,
}

async fn update(
    State(state): State<AppState>,
    Owner(owner): Owner,
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
    let markup = req
        .markup_percent
        .map(|m| m.map(validate::markup_percent).transpose())
        .transpose()
        .map_err(AppError::Validation)?;

    sqlx::query(
        "UPDATE business_settings SET currency = COALESCE($1, currency),
                                      distance_unit = COALESCE($2, distance_unit),
                                      markup_percent = CASE WHEN $3 THEN $4 ELSE markup_percent END,
                                      updated_at = now()",
    )
    .bind(currency)
    .bind(unit)
    .bind(markup.is_some())
    .bind(markup.flatten())
    .execute(&state.db)
    .await?;

    Ok(Json(for_user(&state.db, &owner).await?))
}
