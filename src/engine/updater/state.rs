//! Runtime state for the in-app updater.
//!
//! Holds two pieces of state with very different lifetimes:
//!
//! * A *snapshot* of the most recent [`UpdateState`] — what the UI reads
//!   to decide whether to draw a banner.  Lives in memory only.
//! * A small persisted *cache* (`last_checked_at`, `last_seen_tag`,
//!   `etag`) — written next to the other cache files so we can do
//!   conditional requests on the next run and avoid the 60/hr
//!   unauthenticated GitHub rate limit.
//!
//! The persisted cache lives outside [`crate::config::Config`] on
//! purpose.  Config is `Copy`, copied per-frame, and exposed in the user-
//! editable `Settings.ini`.  The updater cache contains opaque ETag
//! strings the user has no business seeing or editing.

use std::path::Path;
use std::sync::{LazyLock, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use super::{FetchOutcome, UpdateState, UpdaterError, classify, fetch_latest_release};
use crate::config::{self, UpdateCheckMode};
use crate::engine::network;

/// Filename inside `cache_dir` that persists the updater cache.
pub const CACHE_FILENAME: &str = "updater_state.json";

/// Daily-mode cooldown.
const DAILY_COOLDOWN_SECONDS: i64 = 24 * 60 * 60;

/// Environment variable that disables the startup check regardless of
/// config (intended for `--no-update-check` and CI use).
pub const ENV_OPT_OUT: &str = "DEADSYNC_NO_UPDATE_CHECK";

/// Persisted-across-launches cache.  Hand-written serde so an unknown
/// field in the JSON file from a future build doesn't crash startup.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdaterCache {
    #[serde(default)]
    pub last_checked_at: Option<i64>,
    #[serde(default)]
    pub last_seen_tag: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
}

static CACHE: LazyLock<RwLock<UpdaterCache>> = LazyLock::new(|| RwLock::new(UpdaterCache::default()));
static SNAPSHOT: LazyLock<RwLock<Option<UpdateState>>> = LazyLock::new(|| RwLock::new(None));

/// Replace the in-memory snapshot.  Used by both the passive startup
/// check and the manual "Check now" worker in [`crate::engine::updater::action`].
pub fn replace_snapshot(state: UpdateState) {
    if let Ok(mut snap) = SNAPSHOT.write() {
        *snap = Some(state);
    }
}

/// Snapshot of the latest [`UpdateState`] for the UI.  `None` when no
/// check has completed yet (or the check failed silently).
pub fn snapshot() -> Option<UpdateState> {
    SNAPSHOT.read().ok().and_then(|guard| guard.clone())
}

/// Read-only copy of the persisted cache.
pub fn cache() -> UpdaterCache {
    CACHE.read().map(|c| c.clone()).unwrap_or_default()
}

/// Replace the cache and persist it to `cache_dir/CACHE_FILENAME`.
fn write_cache(new_cache: UpdaterCache) {
    {
        let mut guard = match CACHE.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        *guard = new_cache.clone();
    }
    let path = config::dirs::app_dirs().cache_dir.join(CACHE_FILENAME);
    if let Err(err) = save_cache_to(&path, &new_cache) {
        log::warn!("Failed to persist updater cache to {}: {err}", path.display());
    }
}

fn save_cache_to(path: &Path, cache: &UpdaterCache) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cache)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(path, json)
}

/// Load the persisted cache from disk into the in-memory copy.  Missing
/// or malformed files reset the cache to empty without erroring; this is
/// the right call at startup before [`spawn_startup_check`].
pub fn load_persisted_cache() {
    let path = config::dirs::app_dirs().cache_dir.join(CACHE_FILENAME);
    let loaded = load_cache_from(&path).unwrap_or_default();
    if let Ok(mut guard) = CACHE.write() {
        *guard = loaded;
    }
}

fn load_cache_from(path: &Path) -> Option<UpdaterCache> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<UpdaterCache>(&bytes).ok()
}

/// Pure decision: should we run the check right now?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Check,
    Skip(SkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Disabled,
    EnvOptOut,
    DailyCooldown,
}

