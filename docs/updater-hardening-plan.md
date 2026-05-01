# Updater Hardening Plan

Tracks remaining work to harden the in-app updater (`src/engine/updater/`)
before it can be safely shipped to a wider audience or to additional
distribution channels.

Findings come from a holistic review of the current updater on branch
`adstep/main/updater-pr01-version-module`. Items are grouped by severity
and ordered roughly in the sequence they should be addressed.

## Goal

Make the updater safe to enable by default for the **Windows portable**
distribution, and put guardrails / build-time controls in place so that
future distributions (installer, Microsoft Store, macOS, Linux packages,
Steam, Flatpak, Snap, etc.) can opt out without code changes.

## Out of scope (for this plan)

- Implementing macOS apply support (requires Sparkle or equivalent —
  separate plan).
- Linux package-format-specific install detection (deb/rpm/Flatpak/Snap)
  — separate plan once we ship a Linux build.
- Fancy UI for download cancellation / progress beyond what already
  exists.

## Severity legend

- 🔴 **Critical** — security / data-loss / supply-chain. Must be fixed
  before recommending the updater to non-developer users.
- 🟠 **Major** — correctness, robustness, or design choices that will be
  expensive to undo later.
- 🟡 **Minor** — polish, comments, small bugs.
- 💡 **Future / platform** — required before adding a specific
  distribution channel.

## Status

Tracks progress against the items below. Update this table as work
lands so the rest of the document can stay descriptive.

