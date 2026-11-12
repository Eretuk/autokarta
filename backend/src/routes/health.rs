use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    auth::CurrentUser,
    error::AppResult,
    state::AppState,
    updates::{self, Release},
};

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(Json(json!({
        "status": "ok",
        "version": updates::CURRENT,
    })))
}

#[derive(Serialize)]
pub struct VersionInfo {
    version: &'static str,
    /// Owners only: the newest release, once the update check has run.
    #[serde(skip_serializing_if = "Option::is_none")]
    latest: Option<Release>,
    update_available: bool,
}

/// The running version, and for owners whether a newer one is out.
pub async fn version(State(state): State<AppState>, user: CurrentUser) -> Json<VersionInfo> {
    let latest = user
        .is_owner()
        .then(|| updates::latest(&state.latest_release))
        .flatten();
    let update_available = latest
        .as_ref()
        .is_some_and(|r| updates::is_newer(&r.version, updates::CURRENT));
    Json(VersionInfo {
        version: updates::CURRENT,
        latest,
        update_available,
    })
}
