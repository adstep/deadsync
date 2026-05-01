//! Windows-side "apply" half of the in-app updater.
//!
//! On Windows the running `.exe` (and any DLLs it has mapped) cannot
//! be deleted, but they *can* be renamed.  We exploit that as follows:
//!
//! 1. Extract the freshly-downloaded archive into a sibling staging
//!    directory next to the install root.
//! 2. Build a plan (one [`apply_journal::Op`] per staged file) and
//!    write a journal recording the planned mutations
//!    ([`apply_journal::JournalState::Applying`]).
//! 3. For each op, rename the existing target to its per-apply
//!    backup name (`<target>.deadsync-bak-<token>`), then rename the
//!    staged file into the target's place.
//! 4. On any error mid-apply, walk the executed ops in reverse and
//!    restore them; on success rewrite the journal as
//!    [`apply_journal::JournalState::Applied`].
//! 5. Spawn the new executable with `--restart` and exit.  The next
//!    startup runs [`apply_journal::recover`], which deletes the
//!    backups and the staging directory.
//!
//! The crash-recovery story lives in [`crate::engine::updater::apply_journal`]:
//! a crash with the journal still in `Applying` rolls back to a
//! bit-identical pre-apply tree on the next launch.
//!
//! ### Layout assumption
//!
//! The Windows release archive (built by `scripts/package-windows-release.ps1`)
//! has a single top-level directory entry of the form
//! `deadsync-vX.Y.Z-{arch}-windows/`.  [`extract_archive`] strips that
//! prefix when (and only when) every entry shares the same first
//! component.  Archives that lack the prefix extract verbatim.

#![cfg(windows)]

use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use super::apply_journal::{self, Journal, JournalState, Op};
use super::UpdaterError;

/// Result of a successful apply.  Returned for diagnostics + tests;
/// the caller doesn't need to thread anything back into the relaunch
/// command line because the journal at the install root is now the
/// source of truth for cleanup.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// Sibling directory the archive was extracted into.  May contain
    /// leftover empty subdirectories after the swap completes; the
    /// post-update cleanup pass removes the directory entirely.
    pub staging_dir: PathBuf,
    /// Number of files that were swapped in from the archive.
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

/* ---------- planning + journal-driven apply ---------- */

/// Walks `staging_dir` and produces an `Op` for every regular file,
/// pairing it with its target path under `target_dir` and a backup
/// path derived from the journal's per-apply token.  Sorted to make
/// iteration deterministic across platforms.
fn plan_ops(
    journal: &Journal,
    staging_dir: &Path,
    target_dir: &Path,
) -> Result<Vec<Op>, UpdaterError> {
    let files = collect_files(staging_dir)?;
    let mut ops = Vec::with_capacity(files.len());
    for staged in files {
        let rel = staged.strip_prefix(staging_dir).map_err(|_| {
            UpdaterError::Io(format!(
                "staged path '{}' escapes staging dir '{}'",
                staged.display(),
                staging_dir.display(),
            ))
        })?;
        let target = target_dir.join(rel);
        let target_existed = target.exists();
        let backup = journal.backup_path_for(&target);
        ops.push(Op {
            staged,
            target,
            backup,
            target_existed,
        });
    }
    Ok(ops)
}

/// Executes the journal's plan: for each op, ensure the target's
/// parent directory exists, optionally rename the live file aside
/// (recording it in `executed`), then rename the staged file into
/// place.  Any error walks `executed` in reverse to restore the
/// pre-apply state before returning.
fn execute_with_rollback(journal: &Journal) -> Result<(), UpdaterError> {
    let mut executed: Vec<&Op> = Vec::with_capacity(journal.ops.len());
    for op in &journal.ops {
        if let Some(parent) = op.target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                rollback(&executed);
                return Err(io_err(e));
            }
        }
        if op.target_existed {
            // A stale backup from a previous half-completed attempt
            // could only exist if the per-apply token collided, which
            // is statistically impossible for 128-bit tokens.  Belt
            // and braces: surface it as an error rather than silently
            // overwriting.
            if op.backup.exists() {
                rollback(&executed);
                return Err(UpdaterError::Io(format!(
                    "backup path '{}' already exists; refusing to overwrite",
                    op.backup.display(),
                )));
            }
            if let Err(e) = fs::rename(&op.target, &op.backup) {
                rollback(&executed);
                return Err(UpdaterError::Io(format!(
                    "failed to rename '{}' -> '{}': {e}",
                    op.target.display(),
                    op.backup.display(),
                )));
            }
        }
        if let Err(e) = fs::rename(&op.staged, &op.target) {
            // Roll back this op's own backup before recursing.
            if op.target_existed {
                let _ = fs::rename(&op.backup, &op.target);
            }
            rollback(&executed);
            return Err(UpdaterError::Io(format!(
                "failed to install '{}' -> '{}': {e}",
                op.staged.display(),
                op.target.display(),
            )));
        }
        executed.push(op);
    }
    Ok(())
}

