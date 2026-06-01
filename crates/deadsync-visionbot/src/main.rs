//! `deadsync-visionbot` — external client that watches the DeadSync window and
//! presses each arrow key at the instant the arrow visually aligns with its
//! receptor, so press precision and early/late drift can be measured against the
//! engine's own audio-clock export.
//!
//! Modes:
//! * `calibrate [--cal PATH]` — write a starting calibration template to edit.
//! * `validate  [--cal PATH]` — capture + detect + predict crossings, printing
//!   per-lane markers but **never pressing** (verify calibration safely).
//! * `run       [--cal PATH] [--out DIR] [--lead MS]` — full loop: capture →
//!   detect → predict → schedule → inject, with a self-log. Press `Esc` to stop.
//! * `inject-test [--cal PATH]` — tap each lane's key once (no vision) to
//!   characterize the actuator path in isolation.
//!
//! All bot timestamps are 100 ns ticks (see [`deadsync_visionbot::capture`]).

#[cfg(not(windows))]
fn main() {
    eprintln!("deadsync-visionbot: the live capture/inject pipeline is Windows-only.");
    eprintln!("On this platform only the `timing-analyze` binary is useful.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    if let Err(e) = win::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use deadsync_visionbot::calibration::{Calibration, DetectParams, LaneCalibration};
    use deadsync_visionbot::capture::{Capturer, OwnedFrame, TICKS_PER_SECOND, find_window_by_title};
    use deadsync_visionbot::detect::{Detection, detect_leading, distance_to_receptor};
    use deadsync_visionbot::geometry::ScrollDir;
    use deadsync_visionbot::inject::{ScanKey, is_foreground, key_down, key_up, resolve_key};
    use deadsync_visionbot::scheduler::{Decision, ms_to_ticks, schedule, ticks_to_ms};
    use deadsync_visionbot::selflog::{PressRecord, SessionHeader, Writer};
    use deadsync_visionbot::timing::{FitGate, Sample, predict_crossing};

    use windows::Win32::System::Performance::QueryPerformanceCounter;
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    /// Parsed command-line options.
    struct Opts {
        mode: String,
        cal_path: PathBuf,
        out_dir: PathBuf,
        lead_override: Option<f64>,
        title_override: Option<String>,
    }

    fn parse_opts() -> Opts {
        let mut args = std::env::args().skip(1);
        let mode = args.next().unwrap_or_else(|| "help".to_string());
        let mut cal_path = PathBuf::from("visionbot_calibration.toml");
        let mut out_dir = PathBuf::from("visionbot_logs");
        let mut lead_override = None;
        let mut title_override = None;
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--cal" => {
                    if let Some(v) = args.next() {
                        cal_path = PathBuf::from(v);
                    }
                }
                "--out" => {
                    if let Some(v) = args.next() {
                        out_dir = PathBuf::from(v);
                    }
                }
                "--lead" => {
                    if let Some(v) = args.next() {
                        lead_override = v.parse().ok();
                    }
                }
                "--title" => title_override = args.next(),
                _ => {}
            }
        }
        Opts {
            mode,
            cal_path,
            out_dir,
            lead_override,
            title_override,
        }
    }

    pub fn run() -> Result<()> {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        let opts = parse_opts();
        match opts.mode.as_str() {
            "calibrate" => cmd_calibrate(&opts),
            "validate" => cmd_loop(&opts, false),
            "run" => cmd_loop(&opts, true),
            "inject-test" => cmd_inject_test(&opts),
            _ => {
                print_help();
                Ok(())
            }
        }
    }

    fn print_help() {
        eprintln!(
            "deadsync-visionbot <calibrate|validate|run|inject-test> [--cal PATH] [--out DIR] [--lead MS] [--title NAME]\n\n\
             calibrate    write a starting calibration template (edit it, then `validate`)\n\
             validate     detect + predict crossings, print markers, NO key presses\n\
             run          full loop: detect + predict + press; self-logs to --out; Esc to stop\n\
             inject-test  tap each lane key once with no vision (actuator characterization)\n"
        );
    }

    fn load_calibration(opts: &Opts) -> Result<Calibration> {
        let mut cal = Calibration::load(&opts.cal_path).map_err(|e| {
            format!(
                "failed to load calibration {}: {e}\n  run `deadsync-visionbot calibrate` first",
                opts.cal_path.display()
            )
        })?;
        if let Some(t) = &opts.title_override {
            cal.window_title = t.clone();
        }
        if let Some(l) = opts.lead_override {
            cal.detect.lead_ms = l;
        }
        Ok(cal)
    }

    fn cmd_calibrate(opts: &Opts) -> Result<()> {
        if opts.cal_path.exists() {
            return Err(format!(
                "{} already exists; refusing to overwrite",
                opts.cal_path.display()
            )
            .into());
        }
        let cal = Calibration::default_dance_single_up();
        cal.save(&opts.cal_path)?;
        println!("wrote starting calibration to {}", opts.cal_path.display());
        println!(
            "Edit receptor/roi fractions to match your resolution & noteskin, then run\n  \
             deadsync-visionbot validate --cal {}",
            opts.cal_path.display()
        );
        Ok(())
    }

    fn esc_pressed() -> bool {
        // High-order bit set = key currently down.
        unsafe { (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 }
    }

    fn raise_thread_priority() {
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
    }

    fn qpc_raw() -> i64 {
        let mut c = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut c);
        }
        c
    }

    /// "Now" in 100 ns ticks. `freq` is [`TICKS_PER_SECOND`]; the conversion uses
    /// the raw QPC counter and the cached raw frequency.
    fn ticks_now(freq: i64, raw_freq: i64) -> i64 {
        let raw = qpc_raw();
        ((raw as i128 * freq as i128) / raw_freq as i128) as i64
    }

    /// Per-lane runtime state.
    struct Lane {
        cal: LaneCalibration,
        key: ScanKey,
        samples: Vec<Sample>,
        /// Ignore new presses until this tick (refractory after a press).
        cooldown_until: i64,
        /// Frame timestamp of the most recent accepted sample (for gap clearing).
        last_push_ts: i64,
    }

    /// A keyup scheduled for a future tick so the loop never blocks on hold time.
    struct PendingUp {
        key: ScanKey,
        at: i64,
    }

    /// An armed keydown to be fired at its exact scheduled tick.
    struct PendingDown {
        lane_dir: u8,
        key: ScanKey,
        emit_at: i64,
        crossing_qpc: i64,
        fit_r2: f64,
        fit_frames: u32,
    }

    fn build_lanes(cal: &Calibration) -> Result<Vec<Lane>> {
        let mut lanes = Vec::new();
        for lc in &cal.lanes {
            let key = resolve_key(&lc.key)
                .ok_or_else(|| format!("unknown key name '{}' in calibration", lc.key))?;
            lanes.push(Lane {
                cal: lc.clone(),
                key,
                samples: Vec::with_capacity(cal.detect.fit_window + 1),
                cooldown_until: i64::MIN,
                last_push_ts: i64::MIN,
            });
        }
        if lanes.is_empty() {
            return Err("calibration has no lanes".into());
        }
        Ok(lanes)
    }

    fn fit_gate(p: &DetectParams, scroll: ScrollDir) -> FitGate {
        FitGate {
            min_samples: 3,
            min_r2: p.min_fit_r2,
            max_residual_pos: 12.0,
            expected_slope_sign: scroll.travel_sign(),
            max_extrapolation_ticks: ms_to_ticks(60.0, TICKS_PER_SECOND),
        }
    }

    /// Shared timing/latency policy resolved from calibration, in 100 ns ticks.
    struct LoopParams {
        freq: i64,
        raw_freq: i64,
        lead_ms: f64,
        keyup_ms: f64,
        max_late_ms: f64,
        /// Arm a keydown once the emit moment is within this horizon; the press is
        /// then fired at its exact scheduled tick (decoupled from frame cadence so
        /// coarse 60 Hz frames cannot systematically miss presses).
        arm_horizon_ticks: i64,
        /// Final short spin before firing an armed keydown, for sub-ms accuracy.
        spin_ticks: i64,
        stale_ticks: i64,
        refractory_ticks: i64,
        /// Clear a lane's sample buffer after this long without an accepted sample.
        clear_gap_ticks: i64,
    }

    fn cmd_loop(opts: &Opts, press: bool) -> Result<()> {
        let cal = load_calibration(opts)?;
        let hwnd = find_window_by_title(&cal.window_title).ok_or_else(|| {
            format!(
                "window '{}' not found — start the game first",
                cal.window_title
            )
        })?;
        let mut cap = Capturer::new(hwnd)?;
        let mut lanes = build_lanes(&cal)?;
        let gate = fit_gate(&cal.detect, cal.scroll);

        // Bot tick base is 100 ns; the scheduler frequency must match that, NOT
        // the raw QPC frequency.
        let freq = TICKS_PER_SECOND;
        let lp = LoopParams {
            freq,
            raw_freq: cap.qpc_freq(),
            lead_ms: cal.detect.lead_ms,
            keyup_ms: cal.detect.keyup_ms,
            max_late_ms: cal.detect.max_late_ms,
            arm_horizon_ticks: ms_to_ticks(25.0, freq),
            spin_ticks: ms_to_ticks(2.0, freq),
            stale_ticks: ms_to_ticks(50.0, freq),
            refractory_ticks: ms_to_ticks(70.0, freq),
            clear_gap_ticks: ms_to_ticks(33.0, freq),
        };

        let mut writer: Option<Writer> = None;
        let mut pending_ups: Vec<PendingUp> = Vec::new();
        let mut pending_downs: Vec<PendingDown> = Vec::new();

        if press {
            raise_thread_priority();
        }
        println!(
            "{} mode on '{}' ({} lanes). Press Esc to stop.",
            if press { "RUN" } else { "VALIDATE" },
            cal.window_title,
            lanes.len()
        );

        loop {
            if esc_pressed() {
                println!("\nEsc — stopping.");
                break;
            }

            let now = cap.now();

            // Fire any keyups whose hold time elapsed (never blocks the loop).
            pending_ups.retain(|pu| {
                if now >= pu.at {
                    key_up(pu.key);
                    false
                } else {
                    true
                }
            });

            // Fire any armed keydowns that are due (with a short final spin for
            // sub-ms accuracy). Decoupled from frame arrival.
            fire_due_downs(
                &mut pending_downs,
                &lp,
                is_foreground(hwnd),
                &mut pending_ups,
                writer.as_mut(),
            );

            let Some(frame) = cap.poll_newest()? else {
                std::thread::sleep(Duration::from_micros(500));
                continue;
            };
            // Drop stale frames so capture stalls never feed the timing fit.
            if now - frame.timestamp > lp.stale_ticks {
                continue;
            }
            if writer.is_none() && press {
                writer = Some(open_selflog(opts, &cal, (frame.width, frame.height), freq)?);
            }

            process_frame(
                &frame,
                &mut lanes,
                cal.scroll,
                &cal.detect,
                &gate,
                &lp,
                press,
                &mut pending_downs,
            );
        }

        // Release any keys still held.
        for pu in &pending_ups {
            key_up(pu.key);
        }
        if let Some(w) = writer.as_mut() {
            w.flush()?;
        }
        Ok(())
    }

    fn open_selflog(opts: &Opts, cal: &Calibration, dims: (u32, u32), freq: i64) -> Result<Writer> {
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let header = SessionHeader {
            bot_version: env!("CARGO_PKG_VERSION").to_string(),
            started_unix_ms: started,
            calibration_id: cal.id.clone(),
            window_title: cal.window_title.clone(),
            frame_width: dims.0,
            frame_height: dims.1,
            qpc_freq: freq,
            lead_ms: cal.detect.lead_ms,
            keyup_ms: cal.detect.keyup_ms,
        };
        let stem = format!("session_{started}");
        Ok(Writer::create(&opts.out_dir, &stem, &header)?)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_frame(
        frame: &OwnedFrame,
        lanes: &mut [Lane],
        scroll: ScrollDir,
        params: &DetectParams,
        gate: &FitGate,
        lp: &LoopParams,
        press: bool,
        pending_downs: &mut Vec<PendingDown>,
    ) {
        let view = frame.view();
        let w = frame.width;
        let h = frame.height;
        let ts = frame.timestamp;

        for (idx, lane) in lanes.iter_mut().enumerate() {
            let roi = lane.cal.roi_px(w, h);
            let receptor = lane.cal.receptor_coord(w, h, scroll);

            let det: Option<Detection> = detect_leading(&view, roi, scroll, params.luma_threshold);
            let Some(det) = det else {
                lane.samples.clear();
                continue;
            };
            if det.confidence < params.min_confidence {
                // Drop a stale buffer if confidence has been lost for a while so a
                // gap between arrows cannot poison the next arrow's fit.
                if ts - lane.last_push_ts > lp.clear_gap_ticks {
                    lane.samples.clear();
                }
                continue;
            }

            let dist = distance_to_receptor(det.centroid, receptor, scroll);
            lane.samples.push(Sample {
                pos: det.centroid,
                qpc: ts,
            });
            lane.last_push_ts = ts;
            if lane.samples.len() > params.fit_window {
                let excess = lane.samples.len() - params.fit_window;
                lane.samples.drain(0..excess);
            }
            // Only predict for an approaching arrow outside the refractory window.
            if ts < lane.cooldown_until || dist < 0.0 {
                continue;
            }

            let pred = match predict_crossing(&lane.samples, receptor, gate) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let now = ticks_now(lp.freq, lp.raw_freq);
            let emit_at = match schedule(pred.crossing_qpc, now, lp.freq, lp.lead_ms, lp.max_late_ms)
            {
                Decision::SkipLate { .. } => continue,
                Decision::Emit { at_qpc } => at_qpc,
            };
            // Arm only once the emit moment is within the horizon; until then keep
            // refining the fit with more frames.
            if emit_at - now > lp.arm_horizon_ticks {
                continue;
            }

            if !press {
                let lead_to_cross = ticks_to_ms(pred.crossing_qpc - now, lp.freq);
                println!(
                    "[validate] lane {idx} (dir {}) crossing in {:.1} ms  r2={:.3} n={}",
                    lane.cal.direction_code, lead_to_cross, pred.r2, pred.n
                );
            } else {
                pending_downs.push(PendingDown {
                    lane_dir: lane.cal.direction_code,
                    key: lane.key,
                    emit_at,
                    crossing_qpc: pred.crossing_qpc,
                    fit_r2: pred.r2,
                    fit_frames: pred.n as u32,
                });
            }
            // Refractory + buffer reset so this arrow arms exactly once.
            lane.cooldown_until = pred.crossing_qpc + lp.refractory_ticks;
            lane.samples.clear();
        }
    }

    /// Fire all armed keydowns whose scheduled tick has arrived. The nearest
    /// upcoming press within [`LoopParams::spin_ticks`] is spin-waited for sub-ms
    /// accuracy; presses further out are left for a later iteration.
    fn fire_due_downs(
        pending_downs: &mut Vec<PendingDown>,
        lp: &LoopParams,
        foreground: bool,
        pending_ups: &mut Vec<PendingUp>,
        mut writer: Option<&mut Writer>,
    ) {
        loop {
            let now = ticks_now(lp.freq, lp.raw_freq);
            // Find the soonest pending keydown.
            let Some((i, emit_at)) = pending_downs
                .iter()
                .enumerate()
                .min_by_key(|(_, d)| d.emit_at)
                .map(|(i, d)| (i, d.emit_at))
            else {
                break;
            };
            if emit_at - now > lp.spin_ticks {
                break; // nothing imminent; revisit next loop iteration
            }
            // Short spin to the exact tick.
            while ticks_now(lp.freq, lp.raw_freq) < emit_at {
                std::hint::spin_loop();
            }
            let down = pending_downs.swap_remove(i);

            if !foreground {
                continue; // drop the press but keep draining the queue
            }
            let call_qpc = ticks_now(lp.freq, lp.raw_freq);
            key_down(down.key);
            let return_qpc = ticks_now(lp.freq, lp.raw_freq);
            pending_ups.push(PendingUp {
                key: down.key,
                at: return_qpc + ms_to_ticks(lp.keyup_ms, lp.freq),
            });

            if let Some(w) = writer.as_deref_mut() {
                let rec = PressRecord {
                    lane: down.lane_dir,
                    predicted_crossing_qpc: down.crossing_qpc,
                    scheduled_emit_qpc: down.emit_at,
                    sendinput_call_qpc: call_qpc,
                    sendinput_return_qpc: return_qpc,
                    keyup_qpc: return_qpc + ms_to_ticks(lp.keyup_ms, lp.freq),
                    foreground,
                    fit_r2: down.fit_r2,
                    fit_frames: down.fit_frames,
                    scheduler_error_ticks: call_qpc - down.crossing_qpc,
                };
                let _ = w.write_press(&rec);
            }
            log::debug!(
                "press dir {} sched_err={:.2}ms",
                down.lane_dir,
                ticks_to_ms(call_qpc - down.crossing_qpc, lp.freq)
            );
        }
    }

    fn cmd_inject_test(opts: &Opts) -> Result<()> {
        let cal = load_calibration(opts)?;
        let hwnd = find_window_by_title(&cal.window_title).ok_or_else(|| {
            format!(
                "window '{}' not found — start the game first",
                cal.window_title
            )
        })?;
        println!(
            "inject-test on '{}': tapping each lane key once. Focus the game now; Esc aborts.",
            cal.window_title
        );
        std::thread::sleep(Duration::from_secs(2));
        let freq = TICKS_PER_SECOND;
        let raw_freq = deadsync_visionbot::capture::qpc_frequency();
        for lane in &cal.lanes {
            if esc_pressed() {
                break;
            }
            let Some(key) = resolve_key(&lane.key) else {
                eprintln!("skip unknown key '{}'", lane.key);
                continue;
            };
            if !is_foreground(hwnd) {
                eprintln!("game not foreground; skipping {}", lane.key);
                continue;
            }
            let t0 = ticks_now(freq, raw_freq);
            key_down(key);
            std::thread::sleep(Duration::from_millis(cal.detect.keyup_ms as u64));
            key_up(key);
            let t1 = ticks_now(freq, raw_freq);
            println!(
                "tapped {} (dir {}): held ~{:.1} ms",
                lane.key,
                lane.direction_code,
                ticks_to_ms(t1 - t0, freq)
            );
            std::thread::sleep(Duration::from_millis(300));
        }
        Ok(())
    }
}
