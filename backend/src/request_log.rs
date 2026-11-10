//! One log line per request, inside a span that carries the request id, the
//! client address and (once the session is checked) the username, so every
//! other event logged while handling the request names who caused it.
//!
//! Query strings are left out on purpose: searches carry names, phones and
//! plates, which do not belong in logs.

use std::{net::SocketAddr, time::Instant};

use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::{field::Empty, Instrument, Level};

/// The error code of a failed request, put into the response by `AppError`
/// so the request line can show why it failed.
#[derive(Clone)]
pub struct ErrorCode(pub String);

pub async fn log_requests(req: Request, next: Next) -> Response {
    let id = format!("{:08x}", OsRng.next_u32());
    let span = tracing::info_span!("req", %id, ip = Empty, user = Empty);
    if let Some(ip) = client_ip(&req) {
        span.record("ip", tracing::field::display(ip));
    }

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

/// The app runs behind a reverse proxy, which appends the address it saw to
/// `X-Forwarded-For`: the last entry is the one added by our proxy (earlier
/// ones come from the client and can be anything). Without the header, the
/// direct peer.
fn client_ip(req: &Request) -> Option<String> {
    forwarded_for(req.headers()).or_else(|| {
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string())
    })
}

fn forwarded_for(headers: &HeaderMap) -> Option<String> {
    let last = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .rfind(|v| !v.is_empty())?;
    Some(last.chars().take(64).collect())
}

/// Names the signed-in user in the current request's log span.
pub fn record_user(username: &str) {
    tracing::Span::current().record("user", tracing::field::display(username));
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn forwarded_for_takes_the_last_hop() {
        let mut h = HeaderMap::new();
        assert_eq!(super::forwarded_for(&h), None);
        h.append(
            "x-forwarded-for",
            HeaderValue::from_static("1.1.1.1, 10.0.0.9"),
        );
        assert_eq!(super::forwarded_for(&h).as_deref(), Some("10.0.0.9"));
        h.append("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
        assert_eq!(super::forwarded_for(&h).as_deref(), Some("203.0.113.7"));
    }
}