/// Best-effort reversal of every successfully-executed op: restore the
/// new file at `target` back to its original `staged` location (so
/// the staging dir cleanup later sweeps it), then move the backup
/// back over the target.  Failures are intentionally ignored — the
/// next startup's [`apply_journal::recover`] is the safety net.
fn rollback(executed: &[&Op]) {
    for op in executed.iter().rev() {
        let _ = fs::rename(&op.target, &op.staged);
        if op.target_existed {
            let _ = fs::rename(&op.backup, &op.target);
        }
    }
}

/* ---------- top-level orchestration ---------- */

/// Drives the full apply sequence: writability probe → extract zip
/// into a sibling staging dir → write Applying journal → execute the
/// per-op renames with rollback on error → write Applied journal.
/// Returns an [`ApplyOutcome`] describing the staging dir.  On
/// failure, any partial mutations are rolled back and the journal is
/// removed if the rollback completed cleanly.
pub fn apply_zip(zip_path: &Path, exe_dir: &Path) -> Result<ApplyOutcome, UpdaterError> {
    if !is_dir_writable(exe_dir) {
        return Err(UpdaterError::Io(format!(
            "install directory is not writable: {}",
            exe_dir.display(),
        )));
    }
    let mut journal = Journal::new(exe_dir);
    if journal.staging_dir.exists() {
        let _ = fs::remove_dir_all(&journal.staging_dir);
    }
    let file = File::open(zip_path).map_err(io_err)?;
    let installed_file_count = extract_archive(file, &journal.staging_dir)?;
    journal.ops = plan_ops(&journal, &journal.staging_dir, exe_dir)?;
    journal.write_atomic(exe_dir)?;
    if let Err(e) = execute_with_rollback(&journal) {
        // Rollback already restored the install tree; drop the
        // journal so a future startup doesn't try to recover from a
        // state that no longer matches the filesystem.
        let _ = fs::remove_file(apply_journal::journal_path(exe_dir));
        let _ = fs::remove_dir_all(&journal.staging_dir);
        return Err(e);
    }
    journal.state = JournalState::Applied;
    journal.write_atomic(exe_dir)?;
    Ok(ApplyOutcome {
        staging_dir: journal.staging_dir,
        installed_file_count,
    })
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
    fn plan_ops_pairs_staged_files_with_targets() {
        let staging = tempdir("plan-staging");
        let target = tempdir("plan-target");
        fs::write(staging.join("a.txt"), b"A").unwrap();
        fs::create_dir_all(staging.join("nested")).unwrap();
        fs::write(staging.join("nested/b.bin"), b"B").unwrap();
        fs::write(target.join("a.txt"), b"OLD").unwrap();

        let journal = Journal::new(&target);
        let mut ops = plan_ops(&journal, &staging, &target).unwrap();
        ops.sort_by(|x, y| x.target.cmp(&y.target));
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].target, target.join("a.txt"));
        assert!(ops[0].target_existed);
        assert_eq!(ops[0].backup, journal.backup_path_for(&ops[0].target));
        assert_eq!(ops[1].target, target.join("nested").join("b.bin"));
        assert!(!ops[1].target_existed);

        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_dir_all(&target);
    }

    #[test]
    fn execute_with_rollback_installs_each_op() {
        let exe_dir = tempdir("exec-ok");
        let mut journal = Journal::new(&exe_dir);
        fs::create_dir_all(&journal.staging_dir).unwrap();
        fs::write(exe_dir.join("a.txt"), b"OLD").unwrap();
        let staged_a = journal.staging_dir.join("a.txt");
        let staged_b = journal.staging_dir.join("b.txt");
        fs::write(&staged_a, b"NEWA").unwrap();
        fs::write(&staged_b, b"NEWB").unwrap();
        journal.ops = vec![
            Op {
                staged: staged_a,
                target: exe_dir.join("a.txt"),
                backup: journal.backup_path_for(&exe_dir.join("a.txt")),
                target_existed: true,
            },
            Op {
                staged: staged_b,
                target: exe_dir.join("b.txt"),
                backup: journal.backup_path_for(&exe_dir.join("b.txt")),
                target_existed: false,
            },
        ];

        execute_with_rollback(&journal).unwrap();
        assert_eq!(fs::read(exe_dir.join("a.txt")).unwrap(), b"NEWA");
        assert_eq!(fs::read(exe_dir.join("b.txt")).unwrap(), b"NEWB");
        assert!(journal.ops[0].backup.exists());
        assert_eq!(fs::read(&journal.ops[0].backup).unwrap(), b"OLD");

        let _ = fs::remove_dir_all(&exe_dir);
    }

    #[test]
    fn execute_with_rollback_restores_on_mid_apply_failure() {
        let exe_dir = tempdir("exec-fail");
        let mut journal = Journal::new(&exe_dir);
        fs::create_dir_all(&journal.staging_dir).unwrap();
        fs::write(exe_dir.join("a.txt"), b"OLDA").unwrap();
        fs::write(exe_dir.join("b.txt"), b"OLDB").unwrap();
        let staged_a = journal.staging_dir.join("a.txt");
        fs::write(&staged_a, b"NEWA").unwrap();
        // op[0] is well-formed; op[1] points at a non-existent staged
        // file, guaranteeing the rename will fail and trigger rollback.
        let bogus_staged = journal.staging_dir.join("does-not-exist");
        journal.ops = vec![
            Op {
                staged: staged_a,
                target: exe_dir.join("a.txt"),
                backup: journal.backup_path_for(&exe_dir.join("a.txt")),
                target_existed: true,
            },
            Op {
                staged: bogus_staged,
                target: exe_dir.join("b.txt"),
                backup: journal.backup_path_for(&exe_dir.join("b.txt")),
                target_existed: true,
            },
        ];

        let err = execute_with_rollback(&journal).unwrap_err();
        match err {
            UpdaterError::Io(_) => {}
            other => panic!("expected Io, got {other:?}"),
        }
        assert_eq!(
            fs::read(exe_dir.join("a.txt")).unwrap(),
            b"OLDA",
            "op[0] target should be restored to its pre-apply contents",
        );
        assert_eq!(fs::read(exe_dir.join("b.txt")).unwrap(), b"OLDB");
        assert!(!journal.ops[0].backup.exists());
        assert!(!journal.ops[1].backup.exists());

        let _ = fs::remove_dir_all(&exe_dir);
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
    fn apply_zip_installs_files_and_writes_applied_journal() {
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
        assert_eq!(fs::read(exe_dir.join("deadsync.exe")).unwrap(), b"NEW");
        assert_eq!(fs::read(exe_dir.join("assets/x.bin")).unwrap(), b"X");

        let journal = Journal::load(&exe_dir).unwrap().expect("journal present");
        assert_eq!(journal.state, JournalState::Applied);
        assert_eq!(journal.staging_dir, outcome.staging_dir);
        // The displaced old binary is now under the journal-named
        // backup, not the legacy `.old` suffix.
        let backup = journal.backup_path_for(&exe_dir.join("deadsync.exe"));
        assert_eq!(fs::read(&backup).unwrap(), b"OLD");

        let _ = fs::remove_dir_all(&exe_dir);
    }

    #[test]
    fn apply_zip_then_recover_cleans_backups_and_staging() {
        let exe_dir = tempdir("apply-zip-recover");
        fs::write(exe_dir.join("deadsync.exe"), b"OLD").unwrap();
        let zip = build_zip(
            &[("deadsync.exe", b"NEW")],
            Some("deadsync-v1.0.0-x86_64-windows"),
        );
        let zip_path = exe_dir.join("update.zip");
        fs::write(&zip_path, &zip).unwrap();

        let outcome = apply_zip(&zip_path, &exe_dir).unwrap();
        let report = apply_journal::recover(&exe_dir);
        assert!(report.journal_removed);
        assert!(report.staging_removed);
        assert_eq!(report.backups_removed, 1);
        assert!(!outcome.staging_dir.exists());
        assert!(!apply_journal::journal_path(&exe_dir).exists());
        assert_eq!(fs::read(exe_dir.join("deadsync.exe")).unwrap(), b"NEW");

        let _ = fs::remove_dir_all(&exe_dir);
    }
}