| ID  | Title                                                         | Severity | Status         | Landed in / notes                                                                 |
| --- | ------------------------------------------------------------- | -------- | -------------- | --------------------------------------------------------------------------------- |
| C1  | Add independent signature verification for downloaded artifacts | 🔴       | ⏳ Not started |                                                                                   |
| C2  | Gate `DEADSYNC_UPDATER_FAKE_DOWNLOAD` to dev/test builds      | 🔴       | ⏳ Not started |                                                                                   |
| C3  | Stop deleting arbitrary `*.old` files under the install root  | 🔴       | ✅ Done        | Backups now use per-apply token suffix; recursive `*.old` cleanup pass removed (`apply_journal`). |
| M1  | Make `apply` transactional / rollback-capable                 | 🟠       | ✅ Done        | Durable JSON journal + per-op backup-then-install + crash recovery on next launch (`apply_journal`). |
| M2  | Re-verify the staged archive immediately before extraction    | 🟠       | ✅ Done        | `Ready` snapshot now carries the expected SHA-256; `apply_archive_and_relaunch` re-hashes the file via `download::sha256_of_file` and rejects mismatches as `ChecksumMismatch`, dropping the staged archive. |
| M3  | Persist enough release metadata to reconstruct `Available` after a 304 | 🟠 | ✅ Done | `UpdaterCache.cached_release` now holds tag/url/body/assets; `load_persisted_cache` reclassifies it on startup so the banner survives a 304 / offline launch and degrades to UpToDate once installed. |
| M4  | Use `last_checked_at` to throttle startup checks              | 🟠       | ❎ Won't fix   | Reviewed and closed: ETag-conditional polls keep startup checks ~free (304 with empty body), and the existing `RateLimited` path handles the corner case gracefully. `last_checked_at` was dropped from `UpdaterCache`; misleading "Daily mode" comments were removed. Manual checks were never throttled in the first place. |
| M5  | Wire up `UpdateChannel::Prerelease` or remove the choice      | 🟠       | ✅ Done        | Removed: `UpdateChannel` enum, `Config::update_channel`, the `update_update_channel` setter, the `UpdateChannel` ini key (load/save/defaults), and the two related tests. The updater always polls `/releases/latest`. |
| M6  | Gate `DEADSYNC_UPDATER_RELEASE_URL` to dev/test builds        | 🟠       | ⏳ Not started |                                                                                   |
| M7  | Thread `ApplyOutcome.staging_dir` into `relaunch_self`        | 🟠       | ✅ Done        | Resolved by removal: relaunch no longer passes `--cleanup-old <staging>`; journal at install root is the source of truth. |
| M8  | Don't offer in-app install on platforms where apply is unsupported | 🟠 | ✅ Done        | `apply_supported_for_host()` mirrors the cli cfg gate; `classify_check_result` short-circuits Available → `AvailableNoInstall { info }` on macOS; overlay shows release tag + `html_url` with Dismiss only — no Download button. |
| M9  | Make in-app install opt-out for managed distributions         | 🟠       | ✅ Done        | New `[Options] UpdaterInstallEnabled` (default `1`); when `0`, `classify_check_result` routes Available → `AvailableNoInstall` and `request_download` refuses, so banner / Check For Updates still surface releases but the Download button never appears. Packagers (Steam / distro / MSIX) ship the ini with `0`. As of C6 this config is the **sole** install-disable knob — the `self-update` cargo feature has been removed. |
| M10 | Add an inter-process updater lock                             | 🟠       | ✅ Done        | `engine::single_instance` (Windows named mutex / Unix `flock`); second instance exits with code 1; `--restart` retries 3 s. |
| M11 | Reconcile `REQUEST_TIMEOUT` with the shared HTTP agent        | 🟠       | ✅ Done        | Removed unused constant; updater now uses dedicated `check_agent` (10 s global) and `download_agent` (no global, 15 s connect / 10 s resolve) so multi-MB archives aren't capped at the score-submit timeout. |
| N1  | ETag bookkeeping                                              | 🟡       | ✅ Done        | `apply_fresh_to_cache` lifted out of `run_check_once` and now overwrites `etag` unconditionally so a Fresh-without-ETag drops the previous value instead of carrying it into the next `If-None-Match`. Channel-scoping deferred: M5 removed `UpdateChannel`, so there's only one release URL today. |
| N2  | Verify GitHub's API `digest` field too                        | 🟡       | ✅ Done        | New `cross_check_api_digest` helper compares `assets[].digest` (e.g. `sha256:…`) against the parsed `.sha256` sidecar before downloading; mismatch fails closed via `ChecksumMismatch`, unsupported algorithms log-and-skip, missing field is no-op. Wired into `action::run_download`. |
| N3  | Add cancellation during long checks/downloads                 | 🟡       | ✅ Done        | New `action::request_cancel()` + `cancel_requested()` flag, polled by check / sidecar / download / fake-download workers; `download_to_file` takes a `should_cancel` callback that fires before/between chunks and returns `UpdaterError::Cancelled` (partial file is removed). Overlay binds Back during Checking/Downloading to cancel; Applying remains uncancellable. |
| N4  | Stage downloads to `*.part`, then atomically rename           | 🟡       | ✅ Done        | `download_to_file` now writes to `<dest>.part`, fsyncs after the final flush, and renames onto `dest` only after sha256 verifies. Crash / cancel / mismatch leaves no file at the canonical name; any pre-existing `dest` is preserved. Stale `.part` from a previous run is removed before staging. |
| N5  | Audit unused i18n keys                                        | 🟡       | ✅ Done        | Dropped `BodyAvailable`, `BodyDownloading`, `BodyReady`, `BodyApplyHint` from `en.ini`/`sv.ini`/`pseudo.ini`; the overlay only uses `BodyReadyShort` (and the M8-era `BodyManualDownload`). pseudo.ini regenerated via `cargo run --bin generate_pseudo`. |
| N6  | Refresh stale comments                                        | 🟡       | ✅ Done        | Fixed `state.rs` module doc (was `Settings.ini`, actually `deadsync.ini`); rewrote `last_seen_tag` doc to drop the obsolete "M5 (channel wiring)" reference (M5 removed channels); replaced `(PR 10)` / `(PR 10b)` markers in `action.rs` and `download.rs` with the actual module path now that the UI overlay has shipped. |
| C4  | Run journal recovery *before* singleton lock acquired         | 🔴       | ✅ Done        | `main.rs` now acquires the singleton guard *first* and only runs `apply_journal::recover` afterwards (on the lock-winning path or the OS-error soft-fail path). A losing-race second instance exits before recovery can touch live install files. |
| C5  | Recovery of `Applying` fails on Windows when target survives  | 🔴       | ✅ Done        | `recover` now removes a partially-written `target` before renaming `backup -> target`, and only deletes the journal when every rollback step succeeded (locked / failed restores leave the journal in place for the next startup to retry). 3 new unit tests cover the partial-target rename, the journal-preservation-on-failure path, and the missing-backup idempotence case. |
| C6  | `--no-default-features` build is broken                       | 🔴       | ✅ Done        | Resolved by removing the `self-update` cargo feature entirely (per M9 the `[Options] UpdaterInstallEnabled` config is now the sole install-disable knob). `tar` / `flate2` are no longer optional; all `#[cfg(feature = "self-update")]` gates dropped from `mod.rs`, `apply_journal.rs`, `cli.rs`. `main.rs` now calls `apply_journal::recover` directly with no shim. |
| M12 | Cancellation generation token (worker-result race)            | 🟠       | ✅ Done        | Replaced the global `CANCEL: AtomicBool` with a monotonic `OP_GENERATION: AtomicU64` bumped by every `request_check_now` / `request_download` / `request_cancel`. Workers capture their generation at spawn, poll `worker_should_stop(gen)` for cancellation, and publish via `set_phase_if_current(gen, _)` which silently drops stale Ready/Error/Downloading writes. Closes the race where a cancelled worker's late result clobbered a fresh worker's state. |
| M13 | Cancellation not checked after final flush/fsync/rename       | 🟠       | ✅ Done        | `stream_to_file` now re-polls `should_cancel` after EOF/before fsync and again after the hash check; `download_to_file` re-polls after the rename succeeds and removes the renamed archive on a late cancel; `run_fake_download` does the same after its progress loop. Combined with M12's `set_phase_if_current` guard, a Back press anywhere in the post-stream tail leaves no `Ready` phase and no leftover archive. |
| M14 | Windows download rename fails when `dest` already exists      | 🟠       | ✅ Done        | New `replace_file(staging, dest)` helper does an explicit `remove_file(dest)` (NotFound-tolerant) before `rename` on Windows; POSIX keeps the plain atomic `rename`. The pre-delete sidesteps `MoveFileExW` edge cases on AV-instrumented / network paths. Tests `replace_file_moves_staging_onto_missing_dest` and `replace_file_overwrites_pre_existing_dest` cover both branches. |
| M15 | Apply is add/replace-only — removed files stick forever       | 🟠       | ⏭️ Deferred    | Scope is large (Op-kind enum, manifest format, planner rewrite, release tooling) and there is no concrete release-removal pending. Revisit when the first DLL/asset actually needs deleting. Recommended path: ship a small cumulative `removed.txt` per release rather than a full file manifest. |
| M16 | Case-insensitive collisions in apply plan                     | 🟠       | ✅ Done       | A staging tree containing `foo.dll` + `FOO.dll` produces two ops mapping to the same NTFS target; the second backs up what the first just installed. Detect collisions during `plan_ops` and fail before journal write. |
| M17 | Pre-journal extraction failures leak staging directories      | 🟡       | ✅ Done       | If extraction / planning / first journal write fails, the `.deadsync-update-staging-*` dir is never cleaned up. Wrap pre-journal apply setup with cleanup-on-error; existing recovery only handles the post-journal-write window. |
| M18 | Cached release URLs from a prior override survive into release builds | 🟠 | ✅ Done | `state.rs` persists `cached_release` with full asset URLs. A dev/CI run that pointed `DEADSYNC_UPDATER_RELEASE_URL` at localhost can leave a cached release whose `browser_download_url` isn't on `github.com`. C2/M6 should also drop cached releases whose host isn't the canonical one. |
| M19 | `AvailableNoInstall` UX is dead-end on console / no-keyboard input | 🟡  | ✅ Done | Resolved by removal: when `apply_supported_for_host()` is false or `UpdaterInstallEnabled = 0`, `options::activate_current_selection` no-ops the row and the renderer skips it. The menu banner still surfaces available releases through the passive check, so users on externally-managed builds (macOS, Steam, distro packages) still learn about new versions. |
| M20 | I/O errors lose path/operation context                       | 🟡       | ✅ Done       | Added `io_err_at(op, path, err)` and `io_err_op(op, err)` helpers in `src/engine/updater/mod.rs`; call sites in `download.rs`, `apply_journal.rs`, `apply_unix.rs`, and `apply_windows.rs` now produce `IO("create '...': os error 5")` style messages instead of bare `os error 5`. |
| C7  | Journal & apply renames don't fsync the parent directory     | 🟠       | ✅ Done       | Added `super::sync_dir(path)` helper (POSIX: `File::open(path)?.sync_all()`; Windows: no-op). `apply_journal::write_atomic` fsyncs `exe_dir` after renaming the journal; `apply_unix::execute_with_rollback` fsyncs each unique target-parent after all renames complete. |
| M21 | Apply silently flips non-portable installs to portable mode   | 🟠       | ✅ Done       | Release archives ship `portable.txt` because that's how a freshly-extracted ZIP is meant to behave. Applying that over a non-portable install would create `portable.txt` next to the executable and hide the user's existing AppData/XDG config on next launch. New `apply_journal::is_portability_marker(rel)` predicate; both `plan_ops` impls skip `portable.txt`/`portable.ini` when the target file does not already exist. |
| C8  | Apply has no allowlist — can overwrite user data/config files | 🔴       | ⏳ Not started | Planner accepts every staged regular file (`apply_unix.rs:202-230`, `apply_windows.rs:218-246`). In portable installs `data_dir == exe_dir`, so a packaging mistake / compromised release that ships `deadsync.ini`, `save/`, `songs/`, `courses/`, `cache/`, or `deadsync.log` would silently overwrite or hide user state. Add an explicit denylist (or, better, a release manifest of app-owned paths) and reject staged entries that target user-owned roots. |
| M22 | Live rollback failure deletes the journal anyway              | 🟠       | ✅ Done       | `execute_with_rollback` now returns `apply_journal::ExecuteFailure { cause, rollback_clean }`; `rollback()` reports failure on any rename error and the caller (`apply_tar_gz` / `apply_zip`) only removes the journal + staging when the rollback was clean. When dirty, the `Applying` journal is left for next-launch `recover()` to retry. POSIX rollback also fsyncs every restored parent. |
| C9  | Env-var release URL & fake-download active in release builds  | 🔴       | ⏳ Not started | Subsumes C2 + M6: `DEADSYNC_UPDATER_RELEASE_URL` (`mod.rs:47-50`) and `DEADSYNC_UPDATER_FAKE_DOWNLOAD` (`action.rs:393-397`) are unconditional. Together they let any process that controls the launch environment redirect the checker and stage an arbitrary local archive — the fake path bypasses sidecar + API digest entirely and publishes `Ready` with the hash of the attacker bytes. Gate both behind `cfg(any(test, debug_assertions, feature = "updater-test-overrides"))` and add a release-build test that proves the env vars are ignored. |
| M23 | Relaunch may exec the renamed-out backup binary               | 🟠       | ✅ Done       | `apply_archive_and_relaunch` now captures `original_exe = std::env::current_exe()` and the install-tree path `exe_dir.join(original_exe.file_name())` *before* any apply rename, then spawns that captured path. `relaunch_self` no longer calls `current_exe()` itself, so on Linux `/proc/self/exe` resolving to the renamed-out backup can't redirect the relaunch. |
| M24 | Extracted file contents not fsynced before rename             | 🟠       | ✅ Done       | After `io::copy` of each tar/zip entry, `extract_tar_gz` and `extract_archive` now call `out.sync_all()` (with M20 IO-error context) before the file handle drops, so the C7 parent-dir fsync makes both directory entry *and* file body durable across power loss. |
| M25 | Planner doesn't reject dir-vs-file type mismatches            | 🟠       | ✅ Done       | `plan_ops` now calls `fs::symlink_metadata(&target)` and refuses to plan an Op when the existing entry isn't a regular file (catches dir-at-target and symlink-at-target before the journal is ever written). Mirrored on both platforms with unit-test coverage. |
| M26 | No read/idle timeout on download body                         | 🟠       | ⏳ Not started | `download_agent` deliberately has no global timeout (`mod.rs:83-89`), and `stream_to_file` only polls `should_cancel` between chunks (`download.rs:411-417`). A server that accepts the connection then stalls mid-body blocks the worker inside `read()` indefinitely; Back press bumps generation but the worker can't observe it. Configure a read/low-speed timeout on `download_agent`; on timeout, return `Cancelled` if generation is stale or surface a network timeout. ETA stays `None` during the stall and we never publish a fake estimate. |
| M27 | Apply success + relaunch failure looks like apply failure     | 🟠       | ⏳ Not started | If `apply_archive_and_relaunch` succeeds at apply but fails to spawn (`cli.rs:143-147`, `action.rs:665-674`), the current old process keeps running against a mutated install tree and an `Applied` journal. Distinguish the two outcomes: on apply-ok-relaunch-fail, publish a phase that says "Update installed; please restart manually" (and ideally `process::exit` rather than continue). |
| M28 | Tar extractor silently skips symlinks/devices instead of rejecting | 🟡    | ✅ Done | `apply_unix::extract_tar_gz` skips non-file/non-dir entries (`apply_unix.rs:163-167`). If a future release artifact accidentally ships a symlink that runtime depends on, the apply succeeds with missing files. Fail closed: reject any non-regular, non-directory entry. |
| M29 | Future-version journal recovery is silent                     | 🟡       | ✅ Done | When `Journal::load` parses a journal whose state is unknown (e.g. a downgraded/old binary picking up a newer-format journal), `recover()` leaves it untouched and returns a no-op `RecoveryReport` with no warning log (`apply_journal.rs:210-216, 449-458`). Add a `log::warn!` so the stuck state is visible in support reports. |
| M30 | Progress publication is per-chunk and clones release metadata | 🟡       | ✅ Done | `download_to_file`'s progress closure fires every 64 KiB and clones `ReleaseInfo` + `ReleaseAsset` into a fresh `Downloading` phase under the `PHASE` write lock (`action.rs:454-489`). On a fast connection or large future archive that's a lot of allocator/lock churn for no UI benefit. Throttle to ~5–10 Hz plus a final update, or only publish when `(percent, eta_bucket)` actually changes. |
| M31 | Redirect host pinning isn't explicit                          | 🟡       | ⏳ Not started | M18 sanitizes cached URLs and N2 cross-checks the API digest, but live asset/sidecar requests follow whatever redirect chain GitHub returns without an explicit allowlist (`download.rs:241-256, 324-329`). GitHub release downloads do legitimately redirect to GitHub-owned object/CDN hosts, so the policy needs to be written down (and ideally enforced) rather than implicit. Becomes much less load-bearing once C1 (signature verification) ships, since CDN host trust would no longer be the security boundary. |

---

## 🔴 Critical

### C1. Add independent signature verification for downloaded artifacts

- **Problem:** `download.rs` fetches `<asset>.sha256` from the same
  release the asset came from
  (`src/engine/updater/download.rs:136-156, 248-254`). The trust root is
  effectively "GitHub release + maintainer account." A compromised
  maintainer account or release silently delivers a malicious update.
