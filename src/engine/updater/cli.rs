//! Tiny argv parser for the in-app updater driver.
//!
//! We intentionally avoid pulling in `clap` for these three flags so
//! main.rs stays free of CLI plumbing.  The recognised set is:
//!
//! * `--cleanup-old <staging-dir>` — runs the post-swap cleanup pass
//!   (delete `*.old` files under exe_dir, remove the staging dir),
//!   then continues into normal startup.  PR-11 describes the
//!   Windows side; the Unix side is a no-op cleanup but the flag is
//!   accepted so the same parent process can launch either platform.
//!
//! * `--restart` — informational marker logged at startup; the menu
//!   uses it to display "Updated to vX.Y.Z" once.
//!
//! * `--no-update-check` — skips the startup network check.
//!
//! Unknown flags are passed through unchanged; we don't want to
//! conflict with any future windowing-system or test runner argv.
//!
//! All parsing is pure and table-tested.

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdaterCli {
    /// `Some(staging_dir)` if `--cleanup-old <path>` was passed.
    pub cleanup_old: Option<PathBuf>,
    /// `true` if `--restart` was passed (we just self-updated).
    pub restart: bool,
    /// `true` if `--no-update-check` was passed (skip startup check).
    pub no_update_check: bool,
    /// Argv with our recognised flags removed; preserved for any
    /// downstream consumer (currently none, but keeps us future-proof).
    pub remaining: Vec<String>,
}

impl UpdaterCli {
    /// Parse the supplied argv, ignoring `argv[0]`.  Unknown flags
    /// pass through into [`Self::remaining`] unchanged.
    pub fn parse<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut iter = argv.into_iter().map(Into::into).peekable();
        // Skip program name if present.
        let _ = iter.next();
        let mut out = UpdaterCli::default();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--cleanup-old" => {
                    if let Some(path) = iter.next() {
                        out.cleanup_old = Some(PathBuf::from(path));
                    }
                }
                a if a.starts_with("--cleanup-old=") => {
                    let value = &a["--cleanup-old=".len()..];
                    if !value.is_empty() {
                        out.cleanup_old = Some(PathBuf::from(value));
                    }
                }
                "--restart" => out.restart = true,
                "--no-update-check" => out.no_update_check = true,
                _ => out.remaining.push(arg),
            }
        }
        out
    }

    /// Convenience: parse `std::env::args()`.  Lifted out so tests
    /// can hit the table-driven [`parse`](Self::parse) without
    /// touching process state.
    pub fn from_env() -> Self {
        Self::parse(std::env::args())
    }
}

/// Runs the post-update cleanup pass.  On Windows this is the
/// `*.old` reaper from [`crate::engine::updater::apply_windows`]; on
/// Unix the staged files were already moved into place, so the only
/// thing left is to remove the staging dir if it still exists.
///
/// Errors are intentionally swallowed (the caller is post-startup
/// best-effort), but the (removed_count, staging_removed) tuple is
/// returned for diagnostics + tests.
pub fn run_cleanup(exe_dir: &std::path::Path, staging_dir: &std::path::Path) -> (usize, bool) {
    // The `staging_dir` argument is retained for back-compat with old
    // relaunch command lines but is no longer consulted: the journal
    // file at the install root is now the source of truth for both
    // the staging dir path and the per-op backup names.
    let _ = staging_dir;
    #[cfg(feature = "self-update")]
    {
        let report = crate::engine::updater::apply_journal::recover(exe_dir);
        let staging_removed = report.staging_removed;
        let removed_count =
            report.backups_removed + report.backups_restored + report.installed_removed;
        return (removed_count, staging_removed);
    }
    #[cfg(not(feature = "self-update"))]
    {
        let _ = exe_dir;
        (0, false)
    }
}

/// Try to apply the downloaded archive at [`Ready`] and re-launch.
///
/// Returns `Ok(true)` when the calling process should exit `0`
/// (because the new process has been spawned), `Ok(false)` when the
/// caller should continue normal startup (no Ready snapshot found),
/// and `Err` on apply failure.  The Ready snapshot is consumed by
/// transitioning to [`ActionPhase::Idle`] regardless of outcome so
/// the menu UI doesn't re-prompt indefinitely.
///
/// The platform split lives behind `cfg`: PR-11 covers Windows,
/// PR-12 covers Linux/FreeBSD, and macOS falls through to the unix
/// path on a best-effort basis (PR-13 was deferred — for now we
/// surface a clear "platform not supported" error there).
#[allow(clippy::result_large_err)]
pub fn apply_pending_and_relaunch() -> Result<bool, super::UpdaterError> {
    use super::action::{ActionPhase, current, dismiss};
    let phase = current();
    let (archive_path, _info) = match phase {
        ActionPhase::Ready { path, info } => (path, info),
        _ => return Ok(false),
    };
    // Always clear the snapshot so the UI doesn't re-prompt if apply
    // bails out below.
    dismiss();
    apply_archive_and_relaunch(&archive_path)?;
    Ok(true)
}

/// Lower-level apply: caller has already chosen the archive (e.g. via
/// the [`super::action::ActionPhase::Ready`] snapshot) and is responsible
/// for any phase bookkeeping.  Performs the platform-specific extract +
/// swap, then spawns the new process with the appropriate cleanup
/// arguments.  Caller should `std::process::exit(0)` on success to
/// release any binary locks (Windows in particular).
#[allow(clippy::result_large_err)]
pub fn apply_archive_and_relaunch(
    archive_path: &std::path::Path,
) -> Result<(), super::UpdaterError> {
    let exe_dir = exe_dir()?;
    apply_for_host(archive_path, &exe_dir)?;
    relaunch_self(&exe_dir)?;
    Ok(())
}

