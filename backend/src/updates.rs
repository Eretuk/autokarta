//! Whether a newer Autokarta release exists, so the owner can see it in the
//! app instead of watching GitHub.
//!
//! The server asks the public GitHub API twice a day. The request carries
//! only `autokarta/<version>` as User-Agent: no business data, no user
//! data. `UPDATE_CHECK=false` turns it off (offline servers, or anyone who
//! does not want the server to contact GitHub).

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};

const RELEASES_URL: &str = "https://api.github.com/repos/Eretuk/autokarta/releases/latest";
const CHECK_EVERY: Duration = Duration::from_secs(12 * 3600);

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize)]
pub struct Release {
    pub version: String,
    /// Release page with the notes.
    pub url: String,
    pub published_at: Option<String>,
}

/// The newest release seen on GitHub; `None` until the first successful
/// check (or forever, with the check turned off).
pub type LatestRelease = Arc<RwLock<Option<Release>>>;

pub fn latest(state: &LatestRelease) -> Option<Release> {
    state.read().expect("release lock").clone()
}

/// `true` if `candidate` is a higher version than `current`. Anything that
/// is not plain `MAJOR.MINOR.PATCH` (with an optional leading `v`) is never
/// newer: a pre-release or an odd tag must not nag the owner.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.strip_prefix('v').unwrap_or(v);
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
}

async fn fetch(client: &reqwest::Client) -> anyhow::Result<Release> {
    let r: GithubRelease = client
        .get(RELEASES_URL)
        .header("accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(Release {
        version: r.tag_name.trim_start_matches('v').to_string(),
        url: r.html_url,
        published_at: r.published_at,
    })
}

/// Checks now and then every 12 hours, for the life of the server.
pub async fn watch(latest: LatestRelease) {
    let client = match reqwest::Client::builder()
        .user_agent(concat!("autokarta/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "update check disabled: no HTTP client");
            return;
        }
    };
    let mut tick = tokio::time::interval(CHECK_EVERY);
    let mut announced: Option<String> = None;
    loop {
        tick.tick().await;
        match fetch(&client).await {
            Ok(release) => {
                if is_newer(&release.version, CURRENT)
                    && announced.as_deref() != Some(&release.version)
                {
                    tracing::info!(
                        current = CURRENT,
                        available = %release.version,
                        "a newer version is available"
                    );
                    announced = Some(release.version.clone());
                }
                *latest.write().expect("release lock") = Some(release);
            }
            // Not worth more than a debug line: the app works without it.
            Err(e) => tracing::debug!(error = %e, "update check failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    /// Talks to GitHub, so not part of the normal run:
    /// `cargo test -- --ignored fetches_latest`.
    #[tokio::test]
    #[ignore]
    async fn fetches_latest_release() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .user_agent("autokarta-test")
            .build()
            .unwrap();
        let r = super::fetch(&client).await.unwrap();
        assert!(super::parse(&r.version).is_some(), "{r:?}");
        assert!(r.url.starts_with("https://github.com/"), "{r:?}");
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.3.0", "0.2.2"));
        assert!(is_newer("v0.2.10", "0.2.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.2", "0.2.2"));
        assert!(!is_newer("0.2.1", "0.2.2"));
        assert!(!is_newer("0.3.0-rc.1", "0.2.2"));
        assert!(!is_newer("latest", "0.2.2"));
        assert!(!is_newer("0.3", "0.2.2"));
    }
}