- **Fix:** add a minisign / ed25519 signature file (e.g. `.minisig`)
  produced by an offline signing key during release. Embed the public
  key in the binary at build time. Verify the signature before
  transitioning to `Ready`. Treat the SHA256 sidecar as an integrity
  check only.
- **Acceptance:**
  - Apply path refuses to install if the signature file is missing,
    malformed, or doesn't verify against the embedded key.
  - The signing key is documented and rotated through a versioned
    process.
- **Design prereqs (lock down before implementing):**
  - **What is signed?** Recommend signing a manifest (JSON or
    minisign-trusted-comment) that pins `tag`, `asset_name`,
    `asset_size`, `sha256`, and `host_target`. Signing only the archive
    bytes leaves tag/version/asset-mapping unverified.
  - **Where is verification re-done?** `Ready` snapshot must carry
    enough state for `apply_archive_and_relaunch`
    (`src/engine/updater/cli.rs:147-173`) to re-verify the *signature*
    too, not just the SHA-256 (M2 only handles the digest).
  - **Key management.** Embed one or more ed25519 public keys at
    build time. Document a documented rotation policy (overlap window,
    revocation file, etc.).
  - **Downgrade policy.** Decide whether the manifest must include a
    monotonically-increasing build/version number to refuse
    downgrades, even if signed.
  - **Failure mode.** Missing/malformed/unverified signature must
    fail closed (no `Ready` transition) rather than silently degrade
    to digest-only.

### C2. Gate `DEADSYNC_UPDATER_FAKE_DOWNLOAD` to dev/test builds only

- **Problem:** `run_download` honors
  `DEADSYNC_UPDATER_FAKE_DOWNLOAD` in any build and copies the local
  file straight to `Ready` without checksum/signature verification
  (`src/engine/updater/action.rs:252-255, 318-370`). Combined with a
  normal `request_apply`, an attacker who can influence the process
  environment (launcher, shortcut, frontend) can install an arbitrary
  archive.
- **Fix:** wrap the override in `#[cfg(any(test, debug_assertions,
  feature = "updater-test-overrides"))]`. Default release builds must
  not honor it. Same treatment for `DEADSYNC_UPDATER_RELEASE_URL` (see
  M6).
- **Acceptance:**
  - A release build ignores the env var and follows the normal download
    path.
  - The portable test script
    (`scripts/test-updater-portable.ps1`) opts into the test-overrides
    feature explicitly.

### C3. Stop deleting arbitrary `*.old` files under the install root

- **Problem:** `apply_windows::cleanup_stale_old_files` recursively
  removes every file ending in `.old`
  (`src/engine/updater/apply_windows.rs:273-299`). Portable installs
  store user `songs/`, `courses/`, etc. under the executable root
  (`src/config/dirs.rs:197-204`), so legitimate user content named
  `*.old` will be deleted.
- **Fix:** record the exact set of displaced files in a manifest written
  next to the staging directory during apply, and on next launch only
  delete paths listed in that manifest. Optionally add a unique
  updater-generated suffix (e.g. `.deadsync-old-<timestamp>`) so the
  pattern can't collide with user files.
- **Acceptance:**
  - Cleanup never traverses into `songs/`, `courses/`, or any directory
    not part of the install tree.
  - Test: create `songs/foo.old` before applying; verify it survives
    cleanup.

---

## 🟠 Major

### M1. Make `apply` transactional / rollback-capable

- **Problem:** Both Windows
  (`src/engine/updater/apply_windows.rs:313-317`) and Unix
  (`src/engine/updater/apply_unix.rs:187-194`) explicitly leave
  partially-installed state on mid-walk failure. Disk-full, AV locks,
  permission changes, or a second running instance can produce a mixed
  old/new tree.
- **Fix:** preflight the entire move list (free space, file locks where
  practical), stage and verify the full tree, then perform the moves.
  On any failure during the moves, run a recorded rollback that restores
  every displaced file from its `.old` (or manifest-tracked) backup.
- **Acceptance:** an injected mid-apply error leaves the install
  bit-identical to its pre-apply state.

### M2. Re-verify the staged archive immediately before extraction

- **Problem:** `Ready` only carries the archive path
  (`src/engine/updater/action.rs:287-291`); apply consumes it without
  re-checking the digest (`src/engine/updater/action.rs:388-399`,
  `src/engine/updater/cli.rs:141-147`). A modified file between
  download and apply would be installed.
- **Fix:** persist the expected digest (and signature, after C1) with
  `Ready`, and re-hash/re-verify in `apply_archive_and_relaunch` before
  extracting.

### M3. Persist enough release metadata to reconstruct `Available` after a 304

- **Problem:** `UpdaterCache` stores only `last_checked_at`,
  `last_seen_tag`, and `etag`
  (`src/engine/updater/state.rs:37-43`). On `304 Not Modified` the
  state machine only bumps `last_checked_at`
  (`src/engine/updater/state.rs:175-183`), so a previously-seen update
  is invisible on the next launch.
- **Fix:** persist the minimal release metadata needed to materialize
  `UpdateState::Available` (tag, name, body, asset URL/digest), and
  restore it on 304.

### M4. Use `last_checked_at` to throttle startup checks

- **Status:** Closed as won't-fix.
- **Problem (original):** `decide` only inspects the env opt-out
  (`src/engine/updater/state.rs:122-128`); every launch contacts
  GitHub. Comments still implied throttling exists.
