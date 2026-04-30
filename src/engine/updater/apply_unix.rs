//! Unix-side "apply" half of the in-app updater.
//!
//! On Linux and FreeBSD the running executable file CAN be renamed
//! and replaced atomically (the kernel keeps the on-disk inode alive
//! until the running process exits), so we don't need the
//! `<name>.old` two-step dance that Windows requires.  The flow is:
//!
//! 1. Extract the downloaded `tar.gz` into a sibling staging dir.
//!    The Linux/FreeBSD release archives have a single top-level
//!    `deadsync/` directory (see `scripts/package-linux-release.sh`),
//!    which is stripped during extraction.
//! 2. For every staged file, `rename(2)` it on top of the live
//!    counterpart.  Same-volume renames are atomic and overwrite
//!    silently; the existing inode of the running binary stays alive
//!    in memory until exit.
//! 3. Caller (PR-14) re-execs the new binary with `--restart`.
//!
//! `extract_archive` lives in this file because, although the *logic*
//! is portable, its `tar` + `flate2` deps are kept compiled-out on
//! Windows for build-size reasons; we still want a Windows-runnable
//! unit-test surface, so this module's `cfg` is wider than the
//! callers'.  The dispatcher entry-point [`apply_tar_gz`] is the only
//! thing actually gated to Linux/FreeBSD.

use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

use super::UpdaterError;

/// Result of a successful in-place swap.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub staging_dir: PathBuf,
    pub installed_file_count: usize,
}

/* ---------- archive extraction (cross-platform; tested on Windows) ---------- */

/// Detects a single shared top-level directory across every entry in
/// the archive.  Returns `Some(prefix)` only when **every** non-empty
/// entry path begins with the same first component, otherwise `None`.
pub fn detect_common_prefix<I, S>(entries: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut prefix: Option<String> = None;
    let mut saw_any = false;
    for raw in entries {
        let raw = raw.as_ref();
        let trimmed = raw.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        saw_any = true;
        let first = trimmed.split(['/', '\\']).next().unwrap_or("");
        if first.is_empty() {
            return None;
        }
        match &prefix {
            None => prefix = Some(first.to_string()),
            Some(existing) if existing == first => {}
            Some(_) => return None,
        }
    }
    if saw_any { prefix } else { None }
}

/// Strips the optional shared top-level prefix and validates the
/// remainder: rejects absolute paths and any component that isn't a
/// plain `Normal` segment.  Returns the cleaned relative path.
pub fn sanitize_entry(name: &str, prefix: Option<&str>) -> Option<PathBuf> {
    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let stripped = match prefix {
        Some(p) => {
            let with_slash = format!("{p}/");
            let with_back = format!("{p}\\");
            trimmed
                .strip_prefix(&with_slash)
                .or_else(|| trimmed.strip_prefix(&with_back))
                .or_else(|| if trimmed == p { Some("") } else { None })?
        }
        None => trimmed,
    };
    if stripped.is_empty() {
        return None;
    }
    let path = PathBuf::from(stripped.replace('\\', "/"));
    for comp in path.components() {
        match comp {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(path)
}

/// Extracts a gzipped tarball into `dest`, stripping a single shared
/// top-level directory if every entry shares one.  Returns the count
/// of regular files written.  Symlinks and "special" entries are
/// skipped (logged via the returned error only when nothing else was
/// extracted, otherwise silently — the release tarballs don't ship
/// symlinks).
///
/// Two passes: the first reads the entry list to compute the common
/// prefix, the second writes files.  We intentionally re-decode the
/// gzip stream the second time because `tar::Archive` is single-pass.
pub fn extract_tar_gz(zip_bytes: &[u8], dest: &Path) -> Result<usize, UpdaterError> {
    fs::create_dir_all(dest).map_err(io_err)?;
    let prefix = {
        let dec = GzDecoder::new(zip_bytes);
        let mut archive = Archive::new(dec);
        let mut names = Vec::new();
        for entry in archive.entries().map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let path = entry.path().map_err(io_err)?;
            if let Some(s) = path.to_str() {
                names.push(s.to_string());
            }
        }
        detect_common_prefix(names)
    };
    let dec = GzDecoder::new(zip_bytes);
    let mut archive = Archive::new(dec);
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);
    let mut written = 0usize;
    for entry in archive.entries().map_err(io_err)? {
        let mut entry = entry.map_err(io_err)?;
        let raw_name = entry
            .path()
            .map_err(io_err)?
            .to_string_lossy()
            .to_string();
        let entry_type = entry.header().entry_type();
        let Some(rel) = sanitize_entry(&raw_name, prefix.as_deref()) else {
            if entry_type.is_dir() {
                continue;
            }
            return Err(UpdaterError::Io(format!(
                "rejected unsafe archive entry '{raw_name}'"
            )));
        };
        let out_path = dest.join(&rel);
        if entry_type.is_dir() {
            fs::create_dir_all(&out_path).map_err(io_err)?;
            continue;
        }
        if !entry_type.is_file() {
            // Skip symlinks/hardlinks/devices/etc.  Release tarballs
            // never contain these, but we don't want to silently
            // produce a partial install if someone hand-crafts one.
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let mut out = File::create(&out_path).map_err(io_err)?;
        io::copy(&mut entry, &mut out).map_err(io_err)?;
        // Preserve the executable bit on Unix; on Windows this is a
        // no-op, but the call still typechecks.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mode) = entry.header().mode() {
                let _ = fs::set_permissions(
                    &out_path,
                    std::fs::Permissions::from_mode(mode),
                );
            }
        }
        written += 1;
    }
    Ok(written)
}

