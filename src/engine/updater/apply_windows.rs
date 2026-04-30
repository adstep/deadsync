//! Windows-side "apply" half of the in-app updater.
//!
//! On Windows the running `.exe` (and any DLLs it has mapped) cannot be
//! deleted, but they *can* be renamed.  We exploit that by:
//!
//! 1. Extracting the freshly-downloaded archive into a sibling staging
//!    directory next to the install root.
//! 2. For every file in the staging tree, moving the corresponding
//!    in-place file to `<name>.old`, then moving the staged file into
//!    the final location.
//! 3. Spawning the new executable with `--cleanup-old <staging_dir>` and
//!    exiting `0`.  PR-14 wires up the spawn + the cleanup command line;
//!    this module provides the orchestration primitives and the
//!    cleanup helper.
//!
//! The module is gated on `cfg(windows)` per the roadmap.  All
//! filesystem operations are kept synchronous and free of `unsafe`.
//!
//! ### Layout assumption
//!
//! The Windows release archive (built by `scripts/package-windows-release.ps1`)
//! has a single top-level directory entry of the form
//! `deadsync-vX.Y.Z-{arch}-windows/`.  [`extract_archive`] strips that
//! prefix when (and only when) every entry shares the same first
//! component.  Archives that lack the prefix extract verbatim.

#![cfg(windows)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use super::UpdaterError;

/// Suffix appended to displaced live files prior to running the new
/// binary.  Picked so that a glob over the install dir can find them
/// during the post-update cleanup pass.
pub const OLD_SUFFIX: &str = ".old";

/// Result of a successful in-place swap.  Returned to the caller so the
/// outer driver (PR-14) can pass `staging_dir` along to the freshly
/// spawned executable via `--cleanup-old`.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// Sibling directory the archive was extracted into.  May contain
    /// leftover empty subdirectories after [`swap_files`] completes;
    /// the post-update cleanup is responsible for removing it.
    pub staging_dir: PathBuf,
    /// Absolute paths of files that were renamed to `<name>.old` and
    /// are pending deletion on next startup.  Useful for diagnostics.
    pub displaced_old_files: Vec<PathBuf>,
    /// Number of files that were swapped in from the archive (i.e.
    /// number of files in the staging tree).
    pub installed_file_count: usize,
}

/* ---------- writability probe ---------- */

