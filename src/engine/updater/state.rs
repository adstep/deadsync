//! Runtime state for the in-app updater.
//!
//! Holds two pieces of state with very different lifetimes:
//!
//! * A *snapshot* of the most recent [`UpdateState`] — what the UI reads
//!   to decide whether to draw a banner.  Lives in memory only.
//! * A small persisted *cache* (`etag`, last seen tag, cached release)
//!   — written next to the other cache files so we can do conditional
//!   requests on the next run (the ETag does the heavy lifting against
//!   GitHub's 60/hr unauthenticated rate limit) and so the banner
//!   survives a 304 / offline launch.
//!
//! The persisted cache lives outside [`crate::config::Config`] on
//! purpose.  Config is `Copy`, copied per-frame, and exposed in the
//! user-editable `deadsync.ini`.  The updater cache contains opaque
//! ETag strings the user has no business seeing or editing.

use std::path::Path;
use std::sync::{LazyLock, RwLock};
use std::thread;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::{
    FetchOutcome, ReleaseAsset, ReleaseInfo, UpdateState, UpdaterError, classify,
    fetch_latest_release,
};
use crate::config;

/// Filename inside `cache_dir` that persists the updater cache.
pub const CACHE_FILENAME: &str = "updater_state.json";

/// Environment variable that disables the startup check regardless of
/// config (intended for `--no-update-check` and CI use).
pub const ENV_OPT_OUT: &str = "DEADSYNC_NO_UPDATE_CHECK";

/// Minimal serializable mirror of [`ReleaseAsset`].  Lives in the
/// persisted cache so a `304 Not Modified` on the next launch can
/// restore the previously-seen [`UpdateState::Available`] without an
/// extra network round-trip and without losing the `Available` snapshot
/// to GitHub's 60/hr unauthenticated rate limit.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
}

impl From<&ReleaseAsset> for CachedAsset {
    fn from(a: &ReleaseAsset) -> Self {
        Self {
            name: a.name.clone(),
            browser_download_url: a.browser_download_url.clone(),
            size: a.size,
            digest: a.digest.clone(),
        }
    }
}

impl From<CachedAsset> for ReleaseAsset {
    fn from(a: CachedAsset) -> Self {
        Self {
            name: a.name,
            browser_download_url: a.browser_download_url,
            size: a.size,
            digest: a.digest,
        }
    }
}

/// Minimal serializable mirror of [`ReleaseInfo`].  Excludes the
/// derived [`semver::Version`] (re-parsed from `tag` on load) so an
/// out-of-band tag rename in the cache file just demotes us to "no
/// snapshot available" rather than a deserialization failure.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedRelease {
    pub tag: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<CachedAsset>,
}

impl CachedRelease {
    fn from_release_info(info: &ReleaseInfo) -> Self {
        Self {
            tag: info.tag.clone(),
            html_url: info.html_url.clone(),
            body: info.body.clone(),
            published_at: info.published_at.clone(),
            assets: info.assets.iter().map(CachedAsset::from).collect(),
        }
    }

    /// Materialize back into a [`ReleaseInfo`].  Returns `None` if the
    /// stored `tag` no longer parses as semver (e.g. we shipped a
    /// non-semver tag once and renamed the scheme later).
    pub fn into_release_info(self) -> Option<ReleaseInfo> {
        let version = Version::parse(self.tag.trim_start_matches('v')).ok()?;
        Some(ReleaseInfo {
            tag: self.tag,
            version,
            html_url: self.html_url,
            body: self.body,
            published_at: self.published_at,
            assets: self.assets.into_iter().map(ReleaseAsset::from).collect(),
        })
    }
}

