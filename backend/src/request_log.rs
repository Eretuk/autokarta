//! One log line per request, inside a span that carries the request id, the
//! client address and (once the session is checked) the username, so every
//! other event logged while handling the request names who caused it.
//!
//! Query strings are left out on purpose: searches carry names, phones and
//! plates, which do not belong in logs.

use std::{
    net::{IpAddr, SocketAddr},
    time::Instant,
};

use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use tracing::{field::Empty, Instrument, Level};

use crate::state::AppState;

/// The error code of a failed request, put into the response by `AppError`
/// so the request line can show why it failed.
#[derive(Clone)]
pub struct ErrorCode(pub String);

/// The client's address as resolved here (see `client_ip`), for handlers
/// that need it, such as the login rate limiter. `None` without a socket
/// (tests).
#[derive(Clone, Copy)]
pub struct ClientIp(pub Option<IpAddr>);

pub async fn log_requests(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let id = format!("{:08x}", OsRng.next_u32());
    let span = tracing::info_span!("req", %id, ip = Empty, user = Empty);
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let ip = state
        .settings
        .trusted_proxies
        .client_ip(peer, req.headers());
    if let Some(ip) = ip {
        span.record("ip", tracing::field::display(ip));
    }
    req.extensions_mut().insert(ClientIp(ip));

    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let start = Instant::now();
    let mut res = next.run(req).instrument(span.clone()).await;
    let ms = start.elapsed().as_millis() as u64;

    let status = res.status();
    let error = res.extensions().get::<ErrorCode>().map(|e| e.0.clone());
    let level = if status.is_server_error() {
        Level::ERROR
    } else if !path.starts_with("/api/") || path == "/api/health" {
        // Static files and health probes would drown everything else.
        Level::DEBUG
    } else {
        Level::INFO
    };

    let _enter = span.enter();
    let status = status.as_u16();
    macro_rules! line {
        ($level:expr) => {
            match &error {
                Some(error) => {
                    tracing::event!($level, %error, ms, "{method} {path} {status}")
                }
                None => tracing::event!($level, ms, "{method} {path} {status}"),
            }
        };
    }
    match level {
        Level::ERROR => line!(Level::ERROR),
        Level::INFO => line!(Level::INFO),
        _ => line!(Level::DEBUG),
    }

    // Lets a user who saw an error quote something we can find in the log.
    if let Ok(v) = HeaderValue::from_str(&id) {
        res.headers_mut().insert("x-request-id", v);
    }
    res
}

/// Names the signed-in user in the current request's log span.
pub fn record_user(username: &str) {
    tracing::Span::current().record("user", tracing::field::display(username));
}
