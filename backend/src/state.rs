use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::LoginLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub settings: Arc<Settings>,
    pub limiter: Arc<LoginLimiter>,
}

pub struct Settings {
    pub cookie_secure: bool,
    pub session_ttl: time::Duration,
}
