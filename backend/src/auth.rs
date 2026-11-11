//! Passwords, sessions and the login rate limiter.
//!
//! Sessions are random 256-bit tokens kept in an HttpOnly cookie.
//! The database stores only their SHA-256 hash.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::Context;
use argon2::{
    password_hash::{rand_core::OsRng, rand_core::RngCore, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::{AppState, Settings},
};

pub const SESSION_COOKIE: &str = "autokarta_session";

// ---------- passwords ----------

/// Argon2 is deliberately slow, so it runs off the async executor.
pub async fn hash_password(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))
    })
    .await
    .context("hashing task panicked")?
}

/// Verifies a password. With `hash = None` (unknown user) it checks against a
/// dummy hash anyway, so response time does not reveal which usernames exist.
pub async fn verify_password(password: String, hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || {
        let hash = hash.unwrap_or_else(|| dummy_hash().to_string());
        let Ok(parsed) = PasswordHash::new(&hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
    .await
    .unwrap_or(false)
}

fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(b"not-a-real-password", &salt)
            .expect("dummy hash")
            .to_string()
    })
}

// ---------- sessions ----------

fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub async fn create_session(
    state: &AppState,
    user_id: Uuid,
    user_agent: Option<String>,
) -> AppResult<String> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);

    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at, user_agent)
         VALUES ($1, $2, now() + make_interval(secs => $3), $4)",
    )
    .bind(hash_token(&token))
    .bind(user_id)
    .bind(state.settings.session_ttl.whole_seconds() as f64)
    .bind(user_agent.map(|ua| ua.chars().take(300).collect::<String>()))
    .execute(&state.db)
    .await?;

    Ok(token)
}

/// Returns the username the session belonged to, if it existed.
pub async fn delete_session(state: &AppState, token: &str) -> AppResult<Option<String>> {
    let username = sqlx::query_scalar(
        "DELETE FROM sessions s USING users u
          WHERE s.token_hash = $1 AND u.id = s.user_id
         RETURNING u.username",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.db)
    .await?;
    Ok(username)
}

pub fn session_cookie(settings: &Settings, token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .secure(settings.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(settings.session_ttl)
        .build()
}

pub fn removal_cookie() -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, "")).path("/").build()
}

/// Removes expired sessions once an hour.
pub async fn cleanup_expired_sessions(db: sqlx::PgPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(3600));
    loop {
        tick.tick().await;
        match sqlx::query("DELETE FROM sessions WHERE expires_at < now()")
            .execute(&db)
            .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                tracing::info!(removed = r.rows_affected(), "expired sessions removed")
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = ?e, "session cleanup failed"),
        }
    }
}

// ---------- first start ----------

pub const FIRST_OWNER: &str = "autokarta_admin";

/// On the very first start (no accounts at all) creates the owner
/// [`FIRST_OWNER`] with a random password and returns that password, so it can
/// be printed once. The account must replace it on first sign-in. Returns
/// `None` when any account exists, so it never runs again.
pub async fn create_first_owner(db: &sqlx::PgPool) -> anyhow::Result<Option<String>> {
    let mut tx = db.begin().await?;
    // Two instances starting together must not both create an owner.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('autokarta_first_owner'))")
        .execute(&mut *tx)
        .await?;
    let any: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users)")
        .fetch_one(&mut *tx)
        .await?;
    if any {
        return Ok(None);
    }

    let password = generate_password();
    let hash = hash_password(password.clone()).await?;
    sqlx::query(
        "INSERT INTO users (username, display_name, role, password_hash, must_change_password)
         VALUES ($1, 'Admin', 'owner', $2, true)",
    )
    .bind(FIRST_OWNER)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(password))
}

/// 25 random characters in groups of five (125 bits), from an alphabet without
/// look-alikes (no i, l, o, u), easy to type from a log on a phone.
fn generate_password() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let mut bytes = [0u8; 25];
    OsRng.fill_bytes(&mut bytes);
    let chars: Vec<char> = bytes
        .iter()
        .map(|b| ALPHABET[(b & 31) as usize] as char)
        .collect();
    chars
        .chunks(5)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

// ---------- current user extractors ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CurrentUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub locale: String,
    pub must_change_password: bool,
}

impl CurrentUser {
    pub fn is_owner(&self) -> bool {
        self.role == "owner"
    }
}

/// Any signed-in user, including one who still has to replace a password
/// someone else knows. Only for signing out, reading yourself and changing
/// the password; everything else takes [`CurrentUser`].
pub struct SignedIn(pub CurrentUser);

impl FromRequestParts<AppState> for SignedIn {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .map(|c| c.value().to_owned())
            .ok_or(AppError::Unauthorized)?;
        let token_hash = hash_token(&token);

        let user = sqlx::query_as::<_, CurrentUser>(
            "SELECT u.id, u.username, u.display_name, u.role, u.locale, u.must_change_password
               FROM sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = $1 AND s.expires_at > now() AND u.is_active",
        )
        .bind(&token_hash)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
        crate::request_log::record_user(&user.username);

        // Cheap activity tracking: at most one write per session every 10 minutes.
        sqlx::query(
            "UPDATE sessions SET last_seen_at = now()
              WHERE token_hash = $1 AND last_seen_at < now() - interval '10 minutes'",
        )
        .bind(&token_hash)
        .execute(&state.db)
        .await?;

        Ok(SignedIn(user))
    }
}