/// Persisted-across-launches cache.  Hand-written serde so an unknown
/// field in the JSON file from a future build doesn't crash startup.
#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdaterCache {
    /// Tag string of the last release we successfully classified.
    /// Informational; persisted alongside the ETag so the cache file
    /// records both the conditional-request key and the release it
    /// applied to.
    #[serde(default)]
    pub last_seen_tag: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    /// Last release we classified as [`UpdateState::Available`].  Set
    /// when a `Fresh` poll yields `Available`, cleared on `UpToDate`,
    /// left untouched on `UnknownLatest`.  Lets a 304 (or an offline
    /// startup) re-materialize the banner without re-fetching.
    #[serde(default)]
    pub cached_release: Option<CachedRelease>,
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
///
/// Side effect: if the cache contains a previously-classified
/// [`UpdateState::Available`], reclassify it against the *current*
/// build version and seed the in-memory snapshot accordingly.  This
/// restores the menu banner immediately on launch -- before the
/// network check returns (or, for a 304, in lieu of any new info from
/// it) -- and naturally degrades a stale cached release to
/// [`UpdateState::UpToDate`] once the user has installed the update.
pub fn load_persisted_cache() {
    let path = config::dirs::app_dirs().cache_dir.join(CACHE_FILENAME);
    let loaded = load_cache_from(&path).unwrap_or_default();
    let cached_release = loaded.cached_release.clone();
    if let Ok(mut guard) = CACHE.write() {
        *guard = loaded;
    }
    if let Some(release) = cached_release.and_then(CachedRelease::into_release_info) {
        let state = classify(release);
        // Don't shadow a fresher snapshot if one already exists (e.g.
        // an integration test seeded one before calling this).
        if SNAPSHOT.read().ok().is_none_or(|g| g.is_none()) {
            replace_snapshot(state);
        }
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
    EnvOptOut,
}

/// Decide whether a check should run.  Pure so it can be exhaustively
/// unit-tested without IO.
///
/// The startup poll is intentionally *not* throttled by elapsed-time:
/// every check sends an `If-None-Match` ETag (see [`run_check_once`]),
/// so the steady state is a 304 with an empty body.  That's cheap
/// enough on both sides that adding a "have we checked recently?"
/// gate would add complexity without buying us anything material on
/// the unauthenticated 60/hr GitHub budget.  Manual "Check now"
/// requests don't go through this decision at all.
pub fn decide(env_opt_out: bool) -> Decision {
    if env_opt_out {
        Decision::Skip(SkipReason::EnvOptOut)
    } else {
        Decision::Check
    }
}

fn env_opt_out() -> bool {
    std::env::var_os(ENV_OPT_OUT).is_some_and(|v| !v.is_empty())
}

/// Reconcile a Fresh fetch outcome with the persisted cache.  Pure so
/// the bookkeeping is unit-testable without spinning up the worker.
///
/// The caller passes the *current* cache, the classified state, the
/// release tag (lifted out before `classify` consumed the `ReleaseInfo`),
/// and the ETag from the response.  The returned cache is what
/// [`write_cache`] should persist next.
///
/// Two subtleties:
/// * The ETag is overwritten **unconditionally**, even when the server
///   omits it.  Holding on to a stale ETag from a previous response
///   would let the next `If-None-Match` header match an unrelated
///   payload and trigger a spurious 304.
/// * `cached_release` is set on `Available`, cleared on `UpToDate`, and
///   left untouched on `UnknownLatest`: clearing on UpToDate keeps an
///   out-of-date snapshot from re-appearing after the user updates;
///   leaving it on UnknownLatest is a no-op because we never wrote one
///   in that case.
pub fn apply_fresh_to_cache(
    mut prev: UpdaterCache,
    state: &UpdateState,
    tag: &str,
    etag: Option<String>,
) -> UpdaterCache {
    prev.last_seen_tag = Some(tag.to_owned());
    prev.etag = etag;
    match state {
        UpdateState::Available(info) => {
            prev.cached_release = Some(CachedRelease::from_release_info(info));
        }
        UpdateState::UpToDate => {
            prev.cached_release = None;
        }
        UpdateState::UnknownLatest => {}
    }
    prev
}

/// Reach out to GitHub once.  Updates the in-memory snapshot and
/// persisted cache on success.  Errors are logged, not returned, so the
/// caller (a fire-and-forget thread) can stay simple.
pub fn run_check_once() {
    let agent = super::check_agent();
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
            log::debug!("Update check: 304 Not Modified");
        }
        FetchOutcome::Fresh { info, etag } => {
            let tag = info.tag.clone();
            let state = classify(info);
            replace_snapshot(state.clone());
            let next = apply_fresh_to_cache(cache(), &state, &tag, etag);
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
    let decision = decide(env_opt_out());
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

    #[test]
    fn env_opt_out_skips_check() {
        assert_eq!(decide(true), Decision::Skip(SkipReason::EnvOptOut));
    }

    #[test]
    fn no_env_opt_out_runs_check() {
        assert_eq!(decide(false), Decision::Check);
    }

    #[test]
    fn cache_round_trips_through_disk() {
        let dir = tempdir_for("updater-cache-round-trip");
        let path = dir.join(CACHE_FILENAME);
        let original = UpdaterCache {
            last_seen_tag: Some("v0.3.871".into()),
            etag: Some("\"abc\"".into()),
            cached_release: Some(CachedRelease {
                tag: "v9.9.9".into(),
                html_url: "https://example/v9.9.9".into(),
                body: "release notes".into(),
                published_at: Some("2026-04-30T00:00:00Z".into()),
                assets: vec![CachedAsset {
                    name: "deadsync-v9.9.9-x86_64-linux.tar.gz".into(),
                    browser_download_url: "https://example/v9.9.9/deadsync.tar.gz".into(),
                    size: 12345,
                    digest: Some("sha256:deadbeef".into()),
                }],
            }),
        };
        save_cache_to(&path, &original).unwrap();
        let loaded = load_cache_from(&path).expect("loads");
        assert_eq!(loaded, original);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_cache_without_cached_release_loads_with_none() {
        // Files written by older builds (before cached_release was
        // added) lack the key entirely; serde must not reject them.
        let dir = tempdir_for("updater-cache-legacy");
        let path = dir.join(CACHE_FILENAME);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            br#"{"last_checked_at":7,"last_seen_tag":"v0.1.0","etag":"\"e\""}"#,
        )
        .unwrap();
        let loaded = load_cache_from(&path).expect("loads");
        assert_eq!(loaded.last_seen_tag.as_deref(), Some("v0.1.0"));
        assert_eq!(loaded.etag.as_deref(), Some("\"e\""));
        assert!(loaded.cached_release.is_none());
        let _ = std::fs::remove_file(&path);
    }

    fn fresh_release(tag: &str) -> ReleaseInfo {
        ReleaseInfo {
            tag: tag.to_owned(),
            version: semver::Version::parse(tag.trim_start_matches('v')).unwrap(),
            html_url: format!("https://example/{tag}"),
            body: String::new(),
            published_at: None,
            assets: vec![ReleaseAsset {
                name: format!("deadsync-{tag}-x86_64-linux.tar.gz"),
                browser_download_url: format!("https://example/{tag}/asset.tar.gz"),
                size: 1,
                digest: None,
            }],
        }
    }

    #[test]
    fn apply_fresh_clears_etag_when_response_has_none() {
        // GitHub almost always returns an ETag, but if a response
        // ever omits it we must drop the previous one rather than carry
        // a stale value into the next If-None-Match (which could match
        // an unrelated payload and trigger a spurious 304).
        let prev = UpdaterCache {
            last_seen_tag: Some("v0.0.0".into()),
            etag: Some("\"old-etag\"".into()),
            cached_release: None,
        };
        let info = fresh_release("v9.9.9");
        let state = classify(info);
        let next = apply_fresh_to_cache(prev, &state, "v9.9.9", None);
        assert!(next.etag.is_none(), "stale etag must not survive a Fresh-without-etag");
        assert_eq!(next.last_seen_tag.as_deref(), Some("v9.9.9"));
    }

    #[test]
    fn apply_fresh_overwrites_etag_with_new_value() {
        let prev = UpdaterCache {
            last_seen_tag: None,
            etag: Some("\"old\"".into()),
            cached_release: None,
        };
        let state = classify(fresh_release("v9.9.9"));
        let next = apply_fresh_to_cache(prev, &state, "v9.9.9", Some("\"new\"".into()));
        assert_eq!(next.etag.as_deref(), Some("\"new\""));
    }

    #[test]
    fn apply_fresh_clears_cached_release_on_up_to_date() {
        let prev = UpdaterCache {
            last_seen_tag: None,
            etag: None,
            cached_release: Some(CachedRelease {
                tag: "v1.0.0".into(),
                html_url: String::new(),
                body: String::new(),
                published_at: None,
                assets: vec![],
            }),
        };
        let next = apply_fresh_to_cache(prev, &UpdateState::UpToDate, "v0.0.0", None);
        assert!(next.cached_release.is_none());
    }

    #[test]
    fn apply_fresh_preserves_cached_release_on_unknown_latest() {
        let cached = CachedRelease {
            tag: "v1.0.0".into(),
            html_url: String::new(),
            body: String::new(),
            published_at: None,
            assets: vec![],
        };
        let prev = UpdaterCache {
            last_seen_tag: None,
            etag: None,
            cached_release: Some(cached.clone()),
        };
        let next = apply_fresh_to_cache(prev, &UpdateState::UnknownLatest, "nightly", None);
        assert_eq!(next.cached_release, Some(cached));
    }

    #[test]
    fn cached_release_round_trips_to_release_info() {
        let cached = CachedRelease {
            tag: "v1.2.3".into(),
            html_url: "https://example/v1.2.3".into(),
            body: "notes".into(),
            published_at: None,
            assets: vec![CachedAsset {
                name: "deadsync-v1.2.3-x86_64-windows.zip".into(),
                browser_download_url: "https://example/v1.2.3/deadsync.zip".into(),
                size: 1024,
                digest: None,
            }],
        };
        let info = cached.clone().into_release_info().expect("parses");
        assert_eq!(info.tag, "v1.2.3");
        assert_eq!(info.version, semver::Version::parse("1.2.3").unwrap());
        assert_eq!(info.assets.len(), 1);
        assert_eq!(info.assets[0].size, 1024);
        // Round-trip through from_release_info should be lossless.
        let again = CachedRelease::from_release_info(&info);
        assert_eq!(again, cached);
    }

    #[test]
    fn cached_release_with_unparseable_tag_yields_none() {
        let cached = CachedRelease {
            tag: "nightly-2026-04-30".into(),
            ..Default::default()
        };
        assert!(cached.into_release_info().is_none());
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
