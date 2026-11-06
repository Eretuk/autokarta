use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{error::AppResult, state::AppState};

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