/// A signed-in user with a password of their own.
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let SignedIn(user) = SignedIn::from_request_parts(parts, state).await?;
        if user.must_change_password {
            return Err(AppError::PasswordChangeRequired);
        }
        Ok(user)
    }
}

/// Like [`CurrentUser`], but rejects anyone who is not an owner.
pub struct Owner(pub CurrentUser);

impl FromRequestParts<AppState> for Owner {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_owner() {
            return Err(AppError::Forbidden);
        }
        Ok(Owner(user))
    }
}

// ---------- login rate limiter ----------

/// Failures from one address for one username before that address is
/// locked out of it.
const MAX_FAILURES_PER_CLIENT: u32 = 5;
/// Failures for one username from all addresses together: stops a
/// distributed guess without letting a single stranger lock the owner out
/// with five wrong passwords.
const MAX_FAILURES_PER_USER: u32 = 50;
const WINDOW: Duration = Duration::from_secs(15 * 60);

/// In-memory limiter over two counters per username: one per client
/// address and one for all of them. Resets on restart, which is fine for
/// one instance.
#[derive(Default)]
pub struct LoginLimiter {
    entries: Mutex<HashMap<String, (u32, Instant)>>,
}

/// Counter keys; usernames cannot contain a newline, so they never clash.
fn client_key(username: &str, ip: Option<IpAddr>) -> String {
    match ip {
        Some(ip) => format!("{username}\n{ip}"),
        None => format!("{username}\n-"),
    }
}

impl LoginLimiter {
    pub fn check(&self, username: &str, ip: Option<IpAddr>) -> AppResult<()> {
        let entries = self.entries.lock().expect("limiter lock");
        let limits = [
            (client_key(username, ip), MAX_FAILURES_PER_CLIENT),
            (username.to_owned(), MAX_FAILURES_PER_USER),
        ];
        for (key, max) in limits {
            if let Some((failures, since)) = entries.get(&key) {
                let elapsed = since.elapsed();
                if *failures >= max && elapsed < WINDOW {
                    tracing::warn!(username = %username, "sign-in refused: locked out");
                    return Err(AppError::TooManyAttempts {
                        retry_after_secs: (WINDOW - elapsed).as_secs().max(1),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn record_failure(&self, username: &str, ip: Option<IpAddr>) {
        let mut entries = self.entries.lock().expect("limiter lock");
        if entries.len() > 10_000 {
            entries.retain(|_, (_, since)| since.elapsed() < WINDOW);
        }
        let limits = [
            (client_key(username, ip), MAX_FAILURES_PER_CLIENT),
            (username.to_owned(), MAX_FAILURES_PER_USER),
        ];
        for (key, max) in limits {
            let entry = entries.entry(key).or_insert((0, Instant::now()));
            if entry.1.elapsed() >= WINDOW {
                *entry = (0, Instant::now());
            }
            entry.0 += 1;
            if entry.0 == max {
                tracing::warn!(
                    username = %username,
                    minutes = WINDOW.as_secs() / 60,
                    all_addresses = max == MAX_FAILURES_PER_USER,
                    "too many failed sign-ins, username locked"
                );
            }
        }
    }

    /// After a successful sign-in from this address.
    pub fn reset_client(&self, username: &str, ip: Option<IpAddr>) {
        self.entries
            .lock()
            .expect("limiter lock")
            .remove(&client_key(username, ip));
    }

    /// Lifts every lockout of a username (an owner reset its password).
    pub fn reset_user(&self, username: &str) {
        let prefix = format!("{username}\n");
        self.entries
            .lock()
            .expect("limiter lock")
            .retain(|k, _| k != username && !k.starts_with(&prefix));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_passwords() {
        let a = super::generate_password();
        assert_eq!(a.len(), 29);
        assert_eq!(a.split('-').count(), 5);
        assert!(a
            .chars()
            .all(|c| c == '-' || "0123456789abcdefghjkmnpqrstvwxyz".contains(c)));
        assert_ne!(a, super::generate_password());
        assert!(crate::validate::password(&a).is_ok());
    }

    use super::*;

    #[test]
    fn limiter_blocks_per_address_then_per_user() {
        let l = LoginLimiter::default();
        let a: Option<IpAddr> = Some("198.51.100.1".parse().unwrap());
        let b: Option<IpAddr> = Some("198.51.100.2".parse().unwrap());
        for _ in 0..MAX_FAILURES_PER_CLIENT {
            assert!(l.check("ivan", a).is_ok());
            l.record_failure("ivan", a);
        }
        // The guesser is locked out; Ivan at his own address is not.
        assert!(l.check("ivan", a).is_err());
        assert!(l.check("ivan", b).is_ok());
        assert!(l.check("anna", a).is_ok());

        // Many addresses together lock the username everywhere.
        for i in 0..MAX_FAILURES_PER_USER {
            l.record_failure(
                "anna",
                Some(IpAddr::from([10, 0, (i / 256) as u8, i as u8])),
            );
        }
        assert!(l.check("anna", b).is_err());

        l.reset_user("ivan");
        l.reset_user("anna");
        assert!(l.check("ivan", a).is_ok());
        assert!(l.check("anna", b).is_ok());
    }

    #[tokio::test]
    async fn password_roundtrip() {
        let hash = hash_password("correct horse".into()).await.unwrap();
        assert!(verify_password("correct horse".into(), Some(hash.clone())).await);
        assert!(!verify_password("wrong".into(), Some(hash)).await);
        assert!(!verify_password("anything".into(), None).await);
    }
}
