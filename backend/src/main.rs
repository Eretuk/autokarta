//! Autokarta: workshop, parts and fleet management for small auto services.

mod auth;
mod client_ip;
mod error;
mod fifo;
mod request_log;
mod routes;
mod state;
mod updates;
mod validate;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

use crate::state::{AppState, Settings};

#[derive(Parser)]
#[command(name = "autokarta", version, about)]
struct Cli {
    /// PostgreSQL connection string (not needed by `healthcheck`).
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    #[command(flatten)]
    serve: ServeArgs,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Args)]
struct ServeArgs {
    /// Address the HTTP server listens on.
    #[arg(long, env = "BIND_ADDR", default_value = "0.0.0.0:8080")]
    bind: SocketAddr,

    /// Directory with the built frontend. Skipped if it does not exist.
    #[arg(long, env = "STATIC_DIR", default_value = "./static")]
    static_dir: PathBuf,

    /// Mark the session cookie as Secure (HTTPS only).
    /// Set to false only for local development over plain HTTP.
    #[arg(long, env = "COOKIE_SECURE", default_value_t = true, action = clap::ArgAction::Set)]
    cookie_secure: bool,

    /// How long a login stays valid, in days.
    #[arg(long, env = "SESSION_TTL_DAYS", default_value_t = 30)]
    session_ttl_days: i64,

    /// Reverse proxies whose X-Forwarded-For is believed: comma-separated
    /// addresses or CIDR networks; `private` = loopback and private
    /// networks, `none` = never. Everyone else's header is ignored, so a
    /// client reaching the app directly cannot fake its address.
    #[arg(long, env = "TRUSTED_PROXIES", default_value = "private")]
    trusted_proxies: client_ip::TrustedProxies,

    /// Ask GitHub twice a day whether a newer release exists, to tell the
    /// owner in the app. Sends nothing but the app version.
    #[arg(long, env = "UPDATE_CHECK", default_value_t = true, action = clap::ArgAction::Set)]
    update_check: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run the web server (default).
    Serve,
    /// Exit 0 if the server on BIND_ADDR answers /api/health (for Docker).
    Healthcheck,
    /// Create an owner account (e.g. to regain access). Prompts for a password.
    CreateAdmin {
        #[arg(long)]
        username: String,
        #[arg(long)]
        display_name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        // Colours only in a terminal; `docker compose logs` and log
        // collectors get plain text.
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        // Messages say what happened; module paths only add noise.
        .with_target(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("autokarta=info")),
        )
        .init();

    let cli = Cli::parse();
    if let Some(Command::Healthcheck) = cli.command {
        return healthcheck(cli.serve.bind).await;
    }
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "autokarta starting");

    let database_url = cli.database_url.context("DATABASE_URL is not set")?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url)
        .await
        .context("cannot connect to the database")?;

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("database migration failed")?;
    tracing::info!("database ready");

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(db, cli.serve).await,
        Command::Healthcheck => unreachable!("handled before connecting"),
        Command::CreateAdmin {
            username,
            display_name,
        } => create_admin(db, username, display_name).await,
    }
}

async fn serve(db: sqlx::PgPool, args: ServeArgs) -> anyhow::Result<()> {
    let state = AppState {
        db,
        settings: Arc::new(Settings {
            cookie_secure: args.cookie_secure,
            session_ttl: time::Duration::days(args.session_ttl_days),
            trusted_proxies: args.trusted_proxies,
        }),
        limiter: Arc::new(auth::LoginLimiter::default()),
        latest_release: Default::default(),
    };

    if let Some(password) = auth::create_first_owner(&state.db).await? {
        print_first_owner(&password);
    }
    tokio::spawn(auth::cleanup_expired_sessions(state.db.clone()));
    if args.update_check {
        // ring, not rustls' default aws-lc: no cmake/C toolchain needed to
        // cross-build for arm64.
        let _ = rustls::crypto::ring::default_provider().install_default();
        tokio::spawn(updates::watch(state.latest_release.clone()));
    }

    let static_dir = args
        .static_dir
        .is_dir()
        .then_some(args.static_dir.as_path());
    if static_dir.is_none() {
        tracing::warn!(dir = %args.static_dir.display(), "frontend directory not found, serving API only");
    }
    if !args.cookie_secure {
        tracing::warn!("COOKIE_SECURE=false: use this only for local development");
    }

    let app = routes::router(state, static_dir);
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    tracing::info!("listening on http://{}", args.bind);
    // The peer address is the client's only without a proxy; behind one the
    // request log prefers X-Forwarded-For.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Printed once, on the start that created the account: the password is not
/// stored anywhere else, and signing in with it asks for a new one at once.
fn print_first_owner(password: &str) {
    let lines = [
        "First start: created the owner account.".to_string(),
        format!("  username: {}", auth::FIRST_OWNER),
        format!("  password: {password}"),
        "Sign in and choose your own password; this one works only for that.".to_string(),
        "It is shown only now. Lost it? Create another owner:".to_string(),
        "  docker compose exec app autokarta create-admin --username <name>".to_string(),
    ];
    tracing::warn!("{}", "=".repeat(72));
    for line in lines {
        tracing::warn!("{line}");
    }
    tracing::warn!("{}", "=".repeat(72));
}

async fn create_admin(
    db: sqlx::PgPool,
    username: String,
    display_name: Option<String>,
) -> anyhow::Result<()> {
    let username = validate::username(&username).map_err(|e| anyhow::anyhow!("{e}"))?;
    let display_name = display_name.unwrap_or_else(|| username.clone());

    let password = rpassword::prompt_password("Password: ")?;
    if rpassword::prompt_password("Repeat password: ")? != password {
        bail!("passwords do not match");
    }
    validate::password(&password).map_err(|e| anyhow::anyhow!("{e}"))?;

    let hash = auth::hash_password(password).await?;
    let result = sqlx::query(
        "INSERT INTO users (username, display_name, role, password_hash) VALUES ($1, $2, 'owner', $3)",
    )
    .bind(&username)
    .bind(&display_name)
    .bind(&hash)
    .execute(&db)
    .await;

    match result {
        Ok(_) => {
            println!("Owner account '{username}' created.");
            Ok(())
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            bail!("username '{username}' is already taken")
        }
        Err(e) => Err(e.into()),
    }
}

/// A plain HTTP/1.1 request over TCP: the runtime image has no curl, and an
/// HTTP client crate for one request is not worth it.
async fn healthcheck(bind: SocketAddr) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let ip = match bind.ip() {
        ip if !ip.is_unspecified() => ip,
        std::net::IpAddr::V4(_) => std::net::Ipv4Addr::LOCALHOST.into(),
        std::net::IpAddr::V6(_) => std::net::Ipv6Addr::LOCALHOST.into(),
    };
    let check = async {
        let mut stream = tokio::net::TcpStream::connect((ip, bind.port())).await?;
        stream
            .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut head = [0u8; 12];
        stream.read_exact(&mut head).await?;
        anyhow::Ok(head)
    };
    let head = tokio::time::timeout(Duration::from_secs(5), check)
        .await
        .context("health check timed out")??;
    if !head.starts_with(b"HTTP/1.1 200") {
        bail!("unhealthy: {}", String::from_utf8_lossy(&head));
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
