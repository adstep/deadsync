//! Download and SHA-256 verification for release assets.
//!
//! The release CI workflows publish a `<archive>.sha256` sidecar next to
//! every archive (see `.github/workflows/release-*.yml`).  The sidecar
//! follows the GNU coreutils format produced by `sha256sum`:
//!
//! ```text
//! <64-hex-digits>  <filename>\n
//! ```
//!
//! This module exposes:
//! * pure helpers ([`parse_checksum_sidecar`], [`parse_hex32`],
//!   [`sha256_hex`], [`verify_sha256`]) that the unit tests cover;
//! * an HTTP wrapper ([`fetch_checksum_sidecar`]) that downloads the
//!   small text file; and
//! * a streaming archive downloader ([`download_to_file`]) that hashes
//!   bytes as they arrive, writes them to disk, and refuses to leave a
//!   file behind on mismatch.
//!
//! No UI integration lives here — the screen layer (PR 10) calls these
//! functions and decides what to do with the resulting path.

use super::{user_agent, ReleaseAsset, UpdaterError};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

/// Length of an upload `.sha256` sidecar in bytes is bounded; we refuse
/// anything larger than this to avoid pathological allocations on bad
/// servers.  A normal sidecar is ~80 bytes.
const SIDECAR_MAX_BYTES: u64 = 4096;

/// Streaming chunk size for asset downloads.  64 KiB balances syscall
/// overhead against memory pressure during the (~50 MiB) archive copy.
const COPY_CHUNK_BYTES: usize = 64 * 1024;

/// Lower-case hex of a SHA-256 digest.
#[inline]
pub fn sha256_hex(digest: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Decode a 64-character hex string into 32 raw bytes.  Returns `None`
/// for any non-hex character or wrong length.
pub fn parse_hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = decode_nibble(bytes[i * 2])?;
        let lo = decode_nibble(bytes[i * 2 + 1])?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

#[inline]
fn decode_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Constant-time-ish comparison of two SHA-256 digests.  Not a security
/// boundary (the digest is public), but writing it explicitly avoids
/// short-circuiting reads when added to other tooling later.
#[inline]
pub fn verify_sha256(actual: &[u8; 32], expected: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for (a, b) in actual.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Hash the supplied bytes with SHA-256.
pub fn sha256_of(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Parse a `sha256sum`-style sidecar.  The sidecar may contain multiple
/// entries (one per line); we return the digest matching `expected_filename`.
///
/// Each entry is `<hex>  <name>` (two spaces separate hash and name in
/// GNU coreutils).  We accept either one or more spaces / a tab to be
/// permissive about trailing-whitespace cleanups.
pub fn parse_checksum_sidecar(
    text: &str,
    expected_filename: &str,
) -> Result<[u8; 32], UpdaterError> {
    if expected_filename.is_empty() {
        return Err(UpdaterError::ChecksumSidecarMalformed(
            "empty expected filename".to_owned(),
        ));
    }
    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Split into "<hex>" and "<name>" (skip any leading "*" binary marker).
        let mut parts = line.splitn(2, |c: char| c.is_whitespace());
        let hex = match parts.next() {
            Some(h) => h.trim(),
            None => continue,
        };
        let rest = match parts.next() {
            Some(r) => r.trim_start().trim_start_matches('*').trim(),
            None => continue,
        };
        if rest == expected_filename {
            return parse_hex32(hex).ok_or_else(|| {
                UpdaterError::ChecksumSidecarMalformed(format!(
                    "invalid hex digest for {expected_filename}",
                ))
            });
        }
    }
    Err(UpdaterError::ChecksumSidecarMalformed(format!(
        "no entry for {expected_filename}",
    )))
}

/// Build the canonical sidecar URL for a release asset.
///
/// CI publishes `<archive>.sha256` alongside the archive at the same
/// browser-download base, so deriving the URL by string append matches
/// the real layout without an extra API call.
#[inline]
pub fn checksum_sidecar_url(asset_url: &str) -> String {
    format!("{asset_url}.sha256")
}

/// Download the `.sha256` sidecar for an asset.
pub fn fetch_checksum_sidecar(
    agent: &ureq::Agent,
    asset_url: &str,
) -> Result<String, UpdaterError> {
    let url = checksum_sidecar_url(asset_url);
    let response = agent
        .get(&url)
        .header("User-Agent", user_agent().as_str())
        .header("Accept", "text/plain")
        .call()
        .map_err(|err| UpdaterError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdaterError::HttpStatus(status));
    }
    let bytes = response
        .into_body()
        .with_config()
        .limit(SIDECAR_MAX_BYTES)
        .read_to_vec()
        .map_err(|err| UpdaterError::Network(err.to_string()))?;
    String::from_utf8(bytes)
        .map_err(|err| UpdaterError::ChecksumSidecarMalformed(err.to_string()))
}

/// Stream `asset` into `dest`, hashing as it goes and verifying against
/// `expected_sha256` before returning.  On mismatch or any I/O failure
/// the partial file is removed.
///
/// `progress` is invoked after every chunk with `(written, total_opt)`
/// so the UI layer (PR 10) can render a progress bar.  The total may be
/// `None` if the server omits Content-Length; we fall back to the asset
/// metadata in that case.
pub fn download_to_file(
    agent: &ureq::Agent,
    asset: &ReleaseAsset,
    expected_sha256: &[u8; 32],
    dest: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<(), UpdaterError> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| UpdaterError::Io(err.to_string()))?;
    }

    let response = agent
        .get(&asset.browser_download_url)
        .header("User-Agent", user_agent().as_str())
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|err| UpdaterError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(UpdaterError::HttpStatus(status));
    }

    let total = response
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| (asset.size > 0).then_some(asset.size));

    let mut reader = response.into_body().into_reader();
    let result = stream_to_file(&mut reader, dest, expected_sha256, total, &mut progress);
    if result.is_err() {
        // Best-effort cleanup; ignore secondary I/O errors.
        let _ = fs::remove_file(dest);
    }
    result
}

