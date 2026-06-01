# deadsync-visionbot

An external **precision test harness** for DeadSync. It plays the game by
screen-reading the notefield — detecting when each arrow reaches its receptor
and injecting the matching key press — so you can measure how precise the
presses are and whether they drift early/late across a song.

It pairs with the engine's opt-in **timing export** (the authoritative
audio-clock judgment of every note) and a **`timing-analyze`** tool that turns
those exports into precision and drift statistics.

> Platform: the live capture + key-injection pipeline is **Windows-only**
> (Windows.Graphics.Capture + SendInput). On other platforms only the
> `timing-analyze` binary builds and runs.

---

## How it fits together

```
                    presses keys
  ┌───────────────┐  (SendInput)  ┌───────────────┐
  │ deadsync-     │ ────────────► │   DeadSync    │
  │ visionbot     │ ◄──────────── │   (the game)  │
  │ (this crate)  │  screen frames└──────┬────────┘
  └──────┬────────┘  (WGC capture)       │ judges every note vs. audio clock
         │                               ▼
         │ self-log (secondary)   timing export JSON  ◄── GROUND TRUTH
         │ predicted/scheduled/          │
         │ actual press ticks            ▼
         └──────────────────────►  timing-analyze
                                   precision + drift report
```

* **Ground truth for precision/drift is the engine timing export**, not the
  bot. The export is the game's own per-note offset against its audio clock.
* The bot's **self-log** is secondary — it records what the bot *intended*
  vs. *did* (predicted crossing, scheduled emit, actual SendInput tick) so you
  can separate apparatus error from engine timing.

---

## Build

```powershell
# from the repo root
cargo build -p deadsync-visionbot --release
```

Two binaries are produced:

| binary                 | purpose                                            |
| ---------------------- | -------------------------------------------------- |
| `deadsync-visionbot`   | the live capture/detect/press orchestrator (Win)   |
| `timing-analyze`       | offline analysis of engine timing exports          |

---

## Quick start

```powershell
# 1. Start DeadSync.

# 2. Write a starting calibration template, then edit it (see below).
deadsync-visionbot calibrate --cal visionbot_calibration.toml

# 3. Dry-run: detect arrows and predict crossings, but DON'T press.
#    Watch the printed "crossing in N ms / r2 / n" markers line up with play.
deadsync-visionbot validate --cal visionbot_calibration.toml

# 4. Real run: press keys, write a self-log. Focus the game; Esc stops.
deadsync-visionbot run --cal visionbot_calibration.toml --out visionbot_logs

# 5. With DEADSYNC_TIMING_EXPORT=1 set on the game (step 0 below), each play
#    drops a JSON export. Analyze it:
timing-analyze "<data_dir>/save/timing_exports"
```

---

## 0. Enable the engine timing export (ground truth)

The export is **opt-in** and a no-op unless enabled, so set these on the
**game** process before playing:

| env var                       | effect                                                            |
| ----------------------------- | ----------------------------------------------------------------- |
| `DEADSYNC_TIMING_EXPORT`      | truthy → write a per-note JSON export on each evaluation screen.   |
| `DEADSYNC_TIMING_EXPORT_DIR`  | output dir (default `<data_dir>/save/timing_exports`).             |

```powershell
$env:DEADSYNC_TIMING_EXPORT = "1"
# optional:
$env:DEADSYNC_TIMING_EXPORT_DIR = "D:\timing_exports"
# then launch the game from this shell
```

Each export is one play: run metadata (song, chart, rate, grade,
`autoplay_or_replay`), a summary (mean/abs/stddev/max offset, per-column and
per-foot buckets), and the full list of note points (song time + signed
offset_ms). The JSON schema *is* the contract between game and analyzer.

---

## 1. `calibrate` — make a calibration file

```powershell
deadsync-visionbot calibrate --cal visionbot_calibration.toml
```

Writes a starting template (4-panel, up-scroll, P1 single) you then **edit to
match your actual window, resolution, and noteskin**. Calibration is stored as
resolution-relative fractions so it survives window resizes.

Key fields:

```toml
id            = "..."        # recorded into the self-log
window_title  = "DeadSync"   # window to capture (exact title match)
reference_width  = 1920      # resolution the fractions were authored at (info)
reference_height = 1080
scroll        = "Up"         # notefield scroll direction

[detect]
luma_threshold = 120.0   # mean luma (0..255) above which a scanline = arrow
min_confidence = 0.35    # min detector confidence to feed the timing fit
min_fit_r2     = 0.9     # min R² to accept a crossing prediction
fit_window     = 6       # recent centroid samples kept per lane for the fit
lead_ms        = 0.0     # latency lead L subtracted from predicted crossing
keyup_ms       = 16.0    # keyup delay after keydown
max_late_ms    = 8.0     # skip a press already this many ms overdue

[[lanes]]                # one block per playable lane
direction_code = 1       # 1=L 2=D 3=U 4=R  (joins to the engine export)
key            = "Left"  # virtual-key name to inject
receptor       = { x = 0.18, y = 0.12 }            # receptor center (fractions)
roi            = { x = .., y = .., w = .., h = .. } # strip the arrow approaches through
```

Tuning order at bring-up:
1. Get `receptor` and `roi` right so `validate` reports crossings that line up
   with what you see.