/// Best-effort writability probe for `dir`.  Creates and immediately
/// removes a uniquely-named sentinel file; returns `false` if either
/// step fails.  Used to refuse self-update when `exe_dir` lives under
/// `Program Files` or some other location the current user cannot
/// modify, in which case the caller should surface a clear error
/// rather than half-applying the swap.
pub fn is_dir_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let mut probe = dir.to_path_buf();
    probe.push(format!(
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

/* ---------- archive extraction ---------- */

/// Detects a single shared top-level directory across every entry in
/// the archive.  Returns `Some(prefix)` only when **every** non-empty
/// entry path begins with the same first component, otherwise `None`.
///
/// Pure helper extracted so it can be exercised without writing a real
/// archive to disk.
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

/// Validates an extracted entry path: rejects absolute paths and any
/// path containing `..` after the optional prefix has been stripped.
/// Returns the path-relative-to-`dest`, with the prefix removed.
fn sanitize_entry(name: &str, prefix: Option<&str>) -> Option<PathBuf> {
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

/// Extracts `reader` (a seekable zip stream) into `dest`, stripping a
/// single shared top-level directory if present.  Returns the count of
/// regular files written.  Any malformed entry aborts the whole
/// extraction with [`UpdaterError::Io`].
pub fn extract_archive<R: Read + Seek>(reader: R, dest: &Path) -> Result<usize, UpdaterError> {
    fs::create_dir_all(dest).map_err(io_err)?;
    let mut archive = ZipArchive::new(reader).map_err(|e| UpdaterError::Io(e.to_string()))?;
    let mut names = Vec::with_capacity(archive.len());
    for idx in 0..archive.len() {
        let entry = archive
            .by_index_raw(idx)
            .map_err(|e| UpdaterError::Io(e.to_string()))?;
        names.push(entry.name().to_string());
    }
    let prefix = detect_common_prefix(&names);
    let mut written = 0usize;
    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| UpdaterError::Io(e.to_string()))?;
        let raw_name = entry.name().to_string();
        let Some(rel) = sanitize_entry(&raw_name, prefix.as_deref()) else {
            if entry.is_dir() {
                continue;
            }
            return Err(UpdaterError::Io(format!(
                "rejected unsafe archive entry '{raw_name}'"
            )));
        };
        let out_path = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(io_err)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let mut out = File::create(&out_path).map_err(io_err)?;
        io::copy(&mut entry, &mut out).map_err(io_err)?;
        written += 1;
    }
    Ok(written)
}

/* ---------- in-place swap ---------- */

/// Walks `staging_dir` and, for every file, moves the existing file at
/// the corresponding location under `target_dir` to `<name>.old`, then
/// moves the staged file into place.  Missing target files are simply
/// installed (no `.old` placeholder created).
///
/// On any rename error the operation aborts and returns the partial
/// list of `.old` files created, leaving the rest of the install
/// untouched; the caller is expected to surface the error and the
/// next startup's cleanup pass will remove the orphans.
pub fn swap_files(staging_dir: &Path, target_dir: &Path) -> Result<Vec<PathBuf>, UpdaterError> {
    let mut displaced = Vec::new();
    let files = collect_files(staging_dir)?;
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
        if target.exists() {
            let old = old_path_for(&target);
            // Best-effort: if a stale `.old` is still around from a
            // previous failed attempt, drop it so the rename succeeds.
            let _ = fs::remove_file(&old);
            fs::rename(&target, &old).map_err(|e| {
                UpdaterError::Io(format!(
                    "failed to rename '{}' -> '{}': {e}",
                    target.display(),
                    old.display(),
                ))
            })?;
            displaced.push(old);
        }
        fs::rename(staged, &target).map_err(|e| {
            UpdaterError::Io(format!(
                "failed to install '{}' -> '{}': {e}",
                staged.display(),
                target.display(),
            ))
        })?;
    }
    Ok(displaced)
}

/// `<path>.old`, preserving any existing extension (we just append the
/// suffix rather than replacing it; "deadsync.exe" becomes
/// "deadsync.exe.old").
pub fn old_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(OLD_SUFFIX);
    PathBuf::from(s)
}

/* ---------- post-update cleanup ---------- */