#[cfg(all(windows, feature = "self-update"))]
fn apply_for_host(
    archive_path: &std::path::Path,
    exe_dir: &std::path::Path,
) -> Result<(), super::UpdaterError> {
    let _ = super::apply_windows::apply_zip(archive_path, exe_dir)?;
    Ok(())
}

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "self-update"))]
fn apply_for_host(
    archive_path: &std::path::Path,
    exe_dir: &std::path::Path,
) -> Result<(), super::UpdaterError> {
    let _ = super::apply_unix::apply_tar_gz(archive_path, exe_dir)?;
    Ok(())
}

#[cfg(not(any(
    all(windows, feature = "self-update"),
    all(any(target_os = "linux", target_os = "freebsd"), feature = "self-update"),
)))]
fn apply_for_host(
    _archive_path: &std::path::Path,
    _exe_dir: &std::path::Path,
) -> Result<(), super::UpdaterError> {
    Err(super::UpdaterError::Io(
        "self-update apply is disabled in this build".to_string(),
    ))
}

fn exe_dir() -> Result<PathBuf, super::UpdaterError> {
    let exe = std::env::current_exe()
        .map_err(|e| super::UpdaterError::Io(format!("current_exe: {e}")))?;
    exe.parent()
        .map(PathBuf::from)
        .ok_or_else(|| super::UpdaterError::Io("exe has no parent dir".to_string()))
}

#[cfg(all(windows, feature = "self-update"))]
fn relaunch_self(exe_dir: &std::path::Path) -> Result<(), super::UpdaterError> {
    use std::process::Command;
    let _ = exe_dir;
    let exe = std::env::current_exe()
        .map_err(|e| super::UpdaterError::Io(format!("current_exe: {e}")))?;
    // No `--cleanup-old <path>` is needed anymore: the new process
    // discovers the apply journal at its install root and runs
    // recovery unconditionally on startup.
    Command::new(&exe)
        .arg("--restart")
        .spawn()
        .map_err(|e| super::UpdaterError::Io(format!("spawn new exe: {e}")))?;
    Ok(())
}

#[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "self-update"))]
fn relaunch_self(_exe_dir: &std::path::Path) -> Result<(), super::UpdaterError> {
    use std::process::Command;
    let exe = std::env::current_exe()
        .map_err(|e| super::UpdaterError::Io(format!("current_exe: {e}")))?;
    Command::new(&exe)
        .arg("--restart")
        .spawn()
        .map_err(|e| super::UpdaterError::Io(format!("spawn new exe: {e}")))?;
    Ok(())
}

#[cfg(not(any(
    all(windows, feature = "self-update"),
    all(any(target_os = "linux", target_os = "freebsd"), feature = "self-update"),
)))]
fn relaunch_self(_exe_dir: &std::path::Path) -> Result<(), super::UpdaterError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_args_returns_default() {
        let cli = UpdaterCli::parse::<Vec<&str>, _>(vec!["deadsync"]);
        assert!(cli.cleanup_old.is_none());
        assert!(!cli.restart);
        assert!(!cli.no_update_check);
        assert!(cli.remaining.is_empty());
    }

    #[test]
    fn parse_no_update_check_sets_flag() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--no-update-check"]);
        assert!(cli.no_update_check);
        assert!(!cli.restart);
        assert!(cli.cleanup_old.is_none());
    }

    #[test]
    fn parse_restart_sets_flag() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--restart"]);
        assert!(cli.restart);
        assert!(!cli.no_update_check);
    }

    #[test]
    fn parse_cleanup_old_takes_path_argument() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--cleanup-old", "C:\\stage"]);
        assert_eq!(cli.cleanup_old.as_deref(), Some(std::path::Path::new("C:\\stage")));
    }

    #[test]
    fn parse_cleanup_old_supports_equals_form() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--cleanup-old=/tmp/stage"]);
        assert_eq!(cli.cleanup_old.as_deref(), Some(std::path::Path::new("/tmp/stage")));
    }

    #[test]
    fn parse_cleanup_old_without_value_is_ignored() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--cleanup-old"]);
        assert!(cli.cleanup_old.is_none());
    }

    #[test]
    fn parse_passes_unknown_flags_through() {
        let cli = UpdaterCli::parse(vec!["deadsync", "--unknown", "value", "--restart"]);
        assert!(cli.restart);
        assert_eq!(cli.remaining, vec!["--unknown", "value"]);
    }

    #[test]
    fn parse_combines_all_flags() {
        let cli = UpdaterCli::parse(vec![
            "deadsync",
            "--no-update-check",
            "--restart",
            "--cleanup-old",
            "/x/y",
        ]);
        assert!(cli.no_update_check);
        assert!(cli.restart);
        assert_eq!(cli.cleanup_old.as_deref(), Some(std::path::Path::new("/x/y")));
        assert!(cli.remaining.is_empty());
    }

    #[test]
    fn run_cleanup_handles_missing_staging_silently() {
        let stem = format!(
            "deadsync-cli-cleanup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(&stem);
        std::fs::create_dir_all(&dir).unwrap();
        let staging = dir.join("does-not-exist");
        let (_n, staging_gone) = run_cleanup(&dir, &staging);
        assert!(!staging_gone);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
