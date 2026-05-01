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
| M4  | Use `last_checked_at` to throttle startup checks              | 🟠       | ⏳ Not started | `UpdateCheckMode` setting removed; always-on-startup is the only mode now, but no throttle yet. |
| M5  | Wire up `UpdateChannel::Prerelease` or remove the choice      | 🟠       | ⏳ Not started |                                                                                   |
| M6  | Gate `DEADSYNC_UPDATER_RELEASE_URL` to dev/test builds        | 🟠       | ⏳ Not started |                                                                                   |
| M7  | Thread `ApplyOutcome.staging_dir` into `relaunch_self`        | 🟠       | ✅ Done        | Resolved by removal: relaunch no longer passes `--cleanup-old <staging>`; journal at install root is the source of truth. |
| M8  | Don't offer in-app install on platforms where apply is unsupported | 🟠 | ⏳ Not started |                                                                                   |
| M9  | Make `self-update` opt-in per distribution                    | 🟠       | ⏳ Not started |                                                                                   |
| M10 | Add an inter-process updater lock                             | 🟠       | ✅ Done        | `engine::single_instance` (Windows named mutex / Unix `flock`); second instance exits with code 1; `--restart` retries 3 s. |
| M11 | Reconcile `REQUEST_TIMEOUT` with the shared HTTP agent        | 🟠       | ✅ Done        | Removed unused constant; updater now uses dedicated `check_agent` (10 s global) and `download_agent` (no global, 15 s connect / 10 s resolve) so multi-MB archives aren't capped at the score-submit timeout. |
| N1  | ETag bookkeeping                                              | 🟡       | ⏳ Not started |                                                                                   |
| N2  | Verify GitHub's API `digest` field too                        | 🟡       | ⏳ Not started |                                                                                   |
| N3  | Add cancellation during long checks/downloads                 | 🟡       | ⏳ Not started |                                                                                   |
| N4  | Stage downloads to `*.part`, then atomically rename           | 🟡       | ⏳ Not started |                                                                                   |
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

- **Problem:** `decide` only inspects the env opt-out
  (`src/engine/updater/state.rs:122-128`); every launch contacts
  GitHub. Comments still imply throttling exists.
- **Fix:** add a check interval (e.g. 24 h), respect it on startup,
  bypass it for manual checks. Treat future timestamps (wrong clock)
  conservatively.

### M5. Wire up `UpdateChannel::Prerelease` or remove the choice

- **Problem:** `UpdateChannel` exposes `Stable` and `Prerelease`
  (`src/config/updater.rs:6-10`) and is loaded from config
  (`src/config/load/options.rs:498-503`), but release fetching always
  hits `/releases/latest`
  (`src/engine/updater/mod.rs:33-35, 293-299`).
- **Fix:** either remove the prerelease setting until implemented, or
  switch to the releases list endpoint with explicit prerelease
  filtering when the user opts in.

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

### M9. Make `self-update` opt-in per distribution

- **Problem:** `self-update` is in default features
  (`Cargo.toml:36-43`). Risky for Steam/MSIX/distro/Flatpak/Snap builds
  where the host owns updates.
- **Fix:** introduce a build-time distribution mode, e.g. cargo
  features `dist-portable`, `dist-installer`, `dist-store`,
  `dist-managed`. Only `dist-portable` enables the apply path; managed
  distributions disable checks/banners/apply and may show a
  channel-specific message.

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

- On a fresh response without an ETag, `next.etag` retains the previous
  one (`src/engine/updater/state.rs:184-194`). Set it to `None`
  explicitly, or scope ETags by release URL/channel to avoid leakage.

### N2. Verify GitHub's API `digest` field too

- `ReleaseAsset.digest` is captured (`src/engine/updater/mod.rs:60-69`)
  and shown in the UI
  (`src/screens/components/shared/update_overlay.rs:302-307`) but is
  not used for verification. If present, fail closed on any mismatch
  between API digest, sidecar, and downloaded bytes.

### N3. Add cancellation during long checks/downloads

- The overlay intentionally consumes input during checking/downloading
  (`src/screens/components/shared/update_overlay.rs:423-425`). Add a
  cancel binding for downloads and a "Later" escape during checking
  where safe.

### N4. Stage downloads to `*.part`, then atomically rename

- `download_to_file` writes directly to the final cache path
  (`src/engine/updater/download.rs:187-227`). Crash/power loss leaves
  partial files behind. Write to `*.part`, fsync, rename when
  verified.

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