2. Raise/lower `luma_threshold` / `min_confidence` until detection is stable.
3. Tune **`lead_ms` (L)** so presses land on time (see below).

---

## 2. `validate` — detect & predict, no presses

```powershell
deadsync-visionbot validate --cal visionbot_calibration.toml
```

Captures frames, runs detection + the Theil–Sen crossing fit, and prints a
marker per armed lane:

```
[validate] lane 0 (dir 1) crossing in 12.4 ms  r2=0.987 n=6
```

No keys are pressed. Use this to confirm ROIs, thresholds, and that predicted
crossings track the music before you let it press anything.

---

## 3. `run` — play and self-log

```powershell
deadsync-visionbot run --cal visionbot_calibration.toml --out visionbot_logs --lead 35
```

Full loop: capture → detect → predict → schedule → press. It uses an
**arm-within-horizon, fire-at-scheduled-tick** model: a keydown is armed once
its emit moment is within ~25 ms, then fired at the exact tick with a short
spin for sub-ms accuracy — decoupling press timing from the ~16 ms frame
cadence. The game must be the **foreground** window for presses to land; press
**Esc** to stop.

A self-log is written to `--out` (default `visionbot_logs`) as
`session_<unix_ms>.jsonl` with a header (bot version, calibration id, frame
size, tick frequency, lead/keyup) followed by one `PressRecord` per press
(lane, predicted crossing tick, scheduled emit tick, SendInput call/return
ticks).

Flags:

| flag           | meaning                                              |
| -------------- | ---------------------------------------------------- |
| `--cal PATH`   | calibration file (default `visionbot_calibration.toml`) |
| `--out DIR`    | self-log output dir (default `visionbot_logs`)       |
| `--lead MS`    | override `detect.lead_ms` (latency lead `L`)         |
| `--title NAME` | override the window title to capture                 |

### Calibrating the lead `L`

`L` (`lead_ms`) compensates for total actuation latency — detection age,
scheduling, SendInput, USB/HID, and the game's input→audio path. It's a
**constant bias**: a wrong `L` shifts *all* presses early or late but does
**not** create drift. Tune it by playing, reading the engine export's mean
offset, and nudging `L` to drive the mean toward 0.

---

## 4. `inject-test` — actuator characterization

```powershell
deadsync-visionbot inject-test --cal visionbot_calibration.toml
```

Taps each lane's key once with **no vision** — verifies key mapping, foreground
guard, and SendInput plumbing independent of detection.

---

## 5. `timing-analyze` — precision & drift report

```powershell
timing-analyze "<data_dir>/save/timing_exports"      # a directory, or
timing-analyze play1.json play2.json                 # specific files
```

Reads engine timing exports and reports, per lane and overall:

* offset stats (mean, mean-abs, stddev, max-abs),
* a histogram of offsets,
* **drift = slope of offset vs. song time** via linear regression.

Why slope? Constant biases (your `lead_ms`, refractory, keyup, any fixed
latency `L`) shift the whole offset distribution but leave the slope at ~0.
Only **time-varying** effects (clock drift, gradual desync) tilt the line.
That makes the drift slope independent of the unknown constant lead — it
answers "did the presses stay aligned *across* the song?" directly.

---

## Validating the measurement chain (control run)

Before trusting any vision numbers, prove the export + analyzer chain with a
run the bot didn't drive: play with **autoplay/replay** (the engine presses on
audio time). That export is flagged `autoplay_or_replay = true` and should show
**mean offset ≈ 0 and drift ≈ 0**. If it doesn't, the problem is in the
export/analysis chain, not the bot. Once the control run is clean, differences
in a bot-driven run are attributable to the bot/latency.

---

## Scope & limitations (MVP)

* Targets **single-play, constant up-scroll, fixed noteskin and resolution**.
  Other scroll mods, mid-song speed changes, or layout changes need
  recalibration / extension.
* Capture and detection are **window/noteskin specific** — expect to spend the
  first session in `calibrate` → `validate` before a useful `run`.
* The bot reads pixels, so anything that occludes the receptors (combo splashes,
  judgment graphics, mines, heavy mods) can perturb detection; keep ROIs on the
  clean approach strip.
* `L` is assumed constant within a run. Wildly variable system latency widens
  the offset distribution (stddev) even when the mean and slope look fine.

---

## Internals (for hacking)

| module              | responsibility                                                        |
| ------------------- | --------------------------------------------------------------------- |
| `capture`           | WGC per-window frames; one **100 ns tick** time base (QPC rescaled).  |
| `detect`            | per-lane ROI luma profiling → leading-arrow centroid + confidence.    |
| `timing`            | Theil–Sen fit of (centroid, frame tick) → predicted crossing + R².    |
| `scheduler`         | lead-compensated emit decision (`Emit` / `SkipLate`).                 |
| `inject`            | SendInput scancode keydown/keyup with a foreground guard.             |
| `selflog`           | per-press secondary record writer.                                    |
| `engine_export`     | analyzer-side deserialize of the engine export JSON.                  |
| `drift`             | linear regression for the offset-vs-time slope.                       |
| `main`              | the `calibrate`/`validate`/`run`/`inject-test` orchestrator.          |
| `bin/timing_analyze`| the offline analysis CLI.                                             |

All bot timestamps share a single 100 ns tick base so frame timestamps,
scheduling, and press records are directly comparable.