/// Recursively removes every `*.old` file under `root` and then deletes
/// `staging_dir` (if non-empty after [`swap_files`] left empty
/// directories behind, those are flushed as well).  Errors are logged
/// implicitly via the returned counts but never propagated: cleanup
/// must never block a successful start-up.
///
/// Returns `(old_files_removed, staging_removed)`.
pub fn cleanup_old_files(root: &Path, staging_dir: Option<&Path>) -> (usize, bool) {
    let mut removed = 0usize;
    let mut stack = vec![root.to_path_buf()];
    let mut visited = BTreeSet::new();
    while let Some(dir) = stack.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in read.flatten() {
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(OLD_SUFFIX))
            {
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    let staging_removed = match staging_dir {
        Some(s) if s.exists() => fs::remove_dir_all(s).is_ok(),
        _ => false,
    };
    (removed, staging_removed)
}

/* ---------- top-level orchestration ---------- */

/// Drives the full apply sequence: writability probe → extract zip into
/// a sibling staging dir → swap files in place.  Returns an
/// [`ApplyOutcome`] describing the staging dir + displaced files.  On
/// failure, partial state may be left on disk; the next startup's
/// cleanup pass is the safety net.
pub fn apply_zip(zip_path: &Path, exe_dir: &Path) -> Result<ApplyOutcome, UpdaterError> {
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
    let file = File::open(zip_path).map_err(io_err)?;
    let installed_file_count = extract_archive(file, &staging_dir)?;
    let displaced_old_files = swap_files(&staging_dir, exe_dir)?;
    Ok(ApplyOutcome {
        staging_dir,
        displaced_old_files,
        installed_file_count,
    })
}

/// `{exe_dir}/.deadsync-update-staging-{pid}-{nanos}/` — kept as a
/// sibling so the eventual `rename` calls stay on the same volume.
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
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn tempdir(stem: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "deadsync-apply-windows-{stem}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_zip(entries: &[(&str, &[u8])], top: Option<&str>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = ZipWriter::new(Cursor::new(&mut buf));
            let opts =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            if let Some(prefix) = top {
                let _ = w.add_directory(format!("{prefix}/"), opts);
            }
            for (name, body) in entries {
                let full = match top {
                    Some(p) => format!("{p}/{name}"),
                    None => (*name).to_string(),
                };
                w.start_file(full, opts).unwrap();
                w.write_all(body).unwrap();
            }
            w.finish().unwrap();
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
    fn detect_common_prefix_handles_backslashes() {
        assert_eq!(
            detect_common_prefix(["pkg\\a", "pkg\\b"]).as_deref(),
            Some("pkg")
        );
    }

    #[test]
    fn sanitize_entry_strips_known_prefix() {
        let p = sanitize_entry("pkg/a/b.txt", Some("pkg")).unwrap();
        assert_eq!(p, PathBuf::from("a/b.txt"));
    }

    #[test]
    fn sanitize_entry_rejects_parent_dir() {
        assert!(sanitize_entry("pkg/../etc/passwd", Some("pkg")).is_none());
        assert!(sanitize_entry("../escape", None).is_none());
    }

    #[test]
    fn sanitize_entry_rejects_absolute() {
        assert!(sanitize_entry("/abs/path", None).is_none());
    }

    #[test]
    fn sanitize_entry_skips_directory_only_entry() {
        assert!(sanitize_entry("pkg/", Some("pkg")).is_none());
    }

    #[test]
    fn old_path_for_appends_suffix() {
        let p = PathBuf::from("/x/y/deadsync.exe");
        assert_eq!(old_path_for(&p), PathBuf::from("/x/y/deadsync.exe.old"));
    }

    #[test]
    fn extract_archive_strips_single_top_level_dir() {
        let zip = build_zip(
            &[("deadsync.exe", b"NEWBIN"), ("assets/x.bin", b"ASSET")],
            Some("deadsync-v1.0.0-x86_64-windows"),
        );
        let dest = tempdir("strip-prefix");
        let n = extract_archive(Cursor::new(zip), &dest).unwrap();
        assert_eq!(n, 2);
        assert_eq!(fs::read(dest.join("deadsync.exe")).unwrap(), b"NEWBIN");
        assert_eq!(fs::read(dest.join("assets/x.bin")).unwrap(), b"ASSET");
        assert!(!dest.join("deadsync-v1.0.0-x86_64-windows").exists());
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn extract_archive_keeps_layout_when_no_common_prefix() {
        let zip = build_zip(
            &[("a.txt", b"A"), ("dir/b.txt", b"B")],
            None,
        );
        let dest = tempdir("no-prefix");
        let n = extract_archive(Cursor::new(zip), &dest).unwrap();
        assert_eq!(n, 2);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"A");
        assert_eq!(fs::read(dest.join("dir/b.txt")).unwrap(), b"B");
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn extract_archive_rejects_traversal() {
        // The zip crate's writer normalizes paths, so we hand-craft a
        // tiny zip whose central-directory entry contains a literal
        // "../escape.txt" name.  Built once, hex-encoded for clarity.
        let zip: Vec<u8> = vec![
            0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x0e, 0x00, 0x00, 0x00, 0x2e, 0x2e, 0x2f, 0x65, 0x73, 0x63, 0x61, 0x70, 0x65, 0x2e,
            0x74, 0x78, 0x74, 0x00, 0x50, 0x4b, 0x01, 0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x2e, 0x2e, 0x2f, 0x65, 0x73, 0x63, 0x61, 0x70, 0x65, 0x2e, 0x74, 0x78, 0x74, 0x50,
            0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x3c, 0x00, 0x00,
            0x00, 0x3b, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let dest = tempdir("traversal");
        // We accept either Err (preferred) or that the zip crate
        // declines to parse the entry; what we MUST NOT see is a file
        // named `escape.txt` written outside `dest`.
        let _ = extract_archive(Cursor::new(&zip), &dest);
        let escape = dest.parent().unwrap().join("escape.txt");
        assert!(!escape.exists(), "traversal succeeded: {}", escape.display());
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn swap_files_renames_existing_to_old_and_installs_new() {
        let target = tempdir("swap-target");
        let staging = tempdir("swap-staging");

        fs::write(target.join("deadsync.exe"), b"OLDBIN").unwrap();
        fs::create_dir_all(target.join("assets")).unwrap();
        fs::write(target.join("assets/keep.bin"), b"KEEP").unwrap();

        fs::write(staging.join("deadsync.exe"), b"NEWBIN").unwrap();
        fs::create_dir_all(staging.join("assets")).unwrap();
        fs::write(staging.join("assets/new.bin"), b"NEW").unwrap();

        let displaced = swap_files(&staging, &target).unwrap();
        assert_eq!(displaced.len(), 1);
        assert_eq!(
            displaced[0].file_name().and_then(|n| n.to_str()),
            Some("deadsync.exe.old")
        );

        assert_eq!(fs::read(target.join("deadsync.exe")).unwrap(), b"NEWBIN");
        assert_eq!(fs::read(target.join("deadsync.exe.old")).unwrap(), b"OLDBIN");
        assert_eq!(fs::read(target.join("assets/new.bin")).unwrap(), b"NEW");
        assert_eq!(fs::read(target.join("assets/keep.bin")).unwrap(), b"KEEP");

        let _ = fs::remove_dir_all(&target);
        let _ = fs::remove_dir_all(&staging);
    }

    #[test]
    fn swap_files_overwrites_stale_old_file() {
        let target = tempdir("swap-stale");
        let staging = tempdir("swap-stale-stage");

        fs::write(target.join("a.txt"), b"V1").unwrap();
        fs::write(target.join("a.txt.old"), b"STALE").unwrap();
        fs::write(staging.join("a.txt"), b"V2").unwrap();

        let displaced = swap_files(&staging, &target).unwrap();
        assert_eq!(displaced.len(), 1);
        assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"V2");
        assert_eq!(fs::read(target.join("a.txt.old")).unwrap(), b"V1");

        let _ = fs::remove_dir_all(&target);
        let _ = fs::remove_dir_all(&staging);
    }

    #[test]
    fn cleanup_old_files_removes_old_suffix_recursively() {
        let root = tempdir("cleanup");
        let staging = root.join(".staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(root.join("a.txt"), b"keep").unwrap();
        fs::write(root.join("a.txt.old"), b"drop").unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("assets/b.bin.old"), b"drop").unwrap();

        let (n, staging_gone) = cleanup_old_files(&root, Some(&staging));
        assert_eq!(n, 2);
        assert!(staging_gone);
        assert!(!root.join("a.txt.old").exists());
        assert!(!root.join("assets/b.bin.old").exists());
        assert!(root.join("a.txt").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cleanup_old_files_is_safe_when_nothing_to_remove() {
        let root = tempdir("cleanup-empty");
        let (n, staging_gone) = cleanup_old_files(&root, None);
        assert_eq!(n, 0);
        assert!(!staging_gone);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn is_dir_writable_returns_true_for_temp_dir() {
        let dir = tempdir("writable");
        assert!(is_dir_writable(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_dir_writable_returns_false_for_missing_dir() {
        let dir = std::env::temp_dir().join(format!(
            "deadsync-no-such-dir-{}",
            std::process::id()
        ));
        assert!(!is_dir_writable(&dir));
    }

    #[test]
    fn apply_zip_writes_files_into_exe_dir_and_renames_existing() {
        let exe_dir = tempdir("apply-zip");
        fs::write(exe_dir.join("deadsync.exe"), b"OLD").unwrap();

        let zip = build_zip(
            &[("deadsync.exe", b"NEW"), ("assets/x.bin", b"X")],
            Some("deadsync-v1.0.0-x86_64-windows"),
        );
        let zip_path = exe_dir.join("update.zip");
        fs::write(&zip_path, &zip).unwrap();

        let outcome = apply_zip(&zip_path, &exe_dir).unwrap();
        assert_eq!(outcome.installed_file_count, 2);
        assert_eq!(outcome.displaced_old_files.len(), 1);
        assert_eq!(fs::read(exe_dir.join("deadsync.exe")).unwrap(), b"NEW");
        assert_eq!(fs::read(exe_dir.join("deadsync.exe.old")).unwrap(), b"OLD");
        assert_eq!(fs::read(exe_dir.join("assets/x.bin")).unwrap(), b"X");

        let _ = fs::remove_dir_all(&exe_dir);
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
}
