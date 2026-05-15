//! Non-blocking PyPI version check with 24-hour file-based cache.
//!
//! On CLI startup, spawns a background task that checks PyPI for the latest
//! `batchalign3` version. If a newer version is available, prints a one-line
//! notice to stderr. The check result is cached for 24 hours in
//! `~/.cache/batchalign3/update_check.json` (or the platform-appropriate
//! cache directory).
//!
//! Never blocks CLI startup or delays command execution. Silent on any
//! network or filesystem error. Disabled when `BATCHALIGN_NO_UPDATE_CHECK=1`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long a cached check result stays valid before re-querying PyPI.
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// PyPI JSON API endpoint for the batchalign3 package.
const PYPI_URL: &str = "https://pypi.org/pypi/batchalign3/json";

/// The version compiled into this binary.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum time to wait for the PyPI HTTP response.
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Spawn a background update check. Returns a join handle the caller can
/// optionally await at exit (with a short timeout) to ensure the notice
/// prints. The spawned task never panics — all errors are silently logged
/// at `debug` level.
pub fn spawn_update_check() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        if let Err(e) = check_and_notify().await {
            tracing::debug!("update check skipped: {e}");
        }
    })
}

/// Returns `true` if the user has opted out via environment variable.
fn is_disabled() -> bool {
    std::env::var("BATCHALIGN_NO_UPDATE_CHECK")
        .ok()
        .is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Core logic: read cache, hit PyPI if stale, print notice if outdated.
async fn check_and_notify() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if is_disabled() {
        return Ok(());
    }

    let cache_path = cache_file()?;

    // If cache is fresh, use it without a network request.
    if let Ok(cached) = read_cache(&cache_path)
        && cached.checked_at + CACHE_TTL.as_secs() > now_epoch()
    {
        if let Some(msg) = format_update_message(&cached.latest_version) {
            eprintln!("{msg}");
        }
        return Ok(());
    }

    // Cache stale or missing — fetch from PyPI.
    let client = reqwest::Client::builder().timeout(HTTP_TIMEOUT).build()?;
    let resp: PypiResponse = client.get(PYPI_URL).send().await?.json().await?;
    let latest = resp.info.version;

    // Persist for next invocation.
    let entry = CacheEntry {
        latest_version: latest.clone(),
        checked_at: now_epoch(),
    };
    let _ = write_cache(&cache_path, &entry);

    if let Some(msg) = format_update_message(&latest) {
        eprintln!("{msg}");
    }
    Ok(())
}

/// Build the one-line notice, or `None` if the installed version is current.
fn format_update_message(latest: &str) -> Option<String> {
    if is_newer(latest, CURRENT_VERSION) {
        Some(format!(
            "Note: batchalign3 {latest} is available (you have {CURRENT_VERSION}). \
             Run: uv tool upgrade batchalign3"
        ))
    } else {
        None
    }
}

/// Simple semver comparison: returns `true` if `a` is strictly newer than `b`.
///
/// Parses dot-separated integers; non-numeric suffixes (pre-release tags) are
/// ignored, so `1.0.0a1` is treated as `1.0.0`.
fn is_newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|s| {
                s.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for (x, y) in va.iter().zip(vb.iter()) {
        match x.cmp(y) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => continue,
        }
    }
    // e.g. 1.0.0.1 > 1.0.0
    va.len() > vb.len()
}

// -- PyPI JSON response (minimal subset) ------------------------------------

#[derive(serde::Deserialize)]
struct PypiResponse {
    info: PypiInfo,
}

#[derive(serde::Deserialize)]
struct PypiInfo {
    version: String,
}

// -- File-based cache -------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    latest_version: String,
    checked_at: u64,
}

/// Platform-appropriate cache file path.
fn cache_file() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let dir = dirs::cache_dir()
        .ok_or("no cache directory")?
        .join("batchalign3");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("update_check.json"))
}

fn read_cache(path: &Path) -> Result<CacheEntry, Box<dyn std::error::Error + Send + Sync>> {
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn write_cache(
    path: &Path,
    entry: &CacheEntry,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = serde_json::to_string(entry)?;
    std::fs::write(path, data)?;
    Ok(())
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_major() {
        assert!(is_newer("2.0.0", "1.0.0"));
    }

    #[test]
    fn newer_minor() {
        assert!(is_newer("1.1.0", "1.0.0"));
    }

    #[test]
    fn newer_patch() {
        assert!(is_newer("1.0.1", "1.0.0"));
    }

    #[test]
    fn equal_is_not_newer() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn older_is_not_newer() {
        assert!(!is_newer("1.0.0", "1.1.0"));
    }

    #[test]
    fn prerelease_suffix_ignored() {
        // "1.1.0a1" should parse as 1.1.0 and be newer than 1.0.0
        assert!(is_newer("1.1.0a1", "1.0.0"));
    }

    #[test]
    fn format_message_when_outdated() {
        let msg = format_update_message("999.0.0");
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("999.0.0"));
    }

    #[test]
    fn no_message_when_current() {
        // CURRENT_VERSION should not be newer than itself
        let msg = format_update_message(CURRENT_VERSION);
        assert!(msg.is_none());
    }
}
