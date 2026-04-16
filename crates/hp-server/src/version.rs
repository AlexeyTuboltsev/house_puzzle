use std::sync::Arc;
use parking_lot::RwLock;
use serde::Serialize;

/// Current app version baked in at compile time from Cargo.toml.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/AlexeyTuboltsev/house_puzzle/releases/latest";

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
}

pub type VersionState = Arc<RwLock<VersionInfo>>;

pub fn new_version_state() -> VersionState {
    Arc::new(RwLock::new(VersionInfo {
        current: CURRENT_VERSION.to_string(),
        latest: None,
        update_available: false,
        release_url: None,
    }))
}

/// Background task: check GitHub releases once immediately, then every hour.
pub async fn run_version_checker(state: VersionState) {
    // Check immediately on startup, then every hour
    check_and_update(&state).await;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    interval.tick().await; // consume the first immediate tick (already fired above)
    loop {
        interval.tick().await;
        check_and_update(&state).await;
    }
}

async fn check_and_update(state: &VersionState) {
    match fetch_latest_release().await {
        Ok((tag, url)) => {
            let latest = strip_v_prefix(&tag);
            let update_available = is_newer(&latest, CURRENT_VERSION);
            let mut info = state.write();
            info.latest = Some(latest.to_string());
            info.update_available = update_available;
            info.release_url = Some(url);
            if update_available {
                eprintln!("[version] New release available: {} (current: {})", latest, CURRENT_VERSION);
            }
        }
        Err(e) => {
            eprintln!("[version] GitHub release check failed: {e}");
        }
    }
}

async fn fetch_latest_release() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .user_agent("house-puzzle-version-checker/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp: serde_json::Value = client
        .get(GITHUB_API_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let tag = resp["tag_name"]
        .as_str()
        .ok_or("missing tag_name")?
        .to_string();
    let url = resp["html_url"]
        .as_str()
        .ok_or("missing html_url")?
        .to_string();

    Ok((tag, url))
}

/// Strip leading 'v' or 'V' from a version tag (e.g. "v0.2.0" → "0.2.0").
fn strip_v_prefix(tag: &str) -> &str {
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
}

/// Simple semver comparison: returns true if `latest` > `current`.
/// Falls back to string comparison if parsing fails.
fn is_newer(latest: &str, current: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        let major = parts[0].parse::<u64>().ok()?;
        let minor = parts[1].parse::<u64>().ok()?;
        let patch = parts[2].parse::<u64>().ok()?;
        Some((major, minor, patch))
    }

    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest > current,
    }
}