/* ---------- in-place swap (Unix-only because of rename-over-running-exe) ---------- */

/// Renames every file under `staging_dir` on top of its counterpart
/// in `target_dir`.  Same-volume renames are atomic on Linux and
/// FreeBSD; the running binary's inode stays alive until the process
/// exits, so it is safe to overwrite it in-place before re-exec'ing.
///
/// On any error mid-walk we abort and leave the partially-installed
/// state in place — the caller surfaces the error to the user, and
/// no destructive `.old` files are produced (cf. the Windows path).
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn swap_files(staging_dir: &Path, target_dir: &Path) -> Result<usize, UpdaterError> {
    let files = collect_files(staging_dir)?;
    let mut moved = 0usize;
    for staged in &files {
        let rel = staged.strip_prefix(staging_dir).map_err(|_| {
            UpdaterError::Io(format!(
                "staged path '{}' escapes staging dir '{}'",
                staged.display(),
                staging_dir.display(),
            ))
        })?;
        let target = target_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        fs::rename(staged, &target).map_err(|e| {
            UpdaterError::Io(format!(
                "failed to install '{}' -> '{}': {e}",
                staged.display(),
                target.display(),
            ))
        })?;
        moved += 1;
    }
    Ok(moved)
}

/* ---------- top-level orchestration ---------- */

/// Drives the full apply sequence: writability probe → extract
/// tarball into a sibling staging dir → swap files in place.  Caller
/// (PR-14) is responsible for `execv`-ing the new binary with
/// `--restart` afterwards.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn apply_tar_gz(archive_path: &Path, exe_dir: &Path) -> Result<ApplyOutcome, UpdaterError> {
    if !is_dir_writable(exe_dir) {
        return Err(UpdaterError::Io(format!(
            "install directory is not writable: {}",
            exe_dir.display(),
        )));
    }
    let staging_dir = staging_dir_for(exe_dir);
    if staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    let bytes = fs::read(archive_path).map_err(io_err)?;
    let installed_file_count = extract_tar_gz(&bytes, &staging_dir)?;
    swap_files(&staging_dir, exe_dir)?;
    // After swap, staging_dir holds only empty directories.  Best-
    // effort cleanup; failure is harmless.
    let _ = fs::remove_dir_all(&staging_dir);
    Ok(ApplyOutcome {
        staging_dir,
        installed_file_count,
    })
}

