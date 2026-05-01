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
| M8  | Don't offer in-app install on platforms where apply is unsupported | 🟠 | ✅ Done        | `apply_supported_for_host()` mirrors the cli cfg gate; `classify_check_result` short-circuits Available → `AvailableNoInstall { info }` on macOS / non-`self-update` builds; overlay shows release tag + `html_url` with Dismiss only — no Download button. |
| M9  | Make in-app install opt-out for managed distributions         | 🟠       | ✅ Done        | New `[Options] UpdaterInstallEnabled` (default `1`); when `0`, `classify_check_result` routes Available → `AvailableNoInstall` and `request_download` refuses, so banner / Check For Updates still surface releases but the Download button never appears. Packagers (Steam / distro / MSIX) ship the ini with `0`. The `self-update` cargo feature remains for builds that want the apply code stripped entirely. |
| M10 | Add an inter-process updater lock                             | 🟠       | ✅ Done        | `engine::single_instance` (Windows named mutex / Unix `flock`); second instance exits with code 1; `--restart` retries 3 s. |
| M11 | Reconcile `REQUEST_TIMEOUT` with the shared HTTP agent        | 🟠       | ✅ Done        | Removed unused constant; updater now uses dedicated `check_agent` (10 s global) and `download_agent` (no global, 15 s connect / 10 s resolve) so multi-MB archives aren't capped at the score-submit timeout. |
| N1  | ETag bookkeeping                                              | 🟡       | ✅ Done        | `apply_fresh_to_cache` lifted out of `run_check_once` and now overwrites `etag` unconditionally so a Fresh-without-ETag drops the previous value instead of carrying it into the next `If-None-Match`. Channel-scoping deferred: M5 removed `UpdateChannel`, so there's only one release URL today. |
| N2  | Verify GitHub's API `digest` field too                        | 🟡       | ✅ Done        | New `cross_check_api_digest` helper compares `assets[].digest` (e.g. `sha256:…`) against the parsed `.sha256` sidecar before downloading; mismatch fails closed via `ChecksumMismatch`, unsupported algorithms log-and-skip, missing field is no-op. Wired into `action::run_download`. |
| N3  | Add cancellation during long checks/downloads                 | 🟡       | ✅ Done        | New `action::request_cancel()` + `cancel_requested()` flag, polled by check / sidecar / download / fake-download workers; `download_to_file` takes a `should_cancel` callback that fires before/between chunks and returns `UpdaterError::Cancelled` (partial file is removed). Overlay binds Back during Checking/Downloading to cancel; Applying remains uncancellable. |
| N4  | Stage downloads to `*.part`, then atomically rename           | 🟡       | ✅ Done        | `download_to_file` now writes to `<dest>.part`, fsyncs after the final flush, and renames onto `dest` only after sha256 verifies. Crash / cancel / mismatch leaves no file at the canonical name; any pre-existing `dest` is preserved. Stale `.part` from a previous run is removed before staging. |
| N5  | Audit unused i18n keys                                        | 🟡       | ⏳ Not started |                                                                                   |
| N6  | Refresh stale comments                                        | 🟡       | ⏳ Not started |                                                                                   |

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
  ship. The existing `self-update` feature is retained as a layered,
  stricter knob for environments that need the apply code stripped
  from the binary entirely (Microsoft Store review, etc.).
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

- `BodyAvailable`, `BodyDownloading`, `BodyReady`, `BodyApplyHint` look
  unused in the current overlay. Either wire them in or drop them.

### N6. Refresh stale comments

- `src/engine/updater/state.rs:177-178, 208-210` still reference Daily
  mode / config-driven frequency, which no longer exist.

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