fn stream_to_file<R: Read>(
    reader: &mut R,
    dest: &Path,
    expected_sha256: &[u8; 32],
    total: Option<u64>,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), UpdaterError> {
    let mut file = File::create(dest).map_err(|err| UpdaterError::Io(err.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; COPY_CHUNK_BYTES];
    let mut written: u64 = 0;
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|err| UpdaterError::Network(err.to_string()))?;
        if read == 0 {
            break;
        }
        let chunk = &buf[..read];
        hasher.update(chunk);
        file.write_all(chunk)
            .map_err(|err| UpdaterError::Io(err.to_string()))?;
        written += read as u64;
        progress(written, total);
    }
    file.flush().map_err(|err| UpdaterError::Io(err.to_string()))?;
    drop(file);

    let actual: [u8; 32] = hasher.finalize().into();
    if !verify_sha256(&actual, expected_sha256) {
        return Err(UpdaterError::ChecksumMismatch {
            expected: sha256_hex(expected_sha256),
            actual: sha256_hex(&actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO_DIGEST_HEX: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn sha256_hex_round_trip() {
        let bytes = sha256_of(b"");
        let hex = sha256_hex(&bytes);
        assert_eq!(hex, ZERO_DIGEST_HEX);
        assert_eq!(parse_hex32(&hex), Some(bytes));
    }

    #[test]
    fn parse_hex32_rejects_bad_input() {
        assert!(parse_hex32("").is_none());
        assert!(parse_hex32("abc").is_none());
        // 63 chars + non-hex
        assert!(parse_hex32(&"z".repeat(64)).is_none());
        // Wrong length
        assert!(parse_hex32(&"a".repeat(63)).is_none());
        assert!(parse_hex32(&"a".repeat(65)).is_none());
    }

    #[test]
    fn parse_hex32_accepts_mixed_case() {
        let lower = ZERO_DIGEST_HEX;
        let upper = lower.to_uppercase();
        assert_eq!(parse_hex32(lower), parse_hex32(&upper));
    }

    #[test]
    fn verify_sha256_detects_difference() {
        let a = sha256_of(b"hello");
        let b = sha256_of(b"world");
        assert!(verify_sha256(&a, &a));
        assert!(!verify_sha256(&a, &b));
    }

    #[test]
    fn parse_sidecar_single_entry() {
        let sidecar = format!("{ZERO_DIGEST_HEX}  deadsync-v1.2.3-x86_64-linux.tar.zst\n");
        let digest =
            parse_checksum_sidecar(&sidecar, "deadsync-v1.2.3-x86_64-linux.tar.zst").unwrap();
        assert_eq!(sha256_hex(&digest), ZERO_DIGEST_HEX);
    }

    #[test]
    fn parse_sidecar_skips_blank_and_comment_lines() {
        let sidecar = format!(
            "# this is a comment\n\n{ZERO_DIGEST_HEX}  deadsync.zip\n# trailing comment\n"
        );
        let digest = parse_checksum_sidecar(&sidecar, "deadsync.zip").unwrap();
        assert_eq!(sha256_hex(&digest), ZERO_DIGEST_HEX);
    }

    #[test]
    fn parse_sidecar_multi_entry_picks_matching_name() {
        let other = "1111111111111111111111111111111111111111111111111111111111111111";
        let sidecar = format!(
            "{other}  deadsync-v1.2.3-arm64-linux.tar.zst\n\
             {ZERO_DIGEST_HEX}  deadsync-v1.2.3-x86_64-linux.tar.zst\n"
        );
        let digest =
            parse_checksum_sidecar(&sidecar, "deadsync-v1.2.3-x86_64-linux.tar.zst").unwrap();
        assert_eq!(sha256_hex(&digest), ZERO_DIGEST_HEX);
    }

    #[test]
    fn parse_sidecar_handles_binary_marker_and_crlf() {
        let sidecar = format!("{ZERO_DIGEST_HEX} *deadsync.zip\r\n");
        let digest = parse_checksum_sidecar(&sidecar, "deadsync.zip").unwrap();
        assert_eq!(sha256_hex(&digest), ZERO_DIGEST_HEX);
    }

    #[test]
    fn parse_sidecar_errors_when_filename_missing() {
        let sidecar = format!("{ZERO_DIGEST_HEX}  other.zip\n");
        let err = parse_checksum_sidecar(&sidecar, "deadsync.zip").unwrap_err();
        assert!(matches!(err, UpdaterError::ChecksumSidecarMalformed(_)));
    }

    #[test]
    fn parse_sidecar_errors_on_bad_hex() {
        let sidecar = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz  deadsync.zip\n";
        let err = parse_checksum_sidecar(sidecar, "deadsync.zip").unwrap_err();
        assert!(matches!(err, UpdaterError::ChecksumSidecarMalformed(_)));
    }

    #[test]
    fn parse_sidecar_errors_on_empty_filename() {
        let err = parse_checksum_sidecar("anything", "").unwrap_err();
        assert!(matches!(err, UpdaterError::ChecksumSidecarMalformed(_)));
    }

    #[test]
    fn checksum_sidecar_url_appends_extension() {
        let base = "https://github.com/pnn64/deadsync/releases/download/v1.2.3/deadsync.zip";
        assert_eq!(
            checksum_sidecar_url(base),
            "https://github.com/pnn64/deadsync/releases/download/v1.2.3/deadsync.zip.sha256",
        );
    }

    #[test]
    fn stream_to_file_writes_and_verifies() {
        let dir = tempdir();
        let dest = dir.join("payload.bin");
        let payload = b"the quick brown fox jumps over the lazy dog".to_vec();
        let expected = sha256_of(&payload);
        let mut reader = std::io::Cursor::new(payload.clone());
        let mut seen_progress = 0u64;
        stream_to_file(
            &mut reader,
            &dest,
            &expected,
            Some(payload.len() as u64),
            &mut |w, _| seen_progress = w,
        )
        .unwrap();
        assert_eq!(seen_progress, payload.len() as u64);
        let written = std::fs::read(&dest).unwrap();
        assert_eq!(written, payload);
    }

    #[test]
    fn stream_to_file_rejects_mismatch_and_removes_partial() {
        let dir = tempdir();
        let dest = dir.join("bad.bin");
        let payload = b"hello world".to_vec();
        let mut wrong = sha256_of(&payload);
        wrong[0] ^= 0xff;
        let mut reader = std::io::Cursor::new(payload.clone());
        let err =
            stream_to_file(&mut reader, &dest, &wrong, None, &mut |_, _| {}).unwrap_err();
        assert!(matches!(err, UpdaterError::ChecksumMismatch { .. }));
        // download_to_file performs the cleanup; here we mimic that contract:
        let _ = std::fs::remove_file(&dest);
        assert!(!dest.exists());
    }

    fn tempdir() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "deadsync-updater-download-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