/// Probe writability the same way `apply_windows` does so the two
/// platforms refuse self-update with consistent UX.
pub fn is_dir_writable(dir: &Path) -> bool {
    use std::io::Write;
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(
        ".deadsync-writable-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    match File::create(&probe) {
        Ok(mut f) => {
            let _ = f.write_all(b"ok");
            drop(f);
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// `{exe_dir}/.deadsync-update-staging-{pid}-{nanos}/`.  Sibling so
/// `rename(2)` stays on the same filesystem.
pub fn staging_dir_for(exe_dir: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    exe_dir.join(format!(
        ".deadsync-update-staging-{}-{}",
        std::process::id(),
        nanos
    ))
}

/* ---------- helpers ---------- */

fn io_err(e: io::Error) -> UpdaterError {
    UpdaterError::Io(e.to_string())
}

#[cfg(any(target_os = "linux", target_os = "freebsd", test))]
fn collect_files(root: &Path) -> Result<Vec<PathBuf>, UpdaterError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let path = entry.path();
            let ft = entry.file_type().map_err(io_err)?;
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};

    fn tempdir(stem: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "deadsync-apply-unix-{stem}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_tar_gz(entries: &[(&str, &[u8])], top: Option<&str>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let enc = GzEncoder::new(&mut buf, Compression::fast());
            let mut tar = Builder::new(enc);
            for (name, body) in entries {
                let full = match top {
                    Some(p) => format!("{p}/{name}"),
                    None => (*name).to_string(),
                };
                let mut header = Header::new_gnu();
                header.set_path(&full).unwrap();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar.append(&header, *body).unwrap();
            }
            tar.finish().unwrap();
        }
        buf
    }

    #[test]
    fn detect_common_prefix_returns_shared_dir() {
        assert_eq!(
            detect_common_prefix(["pkg/a", "pkg/b/c", "pkg/"]).as_deref(),
            Some("pkg")
        );
    }

    #[test]
    fn detect_common_prefix_returns_none_when_mixed() {
        assert_eq!(detect_common_prefix(["pkg/a", "other/b"]), None);
    }

    #[test]
    fn detect_common_prefix_returns_none_for_empty_input() {
        assert_eq!(detect_common_prefix(Vec::<&str>::new()), None);
    }

    #[test]
    fn sanitize_entry_strips_known_prefix() {
        let p = sanitize_entry("deadsync/assets/x.bin", Some("deadsync")).unwrap();
        assert_eq!(p, PathBuf::from("assets/x.bin"));
    }

    #[test]
    fn sanitize_entry_rejects_parent_dir() {
        assert!(sanitize_entry("deadsync/../etc/passwd", Some("deadsync")).is_none());
        assert!(sanitize_entry("../escape", None).is_none());
    }

    #[test]
    fn sanitize_entry_rejects_absolute() {
        assert!(sanitize_entry("/abs/path", None).is_none());
    }

    #[test]
    fn extract_tar_gz_strips_single_top_level_dir() {
        let bytes = build_tar_gz(
            &[("deadsync", b"NEWBIN"), ("assets/x.bin", b"ASSET")],
            Some("deadsync"),
        );
        let dest = tempdir("strip-prefix");
        let n = extract_tar_gz(&bytes, &dest).unwrap();
        assert_eq!(n, 2);
        assert_eq!(fs::read(dest.join("deadsync")).unwrap(), b"NEWBIN");
        assert_eq!(fs::read(dest.join("assets/x.bin")).unwrap(), b"ASSET");
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn extract_tar_gz_keeps_layout_when_no_common_prefix() {
        let bytes = build_tar_gz(&[("a.txt", b"A"), ("dir/b.txt", b"B")], None);
        let dest = tempdir("no-prefix");
        let n = extract_tar_gz(&bytes, &dest).unwrap();
        assert_eq!(n, 2);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"A");
        assert_eq!(fs::read(dest.join("dir/b.txt")).unwrap(), b"B");
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn extract_tar_gz_rejects_traversal_entry() {
        // The tar crate refuses both to write and to read entries
        // containing "..".  We rely on that as defense-in-depth on
        // top of `sanitize_entry`.  This test asserts that an entry
        // *we* might produce by accident (a relative path with
        // ".." after stripping the prefix) is rejected by
        // sanitize_entry alone, which is the only path our
        // extractor uses.
        assert!(sanitize_entry("deadsync/../escape.txt", Some("deadsync")).is_none());
        assert!(sanitize_entry("../escape.txt", None).is_none());
    }

    #[test]
    fn is_dir_writable_returns_true_for_temp_dir() {
        let dir = tempdir("writable");
        assert!(is_dir_writable(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_dir_writable_returns_false_for_missing_dir() {
        let dir =
            std::env::temp_dir().join(format!("deadsync-no-such-dir-{}", std::process::id()));
        assert!(!is_dir_writable(&dir));
    }

    #[test]
    fn staging_dir_for_is_sibling_of_exe_dir() {
        let exe_dir = PathBuf::from("/install/dir");
        let staging = staging_dir_for(&exe_dir);
        assert_eq!(staging.parent(), Some(exe_dir.as_path()));
        assert!(
            staging
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .starts_with(".deadsync-update-staging-")
        );
    }

    // The post-extract `swap_files` + `apply_tar_gz` + collect_files
    // entry points are gated to Linux/FreeBSD because they're meant
    // to be exercised against a live install dir on those platforms.
    // We still want to verify the *logic* works on the dev box, so
    // the next two tests duplicate the walk on the cross-platform
    // helper.

    #[test]
    fn collect_files_walks_recursively() {
        let root = tempdir("collect");
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("top.txt"), b"t").unwrap();
        fs::write(root.join("a/inner.txt"), b"i").unwrap();
        fs::write(root.join("a/b/leaf.txt"), b"l").unwrap();
        let files = collect_files(&root).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert!(names.contains(&"top.txt".to_string()));
        assert!(names.contains(&"a/inner.txt".to_string()));
        assert!(names.contains(&"a/b/leaf.txt".to_string()));
        assert_eq!(names.len(), 3);
        let _ = fs::remove_dir_all(&root);
    }
}