- **Resolution:** Reviewed and intentionally not implemented. Every
  startup poll sends an `If-None-Match` ETag, so the steady-state
  response is a 304 with an empty body — cheap on both sides. The
  worst case (NAT'd shared IP exceeding 60/hr) is already handled by
  the `UpdaterError::RateLimited` path: logged, snapshot untouched,
  banner survives via the M3 cached release. Adding an interval gate
  would have required a new tunable, a state-dependent skip path,
  and "why is Check For Updates stale?" UX questions for marginal
  benefit. The misleading throttling comments were removed and the
  unused `last_checked_at` field was dropped from `UpdaterCache`.

### M5. Wire up `UpdateChannel::Prerelease` or remove the choice

- **Status:** Done — chose to remove the choice.
- **Problem (original):** `UpdateChannel` exposed `Stable` and
  `Prerelease` and was loaded from config, but release fetching
  always hit `/releases/latest`, so the setting did nothing.
- **Resolution:** Deleted the `UpdateChannel` enum, the
  `Config::update_channel` field, the `update_update_channel` setter,
  the `UpdateChannel` ini key in load / save / defaults, and the two
  associated tests. The updater always polls `/releases/latest`. If
  prerelease support is needed later, the right move is to switch to
  the releases-list endpoint with explicit filtering rather than
  resurrecting a no-op setting.

### M6. Gate `DEADSYNC_UPDATER_RELEASE_URL` to dev/test builds

- **Problem:** the override is honored in production
  (`src/engine/updater/mod.rs:37-45`), letting a malicious launcher
  point the updater at `http://localhost`.
- **Fix:** same gating as C2. If kept in any build, enforce HTTPS and
  optionally a host allowlist.

### M7. Thread `ApplyOutcome.staging_dir` into `relaunch_self`

- **Problem:** `apply_zip` returns the staging dir
  (`src/engine/updater/apply_windows.rs:325-336`) but
  `relaunch_self` recomputes a fresh one with a new timestamp
  (`src/engine/updater/cli.rs:189-199`). The new process tries to clean
  a directory that was never used; the real one is left behind.
- **Fix:** plumb `ApplyOutcome.staging_dir` through
  `apply_archive_and_relaunch` into `relaunch_self`.

### M8. Don't offer in-app install on platforms where apply is unsupported

- **Problem:** `host_target` advertises macOS assets
  (`src/engine/updater/mod.rs:232-240`), so macOS reaches `Ready`, but
  `apply_for_host` rejects everything except Windows / Linux / FreeBSD
  (`src/engine/updater/cli.rs:150-179`).
- **Fix:** introduce an `apply_supported_for_host()` helper, and on
  unsupported platforms either disable the install button before
  download or replace it with "Open release page in browser."
- **Status (resolved):**
  - `engine::updater::apply_supported_for_host()` (in `mod.rs`) returns
    true only when `cfg!(all(feature = "self-update", any(target_os =
    "windows", target_os = "linux", target_os = "freebsd")))` —
    exactly mirroring the cfg gate on `cli::apply_for_host`. A unit
    test (`apply_supported_matches_cfg_targets`) keeps the two in sync.
  - `action::classify_check_result` checks the gate when the remote
    state is `Available` and a matching asset exists. Unsupported hosts
    transition to a new `ActionPhase::AvailableNoInstall { info }`
    instead of `ConfirmDownload`, so the worker never accepts a
    `request_download` call on macOS / unsupported builds.
  - `update_overlay` renders `AvailableNoInstall` with the release tag
    as the focal point, body text "In-app install isn't available on
    this platform. Visit:" + `info.html_url` (truncated to 80 chars),
    and a Dismiss-only footer. No browser-launching dependency was
    added; the URL is shown so the user can navigate to it manually.

### M9. Make in-app install opt-out for managed distributions

- **Problem:** `self-update` is in default features
  (`Cargo.toml:36-43`). Risky for Steam/MSIX/distro/Flatpak/Snap builds
  where the host owns updates. The cargo feature alone isn't enough:
  even a `--no-default-features` build still runs the startup check,
  shows the menu banner, and exposes the "Check for Updates" Options
  row — managed builds hit a useless dead-end overlay (post-M8) when
  the user clicks through.
- **Resolution (runtime opt-out, not a build-time mode):** added
  `[Options] UpdaterInstallEnabled` (default `1`). When set to `0`:
  - `classify_check_result_with(state, install_enabled=false)` routes
    every successful Available → `AvailableNoInstall { info }`, so the
    overlay shows the release tag + GitHub URL with Dismiss only and
    never reaches `ConfirmDownload`.
  - `request_download` re-checks the gate and re-routes to
    `AvailableNoInstall` if a stale `ConfirmDownload` phase is still
    visible from before the operator flipped the flag — the worker is
    never spawned.
  - Banner, startup check, and the "Check for Updates" Options row are
    intentionally **left enabled**: a managed build still tells the
    user a new release exists, just doesn't try to install it.
- **Why a config key, not a cargo feature:** one binary serves all
  channels (Steam ships the same exe as the GitHub release), no CI
  matrix doubling, and packagers just drop the value into the ini they
  ship. (As of C6 the previously-considered `self-update` cargo
  feature has been removed entirely — it was redundant with this
  config key, and was already broken in `--no-default-features`
  builds. `UpdaterInstallEnabled=0` is now the only supported way to
  disable in-app installs.)
- **Tests:**
  - `classify_check_result_with_install_disabled_skips_download`
    asserts the gate flips Available → `AvailableNoInstall` even on
    hosts where apply is supported.
  - The same test re-runs with `install_enabled = true` to guard
    against future regressions silently disabling installs everywhere.

### M10. Add an inter-process updater lock

- **Problem:** the action state machine uses in-process locks only
  (`src/engine/updater/action.rs:110-116`). Two running instances can
  both download/apply, or one can apply while another holds files
  open.
- **Fix:** acquire a cross-process lock (lockfile or named mutex) in
  `apply_archive_and_relaunch`, and refuse to start apply if another
  instance is detected. Detect at check-now time and surface a clear
  error.

### M11. Reconcile `REQUEST_TIMEOUT` with the shared HTTP agent

- **Problem:** `REQUEST_TIMEOUT = 8s`
  (`src/engine/updater/mod.rs:55-58`) is unused; real requests use the
  shared 10 s agent (`src/engine/network.rs:9, 69-72`). The constant
  is only kept alive by an import in `download.rs`
  (`src/engine/updater/download.rs:255`).
- **Fix:** either delete the constant or build a dedicated updater
  agent with explicit connect / read / total timeouts (downloads in
  particular need a different policy than 10 s).

---

## 🟡 Minor

### N1. ETag bookkeeping

- **Problem:** On a fresh response without an ETag, `next.etag` retained
  the previous one (`src/engine/updater/state.rs:184-194`). Set it to
  `None` explicitly, or scope ETags by release URL/channel to avoid
  leakage.
- **Resolution:** lifted the cache-update logic out of `run_check_once`
  into a pure `apply_fresh_to_cache(prev, state, tag, etag)` helper,
  and changed the assignment to `prev.etag = etag` (unconditional).
  GitHub effectively always sends an ETag, but if it ever stops, we
  now drop the previous value instead of letting it match an unrelated
  payload on the next request and trigger a spurious 304.
- **Tests:**
  - `apply_fresh_clears_etag_when_response_has_none` exercises the
    bug-fix branch directly.
  - `apply_fresh_overwrites_etag_with_new_value`,
    `apply_fresh_clears_cached_release_on_up_to_date`, and
    `apply_fresh_preserves_cached_release_on_unknown_latest` lock down
    the rest of the bookkeeping behavior so a future refactor of the
    helper can't silently regress M3 or this fix.
- **Channel-scoping deferred:** the second half of the original fix
  ("scope ETags by release URL/channel") is moot today — M5 removed
  `UpdateChannel` and there's only one release URL.

### N2. Verify GitHub's API `digest` field too

- **Problem:** `ReleaseAsset.digest` is captured
  (`src/engine/updater/mod.rs:60-69`) and shown in the UI
  (`src/screens/components/shared/update_overlay.rs:302-307`) but is
  not used for verification. If present, fail closed on any mismatch
  between API digest, sidecar, and downloaded bytes.
- **Resolution:** added two pure helpers in
  `src/engine/updater/download.rs`:
  - `parse_api_digest(value)` parses GitHub's `"<algo>:<hex>"` form;
    returns `Some([u8; 32])` for sha256, `None` for any other
    algorithm we can't verify, and `Err(ChecksumSidecarMalformed)`
    on bad syntax.
  - `cross_check_api_digest(api, sidecar) -> ApiDigestCheck` compares
    the parsed API digest to the sidecar digest, returning `Absent`
    / `UnsupportedAlgorithm` / `Matched`, or `Err(ChecksumMismatch)`
    when the API digest is sha256 and disagrees with the sidecar.
  Wired into `action::run_download` between sidecar parsing and the
  streaming download — a mismatch now flips the UI to the same error
  phase as a downloaded-bytes mismatch, before any payload bytes
  cross the wire.
- **Tests:** seven new unit tests in `download::tests` covering the
  algo-prefix parser (lower/upper case, unknown algo, missing prefix,
  bad hex, wrong length) plus the four cross-check branches
  (Absent / Matched / UnsupportedAlgorithm / mismatch fails closed /
  parse error propagated).

### N3. Add cancellation during long checks/downloads

- **Problem:** the overlay intentionally consumed input during
  checking/downloading
  (`src/screens/components/shared/update_overlay.rs:423-425`). Add a
  cancel binding for downloads and a "Later" escape during checking
  where safe.
- **Resolution:**
  - `src/engine/updater/mod.rs`: added `UpdaterError::Cancelled`.
  - `src/engine/updater/download.rs`: `download_to_file` and
    `stream_to_file` take a `should_cancel: impl Fn() -> bool` polled
    before each chunk; on cancel they return `Cancelled` and the
    partial file is removed by the existing cleanup path.
  - `src/engine/updater/action.rs`: new `static CANCEL: AtomicBool`
    plus `request_cancel()` / `cancel_requested()` / `clear_cancel()`.
    `request_cancel` is a no-op unless the current phase is
    `Checking` or `Downloading` — `Applying` deliberately can't be
    cancelled because a partial extract / swap would corrupt the
    install. Workers clear the flag at start, poll after the sidecar
    fetch, and pass `cancel_requested` to `download_to_file`. The
    fake-download path also polls between sleeps.
  - `src/screens/components/shared/update_overlay.rs`: Back
    (`p1_back` / `p2_back`) during Checking / Downloading now calls
    `action::request_cancel()`; all other input is still swallowed.
- **Tests:** added 5 tests:
  - `stream_to_file_returns_cancelled_when_flag_set_before_first_chunk`
    (early cancel before any bytes); 
  - `stream_to_file_returns_cancelled_mid_stream` (cancel after a few
    chunks);
  - `request_cancel_flips_checking_to_idle_and_sets_flag`,
    `request_cancel_flips_downloading_to_idle_and_sets_flag`,
    `request_cancel_is_noop_outside_check_or_download`
    (covers Applying + Idle being uncancellable).

### N4. Stage downloads to `*.part`, then atomically rename

- **Problem:** `download_to_file` wrote directly to the final cache path
  (`src/engine/updater/download.rs:187-227`). Crash/power loss left
  partial files behind. Write to `*.part`, fsync, rename when verified.
- **Resolution:** added `staging_path(dest) -> PathBuf` (`<dest>.part`)
  and rewired `download_to_file` to:
  1. remove any leftover `<dest>.part` from a prior crashed run;
  2. stream into the staging file (cancel still cleans up via the
     existing best-effort path);
  3. `flush()` then `sync_all()` so bytes are durable on disk;
  4. only after sha256 verifies, `fs::rename(staging, dest)` —
     atomic on every supported filesystem (NTFS / ext4 / APFS / HFS+
     all guarantee the rename or no rename).
  A pre-existing `dest` from a previous successful download is left
  untouched on cancel / mismatch, so a reboot mid-redownload doesn't
  wipe a usable archive.
- **Tests:** added `staging_path_appends_part_extension`,
  `staging_path_handles_extensionless_filenames`, and updated the
  three existing `stream_to_file_*` tests to assert that
  `stream_to_file` writes to `<dest>.part` and never touches `dest`
  itself (the rename is `download_to_file`'s contract).

### N5. Audit unused i18n keys

- **Problem:** `BodyAvailable`, `BodyDownloading`, `BodyReady`,
  `BodyApplyHint` looked unused in the current overlay.  Either wire
  them in or drop them.
- **Resolution:** confirmed via repo-wide grep that none of the four
  keys are referenced from any source file (only `BodyReadyShort` is
  used, by the Ready-phase footer in `update_overlay.rs`).  Dropped
  the four dead keys from `assets/languages/en.ini` and `sv.ini` and
  regenerated `pseudo.ini` via `cargo run --bin generate_pseudo` so
  the pseudo-locale stays in sync with `en.ini`.

### N6. Refresh stale comments

- **Problem:** `src/engine/updater/state.rs:177-178, 208-210` still
  referenced Daily mode / config-driven frequency, which no longer
  exist.
- **Resolution:** the specific Daily / frequency comments were already
  cleaned up by the M3/M4 work; this pass mopped up the remaining
  stale references found by a fresh repo-wide grep:
  - `state.rs` module doc said "user-editable `Settings.ini`" — the
    actual filename is `deadsync.ini`. Fixed.
  - `UpdaterCache::last_seen_tag` doc still pointed at "M5 (channel
    wiring) will use it" — M5 actually *removed* channels. Rewrote
    the comment to call the field "informational, kept for future
    dismissal-tracking work" without the obsolete forward reference.
  - `action.rs` and `download.rs` carried `(PR 10)` / `(PR 10b)`
    in-progress markers from the original PR series. Replaced with
    the concrete module path
    (`screens::components::shared::update_overlay`) now that the UI
    overlay has shipped.

---

## 🔴 Critical (added by post-N rubber-duck pass)

### C4. Run journal recovery *before* the singleton lock is acquired

- **Status:** Done.
- **Problem:** `src/main.rs:206-227` (pre-fix) called
  `engine::updater::apply_journal::recover(&exe_dir)` before
  `src/main.rs:239-245` acquired the single-instance guard. After a
  self-update relaunch the previous process can still be in the tail of
  `Applying` (extracting / renaming the last few files) for several
  hundred milliseconds. A second user-initiated launch during that
  window would:
  1. read the journal that the still-running process is mid-way through
     committing,
  2. attempt rollback (rename `backup -> target`) on files the original
     process is still holding open,
  3. delete the journal file out from under the original process,
  4. only *then* try to take the singleton lock and exit with
     `AlreadyRunning`.
  By that point the install tree was in an undefined mixed state.
- **Resolution:** swapped the order in `src/main.rs`. The singleton
  lock is acquired first (with the existing 3-second `--restart`
  retry); recovery runs only on the lock-winning path or the OS-error
  soft-fail path. The `AlreadyRunning` branch now `process::exit(1)`s
  before any journal access. The recovery comment was updated to call
  out the ordering invariant so it isn't accidentally moved back.

### C5. `recover(Applying)` doesn't restore backups when the new target survives the crash

- **Status:** Done.
- **Problem:** `src/engine/updater/apply_journal.rs:376-389` (pre-fix)
  rolled back with `fs::rename(&op.backup, &op.target)`. On Windows,
  `rename` fails if `target` already exists. The `Applying` state
  could be left mid-op with both `backup` *and* a partially-written
  new `target` (e.g. `target -> backup` succeeded, `staged -> target`
  partially completed, then crash). Recovery's rename then errored,
  but the code proceeded to remove the staging dir *and* the journal
  anyway, permanently losing the rollback recipe and leaving the
  install tree mixed.
- **Resolution:**
  - `recover` now removes a partially-written `target` first when both
    `backup` and `target` exist, so the subsequent
    `fs::rename(backup, target)` succeeds on Windows.
  - Per-op success is tracked via a local `all_ops_succeeded` bool;
    the journal is removed only when every recoverable op completed.
    Locked or otherwise unrecoverable ops leave the journal in place
    so the next startup can retry — fulfilling the doc-comment promise
    that "the journal is only removed when the major work succeeded."
  - `Applied` cleanup gets the same treatment: backup deletions that
    return `NotFound` are treated as already-cleaned (idempotent
    second pass), but real I/O errors keep the journal around.
  - Staging-dir cleanup failure no longer blocks journal removal —
    leftover staging is annoying but not corruption.
- **Tests:** 3 new unit tests in `apply_journal::tests`:
  - `recover_applying_overwrites_partial_target_with_backup` exercises
    the Windows-specific partial-target rename path.
  - `recover_applying_preserves_journal_when_partial_install_cant_be_removed`
    uses a non-empty directory at the target path to force
    `remove_file` to fail and asserts the journal survives.
  - `recover_applied_skips_already_missing_backup_and_still_drops_journal`
    proves a partially-cleaned `Applied` journal can be finished by a
    second recovery pass.

### C6. `--no-default-features` build is broken

- **Problem:** `src/main.rs:217` calls
  `engine::updater::apply_journal::recover` unconditionally, but
  `src/engine/updater/mod.rs:26-27` only declares the module behind
  `#[cfg(feature = "self-update")]`. `cargo check
  --no-default-features` fails. The M9 plan claims packagers can ship
  with `self-update` disabled to strip the apply code; today they
  can't.
- **Resolution:** the `self-update` cargo feature has been removed
  entirely. After M9 made `[Options] UpdaterInstallEnabled` the
  runtime knob, the feature was redundant — and broken. Dropping it
  also removes a class of "compiles in one config, breaks in the
  other" bugs. `tar` and `flate2` are now unconditional dependencies
  (small cost: ~tens of KB of compiled code on macOS where the apply
  path isn't reachable). All `#[cfg(feature = "self-update")]` gates
  in `mod.rs`, `apply_journal.rs`, and `cli.rs` were dropped; the
  `recover_at_startup` shim was removed and `main.rs` now calls
  `apply_journal::recover` directly. `cargo build` and the 130
  updater unit tests pass.

### C7. Journal & apply renames don't fsync the parent directory

- **Problem:** `src/engine/updater/apply_journal.rs:164-179`
  (`write_atomic`) fsyncs the temp file then renames it; the apply-time
  renames in `apply_windows.rs:263-283` and `apply_unix.rs:239-258` do
  the same. Neither fsyncs the *containing directory*. On POSIX
  filesystems, the directory entry created by the rename can be lost
  on power loss even though the renamed file's bytes are durable.
- **Fix:** Added `pub fn sync_dir(path: &Path) -> io::Result<()>` in
  `src/engine/updater/mod.rs`. POSIX implementation opens the
  directory and calls `sync_all`; Windows is a no-op because NTFS
  commits directory metadata as part of the rename. Call sites:
  `apply_journal::write_atomic` fsyncs `exe_dir` after the journal
  rename; `apply_unix::execute_with_rollback` collects each unique
  target-parent and fsyncs once at the end of a successful apply
  (rollback path doesn't need it — the contents are already on
  disk and we're only restoring the prior state). Best-effort: a
  failed `sync_dir` is logged-then-ignored, since the file bytes are
  already durable and the worst case is a lost rename on power loss
  rather than corruption.
- **Acceptance:** ✅ Done. Comments and behaviour now match: the
  durable-journal claim in `write_atomic` is backed by an `exe_dir`
  fsync, and the apply path fsyncs each parent directory it
  touched. Existing 149 updater unit tests still pass on Windows
  (sync_dir is a no-op there).

---

## 🟠 Major (added by post-N rubber-duck pass)

### M12. Cancellation generation token (worker-result race)

- **Problem:** `src/engine/updater/action.rs:155-173` flips the phase to
  `Idle` and sets `CANCEL` synchronously, but the worker keeps running
  until its next poll. If the user starts a new check/download in
  between, the new worker calls `clear_cancel()`
  (`src/engine/updater/action.rs:270`, `src/engine/updater/action.rs:336`)
  and the *old* worker — which never observed the flag — eventually
  returns `Ok` and calls `set_phase`, clobbering the new worker's
  state with a stale `Ready` / `ConfirmDownload` / `Error`.
- **Resolution:** replaced the global `CANCEL` bool with a monotonic
  `OP_GENERATION: AtomicU64`. `request_check_now`, `request_download`,
  and `request_cancel` each call `begin_operation()` (fetch_add, return
  new). Each worker captures its generation at spawn and polls
  `worker_should_stop(gen)` — true when the global counter has moved
  past `gen`. Publication happens through `set_phase_if_current(gen,
  next)`, which re-checks the generation under the `PHASE` write lock
  before storing, so a stale worker's late result is silently dropped
  rather than clobbering fresh state. New unit tests
  `set_phase_if_current_drops_stale_worker_writes` and
  `worker_should_stop_returns_true_when_generation_advances`
  exercise the guard directly; the existing `request_cancel_*` tests
  were updated to assert the generation bump.

### M13. Cancellation not checked after final flush/fsync/rename

- **Problem:** `stream_to_file` (`src/engine/updater/download.rs:374-416`)
  polls `should_cancel` between chunks but stops checking after EOF;
  `download_to_file` then flushes, fsyncs, hashes, and renames without
  re-checking. The fake-download path
  (`src/engine/updater/action.rs:497-518`) has the same final-window
  bug. A Back press during the multi-second tail of a large archive
  flips the phase to `Idle` but the worker still publishes `Ready`
  immediately afterwards.
- **Resolution:** added three additional cancel-poll points so the
  whole post-final-chunk window is covered:
  - `stream_to_file` polls between EOF and the flush+fsync, and again
    after the hash matches (so a checksum-success result is still
    suppressed if the user has cancelled).
  - `download_to_file` polls after the `rename(staging, dest)`
    succeeds; on cancel it removes the renamed archive and returns
    `UpdaterError::Cancelled`, so a future attempt starts clean
    instead of racing with a Ready-shaped artifact on disk.
  - `run_fake_download` polls after its progress loop (matching the
    real-download cleanup) and removes the staged file on cancel.

  Combined with M12's `set_phase_if_current` guard, a Back press
  anywhere in the post-stream tail leaves no `Ready` phase and no
  leftover archive. New unit tests
  `stream_to_file_returns_cancelled_after_eof_before_fsync` and
  `stream_to_file_returns_cancelled_after_hash_when_flag_flips_late`
  exercise the two new check points; the post-rename cleanup in
  `download_to_file` is covered by the existing integration testing
  plan (no in-process HTTP fake currently).

### M14. Windows download rename fails when `dest` already exists

- **Problem:** `src/engine/updater/download.rs:352-361` uses
  `fs::rename(staging, dest)`. On Windows this errors if `dest` exists
  — and `dest` is the canonical cache filename, so a user who
  dismisses `Ready` and re-checks will hit `IO("rename ... -> ...:
  ...")` even though the new `.part` verified successfully. N4 fixed
  the partial-file problem but introduced this regression.
- **Resolution:** introduced a `replace_file(staging, dest)` helper
  in `download.rs` that, on Windows, does a `fs::remove_file(dest)`
  (treating `NotFound` as success) before the `rename`. POSIX keeps
  the plain atomic `rename`, which already has replace semantics. The
  Windows pre-delete sidesteps the `MoveFileExW` edge cases observed
  on AV-instrumented and some network-share paths where
  `MOVEFILE_REPLACE_EXISTING` doesn't reliably take effect. The
  non-atomic gap on Windows is bounded by a per-user cache dir with
  no concurrent reader; a crash mid-gap leaves no `dest` and the next
  attempt re-downloads. Tests
  `replace_file_moves_staging_onto_missing_dest` and
  `replace_file_overwrites_pre_existing_dest` cover both branches.

### M15. Apply is add/replace-only — files removed from a release stay forever

- **Problem:** `plan_ops` in `src/engine/updater/apply_windows.rs:204-231`
  and `src/engine/updater/apply_unix.rs:192-218` walks only the
  staging tree, so a release that removes (or renames) a DLL, asset,
  helper binary, or config template leaves the old file installed.
  Stale DLLs in particular can produce mixed-version crashes that are
  very hard to diagnose.
- **Fix:** ship a release-side manifest of "updater-owned" paths
  (relative to the install root). At apply time, journal a delete op
  for any owned path that exists on disk but is not in staging. Confine
  the delete set to the manifest so portable user content
  (`songs/`, `courses/`, `replays/`, etc.) is never touched.
- **Acceptance:**
  - A test release that removes `foo.dll` from the staging tree results
    in `foo.dll` being deleted from the install root (with rollback
    via the journal-stored backup).
  - User content named identically to anything outside the manifest is
    untouched.

### M16. Case-insensitive collisions in apply plan

- **Problem:** `plan_ops` joins each staged relative path onto
  `target_dir` and writes one op per staged file. A staging tree
  containing `foo.dll` and `FOO.dll` produces two ops mapping to the
  same NTFS / default-Windows target. The first op installs file A; the
  second op then backs up file A (calling it the backup of file B!) and
  overwrites it with B. Recovery rollback is now ambiguous and almost
  certainly wrong.
- **Resolution:** `apply_journal::check_no_case_collisions` lowercases
  every op's target (and backup, when `target_existed`) into a single
  `HashMap`; the first repeat returns an `UpdaterError::Io` before the
  journal is written. Both `plan_ops` implementations call the helper
  unconditionally so a cross-platform archive with case-only-different
  names is rejected even when the host filesystem happens to be
  case-sensitive.
- **Tests:** `apply_journal::tests::check_no_case_collisions_*` cover
  the helper directly (accept distinct paths, reject case-only target
  duplicates, reject target-vs-backup overlap).
  `apply_unix::tests::plan_ops_rejects_case_colliding_paths` exercises
  the planner end-to-end with a real staging dir on case-sensitive
  hosts.

### M17. Pre-journal extraction failures leak staging directories

- **Problem:** `src/engine/updater/apply_windows.rs:318-325` (and the
  Unix analogue) creates the staging dir, extracts the archive, plans
  ops, and *then* writes the journal. If anything between
  `create_dir_all(&staging_dir)` and the first `write_atomic(journal)`
  fails (disk full mid-extract, AV quarantine, ZIP corruption,
  case-collision per M16), there's no journal so `recover` never
  cleans up the staging dir, and the next apply attempt creates yet
  another timestamped sibling.
- **Resolution:** `apply_journal::StagingGuard` wraps the pre-journal
  block in both `apply_zip` and `apply_tar_gz`. The guard is
  constructed before `extract_archive` / `extract_tar_gz` and disarmed
  immediately after `journal.write_atomic(exe_dir)?` succeeds. Any
  early return (extract error, plan_ops collision, journal write
  failure) drops the guard and `remove_dir_all`s staging on the way
  out. Once the journal is durable, recovery owns staging cleanup in
  both `Applied` and `Applying` branches.
- **Tests:** `apply_journal::tests::staging_guard_*` cover armed
  drop, disarm, and missing-dir tolerance for the guard itself.
  `apply_windows::tests::apply_zip_cleans_staging_when_extract_fails`
  and `apply_unix::tests::apply_tar_gz_cleans_staging_when_extract_fails`
  feed in a garbage archive and assert no `STAGING_PREFIX` directory
  survives next to the install root and no journal was written.

### M18. Cached release URLs from a prior override survive into release builds

- **Problem:** `src/engine/updater/state.rs:37-50,130-135` persists the
  full `cached_release` (including `assets[].browser_download_url`).
  A developer or CI run that pointed `DEADSYNC_UPDATER_RELEASE_URL` at
  `http://localhost:PORT` can leave a `cached_release` whose asset URLs
  point at localhost. A subsequent release build (which under C2/M6
  should ignore the env var) would happily reconstruct `Available`
  from that cache and try to download from the attacker-controlled
  host.
- **Resolution:** `state::sanitize_loaded_cache` runs on every
  `load_persisted_cache`. It drops `cached_release` when (a) no
  `DEADSYNC_UPDATER_RELEASE_URL` override is currently in effect AND
  (b) any asset's `browser_download_url` is not an `https://` URL on
  `github.com` / `api.github.com`. The cleansed cache is rewritten to
  disk so subsequent launches don't have to re-detect the taint.
  `etag` and `last_seen_tag` are preserved (they leak no host info).
- **Tests:** new in `state.rs`:
  `extract_host_parses_common_shapes`,
  `asset_url_host_canonical_recognises_github_hosts` (rejects
  localhost, attacker hosts, and `http://github.com`),
  `cached_release_canonical_requires_every_asset_canonical`,
  `sanitize_strips_localhost_release_when_override_inactive` (round
  trip on disk too), `sanitize_keeps_localhost_release_when_override_active`,
  `sanitize_keeps_canonical_release`.

### M19. `AvailableNoInstall` UX is a dead end on console / no-keyboard input

- **Problem:** `src/screens/components/shared/update_overlay.rs:324-331`
  renders the GitHub URL truncated to 80 chars; the only input handled
  (`update_overlay.rs:422-431`) is Dismiss. macOS users and managed
  distros (`UpdaterInstallEnabled = 0`) are left with a URL they
  cannot click, copy, or open from a controller-driven UI.
- **Resolution:** the in-app entry point ("Check for Updates") is now
  hidden on hosts where install isn't supported and when the operator
  disables installs. `options::activate_current_selection` no-ops the
  row and the row is skipped at render time. The menu banner that
  surfaces "update available v0.x.y" remains regardless, so users still
  learn about new releases — they just go through whichever channel
  ships their build (Steam page, distro repo, github.com) instead of an
  in-app modal that can't be acted on.
- **Acceptance:** users on macOS / on `UpdaterInstallEnabled = 0` builds
  no longer encounter the AvailableNoInstall dead-end overlay from the
  Options menu, while still seeing release availability on the menu.

### M20. I/O errors lose path/operation context

- **Problem:** Many call sites stringify the raw `io::Error` with no
  surrounding context: `src/engine/updater/download.rs:98-106`
  (`sha256_of_file`), `src/engine/updater/apply_windows.rs:344-346`
  (`io_err`), most extraction helpers. Real-world failures (UAC, AV
  locks, UNC shares, Program Files writes, read-only media) surface to
  the user as `IO("Access is denied. (os error 5)")` with no clue
  which path or step failed.
- **Fix:** Added two helpers in `src/engine/updater/mod.rs`:
  `io_err_at(op, path, err)` and `io_err_op(op, err)`. The
  `Io(format!("{op} '{}': {err}"))` shape is used for all
  filesystem operations that have a single meaningful path; the
  op-only variant covers archive-iteration ops where no single path
  applies (zip header reads, tar entry decode, current_exe). The
  per-file `io_err` helpers in `apply_unix.rs` and `apply_windows.rs`
  were removed.
- **Acceptance:** representative failure modes (locked file, read-only
  install dir, missing parent) produce log lines that name the path
  and the step. Verified by inspection — every refactored call site
  carries a verb (`open`/`read`/`write`/`create`/`create_dir_all`/
  `flush`/`fsync`/`rename`) and, where applicable, the offending path.
- **Resolution:** ✅ Done.



---

## 🟠 Major (added during portable-marker fix landing)

### M21. Apply silently flips non-portable installs to portable mode

- **Problem:** The release packaging scripts
  (`scripts/package-windows-release.ps1:57`, `package-linux-release.sh:64`,
  `package-macos-release.sh:59`, `package-freebsd-release.sh:60`) all
  drop an empty `portable.txt` next to the executable so a freshly
  unzipped release behaves portably by default. `plan_ops` accepts
  every staged regular file, so applying that archive over a
  *non-portable* install would create `portable.txt` next to the
  executable, and `crate::config::dirs` (line 166) would silently flip
  the next launch into portable mode — hiding the user's existing
  AppData/XDG config. The reverse (portable user updating) was already
  fine: `target_existed` is true and we just overwrite empty with
  empty.
- **Fix:** Added `apply_journal::is_portability_marker(rel)` matching a
  top-level `portable.txt` or `portable.ini`. Both `plan_ops`
  implementations (`apply_unix.rs`, `apply_windows.rs`) skip the Op
  when the marker is present in staging *and* the corresponding
  target file does not already exist. Existing markers are still
  overwritten (empty → empty), so portable installs stay portable.
- **Acceptance:** ✅ Done. New unit tests
  `plan_ops_skips_portability_marker_when_target_missing` and
  `plan_ops_replaces_portability_marker_when_target_exists` cover
  both directions on Windows and (cfg-gated) Linux/FreeBSD.

---

## 🔴 Critical (added by 2026-04-30 rubber-duck pass)

### C8. Apply has no allowlist — can overwrite user data/config files

- **Problem:** The planner accepts every staged regular file
  (`apply_unix.rs:202-230`, `apply_windows.rs:218-246`). In portable
  installs `data_dir == exe_dir`
  (`src/config/dirs.rs:197-203`), so the install root *also* contains
  `deadsync.ini`, `save/`, `songs/`, `courses/`, `cache/`,
  `deadsync.log`, the updater's own state cache, and any user-added
  noteskins/assets. A packaging mistake — or a compromised release
  asset — that ships any of those paths would silently overwrite or
  hide user state, with the same shape as the `portable.txt` bug but
  potentially data-loss-grade.
- **Fix:** Add an explicit denylist (and ideally a release manifest of
  app-owned paths) checked in `plan_ops`. At minimum reject any
  staged entry whose relative path is, or is under, any of:
  `deadsync.ini`, `deadsync.log`, `save/`, `songs/`, `courses/`,
  `cache/`, plus the updater's own runtime state files. Prefer a
  release-time `manifest.json` listing every app-owned file, and
  reject anything not on that list, so the policy doesn't drift as
  user-state directory names change.
- **Acceptance:** an archive containing `deadsync.ini` + `save/...`
  alongside the executable applies successfully and leaves the
  pre-existing user files untouched. New unit tests exercise the
  denylist for both POSIX and Windows planners. The release
  packaging scripts get a CI check that verifies they never include
  any denylisted path.

### C9. Env-var release URL & fake-download active in release builds

- **Problem:** `DEADSYNC_UPDATER_RELEASE_URL` (`mod.rs:47-50`) and
  `DEADSYNC_UPDATER_FAKE_DOWNLOAD` (`action.rs:393-397`) are read
  unconditionally. Combined, they let any process that controls the
  launch environment (a malicious launcher script, a compromised
  shortcut, a sibling user account on shared boxes) point the
  checker at arbitrary release JSON and then stage an arbitrary local
  archive. The fake path computes the SHA over the attacker's bytes
  and publishes `Ready` directly — bypassing both the `.sha256`
  sidecar and the GitHub API `digest` cross-check (N2). This is
  materially worse than weak host pinning because the live override
  bypasses GitHub entirely. Subsumes C2 + M6.
- **Fix:** Gate both reads behind
  `cfg(any(test, debug_assertions, feature = "updater-test-overrides"))`.
  Add a release-build integration test that sets both env vars,
  invokes the check + download paths, and asserts they're ignored
  (i.e. the production github.com URL is contacted, or the request is
  refused). Also drop any cached release whose URL host doesn't match
  the canonical one — already partially covered by M18 but worth
  reasserting now that the override is gated out.
- **Acceptance:** in a `cargo build --release` binary, `set
  DEADSYNC_UPDATER_RELEASE_URL=http://attacker/` followed by a manual
  check still hits `api.github.com`. `DEADSYNC_UPDATER_FAKE_DOWNLOAD`
  is a no-op.

---

## 🟠 Major (added by 2026-04-30 rubber-duck pass)

### M22. Live rollback failure deletes the journal anyway ✅ Done

- **Problem:** When a per-op rename failed inside
  `execute_with_rollback`, the function called `rollback()` and then
  returned `Err`. `rollback()` was best-effort and ignored every
  inner error; the caller (`apply_tar_gz` / `apply_zip`) then
  unconditionally removed the journal and staging on any execute
  error. If AV, ransomware-protection, a file lock, or a transient
  FS failure prevented the rollback rename(s), the install was left
  in a mixed old/new state *and* the recovery instructions were
  deleted. This undermined the durable-journal property C5 was built
  on.
- **Fix shipped:**
  - Added `apply_journal::ExecuteFailure { cause, rollback_clean }`.
  - Refactored `execute_with_rollback` on both platforms to return
    `Err(ExecuteFailure)` and propagate `rollback_clean` from the
    new `rollback() -> bool` signature.
  - `rollback()` now logs every restore failure at `warn!` and sets
    the return flag to `false` if any rename failed.
  - On POSIX, after a clean rollback the function fsyncs each unique
    restored parent so the restored entries are durable across power
    loss.
  - The callers preserve the `Applying` journal + staging when
    `rollback_clean == false` and emit a `warn!` pointing at the
    journal path so next-launch `recover()` can retry. The existing
    `recover()` `Applying` arm is already idempotent (presence
    checks for backup/target before each rename), so partial
    rollbacks compose with it without changes.
- **Tests:** added `rollback_reports_dirty_when_restore_rename_fails`
  unit test on both platforms (POSIX-gated for unix); 152 updater
  tests pass.

### M23. Relaunch may exec the renamed-out backup binary ✅ Done

- **Problem:** `apply_archive_and_relaunch` ran apply (which renames
  the running executable to `<exe>.deadsync-bak-<token>` and moves
  the new binary onto the original path) and *then* called
  `std::env::current_exe()` to spawn the relaunch. On Linux
  `/proc/self/exe` resolves to the *running inode*, which after
  apply is the backup path — not the freshly installed target.
  macOS has historically been similar. Effect: the relaunch could
  spawn the old binary out of the backup path; that process then ran
  `apply_journal::recover()`, saw a clean `Applied` journal, and
  deleted the backup it was executing.
- **Fix shipped:** `apply_archive_and_relaunch` now resolves the
  running exe path *before* `reverify_archive` / `apply_for_host`
  and forwards the install-tree path
  `exe_dir.join(original_exe.file_name())` into `relaunch_self`.
  `relaunch_self` no longer calls `current_exe()` — on every
  supported platform it just `Command::new(exe).arg("--restart")
  .spawn()`. The unused `exe_dir()` helper is removed and the
  windows + unix `relaunch_self` bodies (now identical) collapse to
  a single `cfg(any(windows, linux, freebsd))` definition.

### M24. Extracted file contents not fsynced before rename ✅ Done

- **Problem:** C7 made directory entries durable but extraction itself
  never fsynced the staged files. The staging files live in the same
  filesystem as the install (we depend on that for `rename`
  atomicity), so if power was lost between "rename committed" and
  "page cache flushed", the install could end up with a directory
  entry pointing at a file whose bytes hadn't hit stable storage —
  "successfully applied" with zeroed/torn content.
- **Fix shipped:** After `io::copy` of each tar/zip entry,
  `extract_tar_gz` (`apply_unix.rs`) and `extract_archive`
  (`apply_windows.rs`) now call `out.sync_all()` before the handle
  drops, with `super::io_err_at("sync_all", ..)` context. On Linux
  this is `fsync(2)`; on Windows it's `FlushFileBuffers` — both flush
  file body to stable storage. The journal sidecar is already
  fsynced by `write_atomic` (`apply_journal.rs:169`), so the full
  pipeline (file body → parent dir entry → journal) is now durable.

### M25. Planner doesn't reject dir-vs-file type mismatches ✅ Done

- **Problem:** `plan_ops` only checked `target.exists()`. If an
  archive entry's target path currently existed as a *directory*
  (e.g. a future release renames `noteskins/foo.png` to a directory
  `noteskins/foo/`), the rename moved the whole subtree to the
  backup path. `Applied` cleanup later called `remove_file` on that
  backup which fails on a directory, leaving the journal stuck
  forever. Symlinks-at-target were similarly opaque.
- **Fix shipped:** `plan_ops` on both platforms now calls
  `fs::symlink_metadata(&target)` (no symlink-following) instead of
  `target.exists()`. When the entry exists and is not a regular file
  it returns `UpdaterError::Io("type mismatch for '...': existing
  entry is not a regular file")` *before* the journal is written, so
  we never enter the durable phase. NotFound still maps to
  `target_existed = false`; other metadata errors propagate with
  M20 path context.
- **Tests:** added `plan_ops_rejects_directory_at_target` to both
  `apply_unix` and `apply_windows` unit tests; staging a regular
  file at `noteskins/foo.png` while a directory of the same path
  exists now errors at planning time. 153 updater tests pass.

### M26. No read/idle timeout on download body

- **Problem:** `download_agent` deliberately drops the global timeout
  so multi-hundred-MB downloads don't get cut off (`mod.rs:83-89`),
  and `stream_to_file` only polls `should_cancel` between chunks
  (`download.rs:411-417`). A server that accepts the connection and
  then stalls mid-body keeps the worker blocked inside `read()`
  indefinitely; pressing Back bumps the generation but the worker
  never wakes up. Visible symptoms: the overlay disappears (overlay
  state advances on the new generation), but a thread + socket
  leak quietly until process exit. ETA also stays frozen for the
  entire stall, which is a user-facing wart on top of the resource
  leak.
- **Fix:** Configure a per-read or low-speed timeout on
  `download_agent` (e.g. `read_timeout(60s)` or a `<512 B/s` for
  `>30s` low-speed cutoff if the http client supports it). On
  timeout, check the worker's generation: if stale, return
  `UpdaterError::Cancelled` and clean up the `.part` file; otherwise
  surface as a network error and let the user retry. If `ureq`'s
  knobs aren't granular enough, wrap the body reader in a
  timeout-aware adapter.
- **Acceptance:** integration test points the download path at a
  test server that sends headers + 1 KiB then sleeps; the worker
  exits cleanly within the configured timeout, no leaked thread,
  no leftover `.part`.

### M27. Apply success + relaunch failure looks like apply failure

- **Problem:** `apply_archive_and_relaunch` applies first, spawns
  second (`cli.rs:143-147`, `action.rs:665-674`). If apply succeeds
  but spawn fails (sandbox refusal, missing perms, ENOENT on a
  renamed exe path — see M23), the current old process keeps
  running and the worker publishes a generic `Error` phase. The
  user thinks the update failed; in reality the install tree is
  already on the new version, only the running process isn't.
- **Fix:** Distinguish the two outcomes in `apply_archive_and_relaunch`:
  - On apply-fail: keep the existing rollback-capable `Error`.
  - On apply-ok-but-spawn-fail: publish a new
    `ActionPhase::AppliedRestartRequired { info }` that the overlay
    renders as "Update installed; please restart." Strongly consider
    `process::exit(0)` after a short delay so the user can see the
    message, since continuing in the old process against the new
    install tree is risky.
- **Acceptance:** unit test injects a spawn failure, asserts the
  published phase is the new `AppliedRestartRequired` variant and
  that the journal is `Applied` (not removed).

### M28. Tar extractor silently skips symlinks/devices ✅ Done

- **Problem:** `apply_unix::extract_tar_gz` skips non-file/non-dir
  entries (`apply_unix.rs:163-167`). The intent is "release
  tarballs never contain these," which is true today, but if a
  future release artifact accidentally ships a symlink that runtime
  depends on, extraction reports "succeeded" and the install is
  silently incomplete.
- **Fix:** Change the `!entry.is_file() && !entry.is_dir()` branch
  from `continue` to `return Err(UpdaterError::Io("rejected
  non-regular entry '...'"))`. Fail closed: a malformed archive is
  better surfaced as an apply error than as a partial install.
  Apply the same rule to the zip extractor on Windows
  (`apply_windows::extract_archive`), which previously had no
  symlink check at all even though zip entries can carry Unix-mode
  bits indicating a symlink.
- **Acceptance:** existing extraction tests pass; a new test that
  feeds a tar with a symlink entry asserts the apply errors, and
  an analogous test feeds a zip containing a symlink entry. Both
  added.

### M29. Future-version journal recovery is silent ✅ Done

- **Problem:** When `Journal::load` parses a journal whose state is
  unknown (e.g. a downgraded build picking up a journal from a
  newer version), `recover()` leaves it untouched and returns a
  no-op `RecoveryReport` with no warning log
  (`apply_journal.rs:210-216, 449-458`). Safe behavior but
  invisible: a stuck install will never produce any signal in
  `deadsync.log` to triage from.
- **Fix:** When `Journal::load` succeeds but `validate()` reports an
  unknown state / unsupported schema, emit `log::warn!("…journal at
  {path} has unsupported schema; leaving in place for newer
  binary…")` from the call site. Don't touch the file.
- **Acceptance:** unit test that hand-writes a journal with an
  unknown `state` field asserts a warning is logged and the file is
  unchanged after `recover()`.

### M30. Per-chunk progress publication is wasteful ✅ Done

- **Problem:** The download progress closure
  (`action.rs:454-489`) fires every 64 KiB chunk, clones
  `ReleaseInfo` (which contains the full release notes body) and
  `ReleaseAsset`, and takes the `PHASE` write lock. For a 50 MiB
  archive that's ~800 lock-acquire-and-clone passes per download;
  fast (gigabit) connections push this into the thousands per
  second. Not a correctness bug today, but unnecessary churn.
- **Fix:** Throttle the closure: track `last_published: Instant` and
  `last_pct: u32` in the closure state alongside `first_sample`;
  publish only when `now - last_published >= 100ms` *or* the integer
  percent / ETA-bucket changed *or* this is the final byte. Always
  publish the final tick so the UI never sticks at 99 %.
- **Acceptance:** existing overlay/download tests still pass; a new
  test counts how many `Downloading` phases are published for a
  10 MiB simulated download and asserts it's ≤ 30, not ≥ 160.

### M31. Redirect host pinning isn't explicit

- **Problem:** M18 sanitizes cached URLs and N2 cross-checks the
  GitHub API `digest`, but live asset / sidecar requests follow
  whatever redirect chain `ureq` returns without an explicit host
  allowlist (`download.rs:241-256, 324-329`). GitHub release
  downloads do legitimately redirect to GitHub-owned object/CDN
  hosts (`*.githubusercontent.com`, S3-backed signed URLs), so a
  naive "final host must be github.com" rule would break real
  downloads — but the *current* policy is "trust whatever the chain
  resolves to," which is also wrong.
- **Fix:** Decide and document a redirect allowlist that matches
  observed GitHub release behavior (`github.com`,
  `objects.githubusercontent.com`, plus whatever else GitHub
  currently uses). Enforce in a custom redirect handler. Once C1
  ships, signature verification will become the load-bearing
  security check and host pinning can relax to "is HTTPS" — keep
  the allowlist as defense in depth until then.
- **Acceptance:** a unit/integration test stubs a 302 to a
  non-allowlisted host and asserts download fails closed; a 302 to
  an allowlisted GitHub CDN host succeeds.



---

## 💡 Future / Platform Blockers

### Windows portable (today)

- Strong code signing + reputation strategy for the released exe
  (SmartScreen / MOTW behavior of downloaded archives).
- For Program Files installs, disable apply and direct users to the
  installer (the writability probe in
  `src/engine/updater/apply_windows.rs:62-90` is a starting point).
- C3 (cleanup) and M1 (transactional apply) are the biggest gaps for
  this channel.

### Windows installer (MSI / Inno / NSIS — future)

- In-app file replacement breaks installer ownership of repair /
  uninstall and code-signature expectations.
- Installer builds should select `dist-installer` (M9) and either
  delegate to the installer's upgrade mechanism or disable in-app
  apply.

### Microsoft Store / MSIX

- Self-mutation is forbidden / inappropriate.
- `dist-store` (M9) disables checks, banners, and apply, and may show
  "updates are managed by Microsoft Store."

### macOS (future)

- Need a `.app`-bundle-aware apply that preserves code signature,
  notarization ticket, and quarantine handling, plus arch selection
  (Apple Silicon vs Intel).
- Recommendation: integrate Sparkle (or sparkle-style signing /
  appcast) before enabling apply. Until then, "Open release page" only.

### Linux (future)

- Current Unix apply (`src/engine/updater/apply_unix.rs:225-251`) is
  only safe for portable / AppImage-style installs.
- Distro packages, Flatpak, Snap, Steam, and read-only mounts must
  disable apply (`dist-managed` from M9).
- Add install-type detection (e.g. "running from /usr", "running from
  /snap", "running from AppImage") to choose behavior.

### Steam / itch app / Flatpak / Snap

- External tooling owns updates. `dist-managed` disables checks /
  banners / apply.
- Document the recommended build profile for each storefront.

### WSL / emulated environments

- `host_target` is compile-time only
  (`src/engine/updater/mod.rs:221-250`). A Linux binary under WSL may
  try to apply over mounted Windows paths with surprising rename
  semantics.
- Detect WSL / emulated / containerized / read-only environments and
  disable apply (or require explicit confirmation with diagnostics).

---

## Suggested execution order

1. **C2, C3, M6, M7, M8** — small, contained fixes that immediately
   reduce blast radius.
2. **M9** — introduce distribution feature flags so future channels
   have a place to opt out.
3. **C1, M2** — signature verification end-to-end, including re-verify
   before apply.
4. **M1, M10** — transactional apply with cross-process locking.
5. **M3, M4, M5** — cache schema improvements + channel wiring.
6. **N1–N6** — polish.
7. Platform-specific work as new distribution channels are picked up.


---

## 🧪 Integration test plan (formerly the `integration-tests` todo)

Cross-process / fault-injection cases that the journal-level unit tests
can't cover. Each case spawns a child binary built with a
`updater-test-fault-injection` feature that aborts at a named
checkpoint, then the parent inspects the install-tree state and re-runs
recovery via a fresh process.

Required cases:

1. **Kill after extraction, before journal write** — staging dir is
   cleaned up (covers M17), no journal exists, no install mutations.
2. **Kill after `Applying` journal write, before any rename** — recovery
   removes journal + staging, install bit-identical to pre-apply.
3. **Kill after `target -> backup` of op N, before `staged -> target`**
   — recovery rolls back op N (rename `backup -> target`).
4. **Kill mid-`staged -> target` of op N, with backup still present** —
   recovery removes the partial `target` *and* renames `backup ->
   target` (covers C5 on Windows).
5. **Kill after several ops complete** — recovery walks ops in reverse,
   rolls back every backup, install bit-identical.
6. **Kill after all ops complete, before `Applied` journal write** —
   recovery rolls back to old version.
7. **Kill after `Applied` journal write, before relaunch** — next
   startup sees `Applied`, removes backups, leaves new install.
8. **Restart while previous process still holds singleton lock** —
   recovery doesn't run on the loser (covers C4).
9. **Locked backup or target during recovery** — recovery returns a
   partial report and *leaves the journal in place* for retry on the
   next launch.
10. **Corrupt journal** (truncated JSON, wrong version, validate
    failure) — no install mutations, journal removed iff safe.
11. **Power-loss simulation** (where filesystem testing tools allow) —
    asserts the directory entry survives, or weakens the comment to
    match (covers C7).
12. **Concurrent re-download into existing `dest`** — second download
    succeeds without spurious rename error (covers M14).

Harness suggestion: use the existing portable test script
(`scripts/test-updater-portable.ps1`) as the per-case runner, with a
`--fault-checkpoint <name>` flag wired into the
`updater-test-fault-injection` feature.

