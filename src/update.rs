use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO_OWNER: &str = "jiunjiun";
const REPO_NAME: &str = "ccu";
const BIN_NAME: &str = "ccu";
/// How long a cached "latest version" answer stays valid. Network-light: at
/// most one HTTPS request per day no matter how often `ccu` is invoked.
const CACHE_TTL_SECS: i64 = 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct VersionCache {
    checked_at: i64,
    latest_version: String,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("ccu").join("version.json"))
}

fn read_cache() -> Option<VersionCache> {
    let path = cache_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(latest: &str) {
    let Some(path) = cache_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let cache = VersionCache {
        checked_at: now_unix(),
        latest_version: latest.to_string(),
    };
    if let Ok(raw) = serde_json::to_string(&cache) {
        let _ = std::fs::write(&path, raw);
    }
}

fn fetch_latest_from_github() -> Option<String> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .ok()?
        .fetch()
        .ok()?;
    releases.first().map(|r| r.version.trim_start_matches('v').to_string())
}

fn newer_than_current(latest: &str) -> bool {
    let current = env!("CARGO_PKG_VERSION");
    let latest = latest.trim_start_matches('v');
    self_update::version::bump_is_greater(current, latest).unwrap_or(false)
}

fn fresh_cached_version() -> Option<String> {
    read_cache().and_then(|c| {
        (now_unix() - c.checked_at < CACHE_TTL_SECS).then_some(c.latest_version)
    })
}

/// Latest-version checker. Cache-hit settles synchronously without a thread;
/// cache-miss kicks off a background fetch so short commands like `ccu today`
/// don't pay GitHub-API latency on the main path.
pub struct UpdateChecker {
    rx: mpsc::Receiver<Option<String>>,
}

impl UpdateChecker {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        if let Some(cached) = fresh_cached_version() {
            let _ = tx.send(Some(cached).filter(|v| newer_than_current(v)));
        } else {
            std::thread::spawn(move || {
                let latest = fetch_latest_from_github().inspect(|v| write_cache(v));
                let _ = tx.send(latest.filter(|v| newer_than_current(v)));
            });
        }
        UpdateChecker { rx }
    }

    /// Try to read the result, giving the background thread up to `timeout`.
    /// Returns `Some(latest_version)` only when an upgrade is available.
    pub fn try_recv(&self, timeout: Duration) -> Option<String> {
        self.rx.recv_timeout(timeout).ok().flatten()
    }
}

/// Format the "new version available" banner, dimming when stderr is a TTY so
/// it sits visually below the main output instead of competing with it.
pub fn format_banner(latest: &str) -> String {
    let core = format!("→ ccu v{latest} available, run `ccu update` to upgrade");
    use std::io::IsTerminal;
    let use_color = std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal();
    if use_color {
        format!("\x1b[38;5;242m{core}\x1b[39m")
    } else {
        core
    }
}

/// Replace the current binary with the latest GitHub Release matching this
/// host's target triple. Asks self_update to download/extract/install in one
/// call; progress bar goes to stderr.
pub fn run_update() -> Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;
    if status.updated() {
        println!("Updated to v{}", status.version());
    } else {
        println!("Already up to date (v{})", status.version());
    }
    Ok(())
}
