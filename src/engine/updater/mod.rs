//! GitHub-release based update checker.
//!
//! This module is intentionally split into pure parsing/data logic and a
//! single thin HTTP wrapper so that everything except the actual network
//! call is unit-testable against the checked-in `fixtures/` JSON.
//!
//! The HTTP wrapper supports `If-None-Match` so we can re-check on launch
//! without re-downloading the (~14 KB) JSON payload, and it returns a typed
//! [`FetchOutcome`] that distinguishes a fresh response from a 304.

use crate::engine::version;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

/// Owner/repo of the upstream release feed.  Centralised so test fixtures
/// and CI artifacts use the same string.
pub const RELEASES_REPO: &str = "pnn64/deadsync";

/// Endpoint for the most recent non-prerelease, non-draft release.
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/pnn64/deadsync/releases/latest";

/// User-Agent header value sent with every request.  GitHub rejects API
/// calls that omit a UA.  Includes the build version so server-side logs
/// can correlate stale clients.
#[inline]
pub fn user_agent() -> String {
    format!("deadsync/{} (+https://github.com/pnn64/deadsync)", env!("CARGO_PKG_VERSION"))
}

/// Networking timeout applied to update-check HTTP calls.  Distinct from
/// the score-submission default so we don't block startup behind a slow
/// network indefinitely.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub html_url: String,
    pub body: String,
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    /// The build is on or ahead of the latest published release.
    UpToDate,
    /// A newer release is available.
    Available(ReleaseInfo),
    /// The latest tag could not be parsed as semver (e.g. someone pushed a
    /// tag like `nightly-…`).  Surfaced so the UI can decline to display
    /// stale "update available" banners on garbage tags.
    UnknownLatest,
}

/// Outcome of an HTTP poll against the releases endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchOutcome {
    /// Server responded `304 Not Modified` for the supplied ETag.
    NotModified,
    /// Server returned a fresh payload.
    Fresh {
        info: ReleaseInfo,
        etag: Option<String>,
    },
}

#[derive(Debug)]
pub enum UpdaterError {
    Network(String),
    HttpStatus(u16),
    RateLimited,
    Parse(String),
}

impl Display for UpdaterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::HttpStatus(code) => write!(f, "unexpected HTTP status {code}"),
            Self::RateLimited => f.write_str("github API rate limit exceeded"),
            Self::Parse(msg) => write!(f, "failed to parse release JSON: {msg}"),
        }
    }
}

impl Error for UpdaterError {}

/* ---------- raw JSON shape ---------- */

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Parse a GitHub Releases JSON payload into [`ReleaseInfo`].
///
/// Unknown tags (those that don't parse as semver) cause this to return
/// `Err(UpdaterError::Parse(..))` rather than silently dropping the result,
/// because the alternative (treating an unparseable tag as "up to date")
/// would mask CI mistakes.
pub fn parse_release_json(bytes: &[u8]) -> Result<ReleaseInfo, UpdaterError> {
    let raw: RawRelease = serde_json::from_slice(bytes)
        .map_err(|err| UpdaterError::Parse(err.to_string()))?;
    let version = version::parse_release_tag(&raw.tag_name).ok_or_else(|| {
        UpdaterError::Parse(format!("tag '{}' is not valid semver", raw.tag_name))
    })?;
    let assets = raw
        .assets
        .into_iter()
        .map(|a| ReleaseAsset {
            name: a.name,
            browser_download_url: a.browser_download_url,
            size: a.size,
        })
        .collect();
    Ok(ReleaseInfo {
        tag: raw.tag_name,
        version,
        html_url: raw.html_url,
        body: raw.body,
        published_at: raw.published_at,
        assets,
    })
}

/// Compare a release against the current build and decide what to surface.
#[inline]
pub fn classify(latest: ReleaseInfo) -> UpdateState {
    let current = version::current();
    if version::is_newer(&latest.version, &current) {
        UpdateState::Available(latest)
    } else {
        UpdateState::UpToDate
    }
}