/// Decide whether a check should run.  Pure so it can be exhaustively
/// unit-tested without IO.
pub fn decide(
    mode: UpdateCheckMode,
    cache: &UpdaterCache,
    now_unix: i64,
    env_opt_out: bool,
) -> Decision {
    if env_opt_out {
        return Decision::Skip(SkipReason::EnvOptOut);
    }
    match mode {
        UpdateCheckMode::Disabled => Decision::Skip(SkipReason::Disabled),
        UpdateCheckMode::OnStartup => Decision::Check,
        UpdateCheckMode::Daily => match cache.last_checked_at {
            Some(prev) if now_unix.saturating_sub(prev) < DAILY_COOLDOWN_SECONDS => {
                Decision::Skip(SkipReason::DailyCooldown)
            }
            _ => Decision::Check,
        },
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn env_opt_out() -> bool {
    std::env::var_os(ENV_OPT_OUT).is_some_and(|v| !v.is_empty())
}

/// Reach out to GitHub once.  Updates the in-memory snapshot and
/// persisted cache on success.  Errors are logged, not returned, so the
/// caller (a fire-and-forget thread) can stay simple.
pub fn run_check_once() {
    let agent = network::get_agent();
    let prev_etag = cache().etag.clone();

    let outcome = match fetch_latest_release(&agent, prev_etag.as_deref()) {
        Ok(o) => o,
        Err(UpdaterError::Network(msg)) => {
            log::info!("Update check failed (network): {msg}");
            return;
        }
        Err(UpdaterError::HttpStatus(code)) => {
            log::warn!("Update check returned HTTP {code}");
            return;
        }
        Err(UpdaterError::RateLimited) => {
            log::info!("Update check skipped: GitHub rate limit reached");
            return;
        }
        Err(UpdaterError::Parse(msg)) => {
            log::warn!("Update check parse error: {msg}");
            return;
        }
        Err(other) => {
            // Download/checksum errors aren't producible by the JSON
            // poll path today, but a catch-all keeps the match exhaustive
            // as the error enum grows.
            log::warn!("Update check failed: {other}");
            return;
        }
    };

    match outcome {
        FetchOutcome::NotModified => {
            // Server confirmed nothing changed; just bump the timestamp
            // so Daily mode doesn't re-fire immediately.
            let mut next = cache();
            next.last_checked_at = Some(now_unix());
            write_cache(next);
            log::debug!("Update check: 304 Not Modified");
        }
        FetchOutcome::Fresh { info, etag } => {
            let tag = info.tag.clone();
            let state = classify(info);
            replace_snapshot(state.clone());
            let mut next = cache();
            next.last_checked_at = Some(now_unix());
            next.last_seen_tag = Some(tag);
            if etag.is_some() {
                next.etag = etag;
            }
            write_cache(next);
            match state {
                UpdateState::UpToDate => log::info!("Update check: up to date"),
                UpdateState::Available(ref info) => {
                    log::info!("Update available: {} ({})", info.tag, info.html_url);
                }
                UpdateState::UnknownLatest => {
                    log::info!("Update check: latest release tag did not parse as semver");
                }
            }
        }
    }
}

/// Spawn a background thread to run the startup update check, if the
/// current configuration says to.  Returns `Decision::Skip(...)` if no
/// thread was spawned, otherwise [`Decision::Check`].
pub fn spawn_startup_check() -> Decision {
    let cfg = config::get();
    let cache_now = cache();
    let decision = decide(cfg.update_check_mode, &cache_now, now_unix(), env_opt_out());
    match decision {
        Decision::Skip(reason) => {
            log::debug!("Update check skipped: {reason:?}");
        }
        Decision::Check => {
            thread::Builder::new()
                .name("deadsync-updater".to_string())
                .spawn(run_check_once)
                .map(|_| ())
                .unwrap_or_else(|err| {
                    log::warn!("Failed to spawn updater thread: {err}");
                });
        }
    }
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cache_with_last_checked(prev: i64) -> UpdaterCache {
        UpdaterCache { last_checked_at: Some(prev), ..UpdaterCache::default() }
    }

    #[test]
    fn disabled_mode_always_skips() {
        let c = UpdaterCache::default();
        assert_eq!(
            decide(UpdateCheckMode::Disabled, &c, 1_700_000_000, false),
            Decision::Skip(SkipReason::Disabled)
        );
    }

    #[test]
    fn env_opt_out_overrides_every_mode() {
        let c = UpdaterCache::default();
        for mode in [UpdateCheckMode::Disabled, UpdateCheckMode::OnStartup, UpdateCheckMode::Daily] {
            assert_eq!(
                decide(mode, &c, 1_700_000_000, true),
                Decision::Skip(SkipReason::EnvOptOut),
                "mode {mode:?}"
            );
        }
    }

    #[test]
    fn on_startup_always_checks_when_not_opted_out() {
        let c = cache_with_last_checked(1_699_999_990);
        assert_eq!(
            decide(UpdateCheckMode::OnStartup, &c, 1_700_000_000, false),
            Decision::Check
        );
    }

    #[test]
    fn daily_skips_within_24h_window() {
        let now = 1_700_000_000;
        let c = cache_with_last_checked(now - DAILY_COOLDOWN_SECONDS + 1);
        assert_eq!(
            decide(UpdateCheckMode::Daily, &c, now, false),
            Decision::Skip(SkipReason::DailyCooldown)
        );
    }

    #[test]
    fn daily_runs_after_24h_window() {
        let now = 1_700_000_000;
        let c = cache_with_last_checked(now - DAILY_COOLDOWN_SECONDS);
        assert_eq!(
            decide(UpdateCheckMode::Daily, &c, now, false),
            Decision::Check
        );
    }

    #[test]
    fn daily_runs_when_no_prior_check() {
        let c = UpdaterCache::default();
        assert_eq!(
            decide(UpdateCheckMode::Daily, &c, 1_700_000_000, false),
            Decision::Check
        );
    }

    #[test]
    fn cache_round_trips_through_disk() {
        let dir = tempdir_for("updater-cache-round-trip");
        let path = dir.join(CACHE_FILENAME);
        let original = UpdaterCache {
            last_checked_at: Some(123_456),
            last_seen_tag: Some("v0.3.871".into()),
            etag: Some("\"abc\"".into()),
        };
        save_cache_to(&path, &original).unwrap();
        let loaded = load_cache_from(&path).expect("loads");
        assert_eq!(loaded, original);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_cache_file_loads_as_default() {
        let dir = tempdir_for("updater-cache-missing");
        let path = dir.join("does-not-exist.json");
        assert!(load_cache_from(&path).is_none());
    }

    #[test]
    fn malformed_cache_file_loads_as_default() {
        let dir = tempdir_for("updater-cache-malformed");
        let path = dir.join(CACHE_FILENAME);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"this is not json").unwrap();
        assert!(load_cache_from(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }

    fn tempdir_for(stem: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("deadsync-{stem}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}