/// Fetch the latest release from GitHub.
///
/// `agent` is taken by reference so callers can plug in a configured ureq
/// agent (we use the shared one from `engine::network` in production but
/// tests can construct a no-network agent if needed).
///
/// Pass `etag = Some(prev)` to enable conditional requests; the server
/// returns 304 when the release hasn't changed and we avoid re-parsing.
pub fn fetch_latest_release(
    agent: &ureq::Agent,
    etag: Option<&str>,
) -> Result<FetchOutcome, UpdaterError> {
    let mut request = agent
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", user_agent().as_str())
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(prev) = etag {
        request = request.header("If-None-Match", prev);
    }

    let response = match request.call() {
        Ok(resp) => resp,
        Err(err) => return Err(UpdaterError::Network(err.to_string())),
    };

    let status = response.status().as_u16();
    if status == 304 {
        return Ok(FetchOutcome::NotModified);
    }
    if status == 403 {
        // GitHub returns 403 for rate-limit exhaustion; distinguish so the
        // UI can show a friendlier message.
        if let Some(remaining) = response
            .headers()
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            && remaining == "0"
        {
            return Err(UpdaterError::RateLimited);
        }
        return Err(UpdaterError::HttpStatus(status));
    }
    if !(200..300).contains(&status) {
        return Err(UpdaterError::HttpStatus(status));
    }

    let etag = response
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    let bytes = response
        .into_body()
        .read_to_vec()
        .map_err(|err| UpdaterError::Network(err.to_string()))?;
    let info = parse_release_json(&bytes)?;
    Ok(FetchOutcome::Fresh { info, etag })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("fixtures/latest_release.json");

    #[test]
    fn parses_real_fixture() {
        let info = parse_release_json(FIXTURE).expect("fixture parses");
        assert_eq!(info.tag, "v0.3.871");
        assert_eq!(info.version, Version::new(0, 3, 871));
        assert!(info.html_url.contains("v0.3.871"));
        assert_eq!(info.assets.len(), 6, "fixture should expose 6 assets");
        let win = info
            .assets
            .iter()
            .find(|a| a.name == "deadsync-v0.3.871-x86_64-windows.zip")
            .expect("windows asset present");
        assert!(win.size > 1_000_000, "size should be the real archive size");
        assert!(win.browser_download_url.starts_with("https://github.com/"));
    }

    #[test]
    fn classify_up_to_date_when_versions_match() {
        let mut info = parse_release_json(FIXTURE).unwrap();
        info.version = version::current();
        info.tag = format!("v{}", version::current());
        assert_eq!(classify(info), UpdateState::UpToDate);
    }

    #[test]
    fn classify_available_when_remote_newer() {
        let mut info = parse_release_json(FIXTURE).unwrap();
        let cur = version::current();
        info.version = Version::new(cur.major, cur.minor, cur.patch + 1);
        info.tag = format!("v{}", info.version);
        assert!(matches!(classify(info), UpdateState::Available(_)));
    }

    #[test]
    fn classify_up_to_date_when_remote_older() {
        let mut info = parse_release_json(FIXTURE).unwrap();
        info.version = Version::new(0, 0, 1);
        info.tag = "v0.0.1".to_string();
        assert_eq!(classify(info), UpdateState::UpToDate);
    }

    #[test]
    fn rejects_invalid_tag() {
        let bad = br#"{"tag_name":"nightly","html_url":"x","assets":[]}"#;
        assert!(matches!(
            parse_release_json(bad),
            Err(UpdaterError::Parse(_))
        ));
    }

    #[test]
    fn rejects_garbage_payload() {
        assert!(matches!(
            parse_release_json(b"not json"),
            Err(UpdaterError::Parse(_))
        ));
    }

    #[test]
    fn user_agent_includes_version() {
        let ua = user_agent();
        assert!(ua.starts_with("deadsync/"));
        assert!(ua.contains(env!("CARGO_PKG_VERSION")));
    }
}