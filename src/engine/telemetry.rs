//! Live game-state telemetry.
//!
//! Provides a generic, integration-agnostic mechanism for external tools
//! (stream overlays, companion apps, bots, dashboards, …) to observe the
//! current game state in real time.
//!
//! The module owns a single background publisher thread. Game code calls
//! [`publish`] with partial [`Update`]s; the publisher merges them into a
//! cumulative [`GameStateSnapshot`] and fans the result out to whichever
//! backends are enabled in `deadsync.ini`:
//!
//! * **File backend** — atomic write of `state.json` plus a flat
//!   `nowplaying.txt` under `<data dir>/telemetry/`.
//! * **WebSocket backend** — broadcasts each snapshot as JSON to clients
//!   connected to `ws://127.0.0.1:<port>`.
//!
//! The snapshot schema is intentionally generic (StepMania-style field
//! naming) and is **not** OBS-specific: OBS Browser Source is one supported
//! consumer, but anything that can read a file or open a WebSocket will
//! work.

use log::{debug, info, warn};
use serde::Serialize;
use std::fs;
use std::io::{ErrorKind, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tungstenite::WebSocket;
use tungstenite::protocol::Message;

use crate::config::dirs::AppDirs;

/// Bumped whenever the wire format breaks compatibility. Consumers should
/// check this and surface a friendly error if it doesn't match what they
/// were written against.
pub const SCHEMA_VERSION: u32 = 1;

/// Channel capacity. Updates are tiny (an enum with a few owned strings),
/// so a generous buffer is fine. If the publisher thread can't keep up the
/// game thread silently drops the *oldest* update by best-effort `try_send`.
const CHANNEL_CAPACITY: usize = 256;

/// How often the WebSocket accept loop polls for new connections. The
/// listener is in non-blocking mode so the thread can also forward
/// snapshots without getting stuck waiting on `accept()`.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Default settle window for `Song` updates. Wheel scrolling fires a focus
/// change for every song the cursor passes; debouncing avoids broadcasting
/// dozens of half-second-old snapshots and only sends the song the user
/// actually lands on.
const DEFAULT_SONG_DEBOUNCE: Duration = Duration::from_millis(150);

/// Bind address. Telemetry is local-only by design — never bind to 0.0.0.0.
const BIND_ADDR: &str = "127.0.0.1";

/* ---------------- Public configuration ---------------- */

#[derive(Clone, Debug)]
pub struct TelemetryConfig {
    /// Master switch. When false, [`start`] is a no-op and [`publish`]
    /// returns immediately without queueing.
    pub enabled: bool,
    /// Write `state.json` and `nowplaying.txt` under `telemetry/` in the
    /// data directory.
    pub write_state_file: bool,
    /// `0` disables the WebSocket backend; any other value binds
    /// `127.0.0.1:<port>`.
    pub websocket_port: u16,
    /// Settle window for `Song` updates. `0` disables debouncing entirely
    /// and emits every focus change immediately.
    pub song_debounce: Duration,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            write_state_file: true,
            websocket_port: 0,
            song_debounce: DEFAULT_SONG_DEBOUNCE,
        }
    }
}

/* ---------------- Snapshot schema ---------------- */

/// Cumulative game-state snapshot sent to clients. All fields are optional
/// so that screens which don't have a particular piece of context (e.g.
/// the title menu has no song or chart) can leave them `None`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct GameStateSnapshot {
    pub schema_version: u32,
    /// Wall-clock milliseconds since the Unix epoch when this snapshot was
    /// last updated.
    pub timestamp_ms: u64,
    /// Stable screen name (matches `Screen::current_screen_file_name`),
    /// e.g. `"ScreenSelectMusic"` or `"ScreenGameplay"`.
    pub screen: String,
    pub song: Option<SongInfo>,
    pub chart: Option<ChartInfo>,
    /// Per-side player state. `None` for unused player slots.
    pub players: Vec<Option<PlayerState>>,
    pub music_rate: Option<f32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SongInfo {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub pack: String,
    pub display_bpm: String,
    pub music_length_seconds: f32,
    pub banner_path: Option<String>,
    pub background_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChartInfo {
    /// Stable chart identifier (the simfile's short hash). Matches what
    /// scores, leaderboards, and GrooveStats use as the chart key.
    pub id: String,
    pub difficulty: String,
    pub meter: u32,
    pub step_artist: String,
    pub stepstype: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PlayerState {
    pub profile_name: String,
    pub score_percent: f64,
    pub ex_score_percent: f64,
    pub hard_ex_score_percent: f64,
    pub grade: String,
    pub disqualified: bool,
    pub judgments: JudgmentCounts,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct JudgmentCounts {
    pub w0: u32,
    pub w1: u32,
    pub w2: u32,
    pub w3: u32,
    pub w4: u32,
    pub w5: u32,
    pub miss: u32,
}

/* ---------------- Partial-update API ---------------- */

/// Partial update queued from the game thread. The publisher thread
/// merges these into the running [`GameStateSnapshot`] before emitting.
#[derive(Clone, Debug)]
pub enum Update {
    /// Screen transition. Always emit at least this on every transition.
    /// Takes the typed `Screen` so callers can't accidentally publish an
    /// unknown name; the wire format uses
    /// [`Screen::current_screen_file_name`].
    Screen(crate::screens::Screen),
    /// Selected song + chart. Pass `None` to clear (e.g. on screens that
    /// have no song context). Music rate, if known, is forwarded too.
    Song {
        song: Option<SongInfo>,
        chart: Option<ChartInfo>,
        music_rate: Option<f32>,
    },
    /// Per-player state for one slot (0-based). Pass `None` to clear that
    /// slot.
    Player { side: usize, state: Option<PlayerState> },
    /// Wipe all per-player state (e.g. when leaving evaluation).
    ClearPlayers,
}

/* ---------------- Convenience builders ---------------- */

/// Emit a snapshot derived from a [`StageSummary`]. Used by the evaluation
/// screen so external clients see the full final result of a play.
pub fn publish_stage_summary(stage: &crate::game::stage_stats::StageSummary) {
    let song = song_info_from(&stage.song);
    let chart = stage
        .players
        .iter()
        .find_map(|p| p.as_ref())
        .map(|p| chart_info_from(&p.chart));
    publish(Update::Song {
        song: Some(song),
        chart,
        music_rate: Some(stage.music_rate),
    });
    for (idx, slot) in stage.players.iter().enumerate() {
        let state = slot.as_ref().map(player_state_from);
        publish(Update::Player { side: idx, state });
    }
}

/// Emit a snapshot for a song/chart focus change (e.g. on the music wheel).
///
/// Internally deduplicates: repeated calls with the same `(song_path,
/// chart_index)` are dropped so callers can invoke this every frame
/// without flooding the publisher.
pub fn publish_song_focus(
    song: Option<&crate::game::song::SongData>,
    chart: Option<&crate::game::chart::ChartData>,
    music_rate: Option<f32>,
) {
    let key = focus_key(song, chart);
    {
        let mut last = LAST_FOCUS_KEY.lock().unwrap();
        if last.as_deref() == Some(key.as_str()) {
            return;
        }
        *last = Some(key);
    }
    publish(Update::Song {
        song: song.map(song_info_from),
        chart: chart.map(chart_info_from),
        music_rate,
    });
}

fn focus_key(
    song: Option<&crate::game::song::SongData>,
    chart: Option<&crate::game::chart::ChartData>,
) -> String {
    let s = song
        .map(|s| s.simfile_path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let c = chart
        .map(|c| format!("{}|{}|{}", c.chart_type, c.difficulty, c.meter))
        .unwrap_or_default();
    format!("{s}#{c}")
}

static LAST_FOCUS_KEY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn song_info_from(song: &crate::game::song::SongData) -> SongInfo {
    SongInfo {
        title: song.title.clone(),
        subtitle: song.subtitle.clone(),
        artist: song.artist.clone(),
        pack: pack_name_from_path(&song.simfile_path),
        display_bpm: song.display_bpm.clone(),
        music_length_seconds: song.music_length_seconds,
        banner_path: song
            .banner_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        background_path: song
            .background_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
    }
}

fn chart_info_from(chart: &crate::game::chart::ChartData) -> ChartInfo {
    ChartInfo {
        id: chart.short_hash.clone(),
        difficulty: chart.difficulty.clone(),
        meter: chart.meter,
        step_artist: chart.step_artist.clone(),
        stepstype: chart.chart_type.clone(),
    }
}

fn player_state_from(p: &crate::game::stage_stats::PlayerStageSummary) -> PlayerState {
    PlayerState {
        profile_name: p.profile_name.clone(),
        score_percent: p.score_percent,
        ex_score_percent: p.ex_score_percent,
        hard_ex_score_percent: p.hard_ex_score_percent,
        grade: grade_name(p.grade),
        disqualified: p.disqualified,
        judgments: JudgmentCounts {
            w0: p.window_counts.w0,
            w1: p.window_counts.w1,
            w2: p.window_counts.w2,
            w3: p.window_counts.w3,
            w4: p.window_counts.w4,
            w5: p.window_counts.w5,
            miss: p.window_counts.miss,
        },
    }
}

fn grade_name(grade: crate::game::scores::Grade) -> String {
    use crate::game::scores::Grade;
    match grade {
        Grade::Quint => "Quint",
        Grade::Tier01 => "Tier01",
        Grade::Tier02 => "Tier02",
        Grade::Tier03 => "Tier03",
        Grade::Tier04 => "Tier04",
        Grade::Tier05 => "Tier05",
        Grade::Tier06 => "Tier06",
        Grade::Tier07 => "Tier07",
        Grade::Tier08 => "Tier08",
        Grade::Tier09 => "Tier09",
        Grade::Tier10 => "Tier10",
        Grade::Tier11 => "Tier11",
        Grade::Tier12 => "Tier12",
        Grade::Tier13 => "Tier13",
        Grade::Tier14 => "Tier14",
        Grade::Tier15 => "Tier15",
        Grade::Tier16 => "Tier16",
        Grade::Tier17 => "Tier17",
        Grade::Failed => "Failed",
    }
    .to_string()
}

fn pack_name_from_path(simfile_path: &std::path::Path) -> String {
    // <root>/<Pack>/<Song>/<Song>.ssc -> "Pack"
    simfile_path
        .parent()
        .and_then(|song_dir| song_dir.parent())
        .and_then(|pack_dir| pack_dir.file_name())
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/* ---------------- Publisher singleton ---------------- */

struct Publisher {
    sender: SyncSender<Update>,
}

static PUBLISHER: OnceLock<Option<Publisher>> = OnceLock::new();

/// Initialise the telemetry publisher. Spawns the background thread and any
/// requested backend listeners. Safe to call multiple times — only the
/// first call has an effect; subsequent calls are silently ignored so the
/// public API never panics on misuse.
///
/// When `cfg.enabled` is false this returns without spawning anything.
pub fn start(cfg: TelemetryConfig, dirs: &AppDirs) {
    PUBLISHER.get_or_init(|| {
        if !cfg.enabled {
            return None;
        }

        let (snapshot_tx, snapshot_rx) = sync_channel::<Update>(CHANNEL_CAPACITY);
        let telemetry_dir = dirs.data_dir.join("telemetry");
        let publisher_cfg = cfg.clone();

        // Optional WebSocket listener. Created upfront so we know if the bind
        // failed before clients try to connect; runs on its own thread and
        // forwards new sockets via a channel to the main publisher thread.
        let (ws_client_tx, ws_client_rx) = if publisher_cfg.websocket_port != 0 {
            let (tx, rx) = sync_channel::<TcpStream>(8);
            match spawn_ws_acceptor(publisher_cfg.websocket_port, tx.clone()) {
                Ok(()) => (Some(tx), Some(rx)),
                Err(err) => {
                    warn!(
                        "telemetry: failed to bind ws://{}:{} — websocket backend disabled: {err}",
                        BIND_ADDR, publisher_cfg.websocket_port
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        thread::Builder::new()
            .name("telemetry".into())
            .spawn(move || {
                publisher_loop(publisher_cfg, telemetry_dir, snapshot_rx, ws_client_rx);
            })
            .expect("spawn telemetry thread");

        // Keep ws_client_tx alive so the acceptor thread's clones don't all
        // drop out from under it. The acceptor owns a clone already; this
        // makes the intent explicit.
        drop(ws_client_tx);

        info!(
            "telemetry: enabled (file={}, websocket_port={})",
            cfg.write_state_file, cfg.websocket_port
        );
        Some(Publisher { sender: snapshot_tx })
    });
}

/// Queue an [`Update`] for emission. Non-blocking. Silently no-ops when
/// telemetry is disabled or the channel is full (back-pressure must never
/// affect game timing).
pub fn publish(update: Update) {
    let Some(Some(publisher)) = PUBLISHER.get() else {
        return;
    };
    match publisher.sender.try_send(update) {
        Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
    }
}

/* ---------------- Publisher thread ---------------- */

fn publisher_loop(
    cfg: TelemetryConfig,
    telemetry_dir: PathBuf,
    updates: Receiver<Update>,
    ws_clients: Option<Receiver<TcpStream>>,
) {
    let mut snapshot = GameStateSnapshot {
        schema_version: SCHEMA_VERSION,
        ..Default::default()
    };
    let mut sockets: Vec<WebSocket<TcpStream>> = Vec::new();
    // Most-recent Song update + the deadline at which it should be
    // applied. Subsequent Song updates replace the value and reset the
    // deadline so bursts of focus changes only emit the final, settled
    // value (typical wheel scrolling).
    let mut pending_song: Option<(Update, Instant)> = None;

    loop {
        // Drain any pending new clients first so the very next emission
        // includes them.
        if let Some(rx) = ws_clients.as_ref() {
            while let Ok(stream) = rx.try_recv() {
                match handshake_blocking(stream) {
                    Ok(ws) => {
                        debug!("telemetry: ws client connected (now {})", sockets.len() + 1);
                        sockets.push(ws);
                    }
                    Err(err) => warn!("telemetry: ws handshake failed: {err}"),
                }
            }
        }

        // Wait long enough to also catch the next pending-song deadline.
        let timeout = match pending_song.as_ref() {
            Some((_, deadline)) => deadline
                .saturating_duration_since(Instant::now())
                .min(ACCEPT_POLL_INTERVAL),
            None => ACCEPT_POLL_INTERVAL,
        };

        let mut emit = false;
        match updates.recv_timeout(timeout) {
            Ok(update) => {
                if matches!(update, Update::Song { .. }) && !cfg.song_debounce.is_zero() {
                    pending_song = Some((update, Instant::now() + cfg.song_debounce));
                } else {
                    apply_update(&mut snapshot, update);
                    emit = true;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        // Flush a pending song update if its settle window has elapsed.
        if let Some((_, deadline)) = pending_song.as_ref() {
            if Instant::now() >= *deadline {
                if let Some((update, _)) = pending_song.take() {
                    apply_update(&mut snapshot, update);
                    emit = true;
                }
            }
        }

        if !emit {
            continue;
        }
        snapshot.timestamp_ms = now_ms();
        let payload = match serde_json::to_vec(&snapshot) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!("telemetry: serialize failed: {err}");
                continue;
            }
        };
        if cfg.write_state_file {
            if let Err(err) = write_state_files(&telemetry_dir, &snapshot, &payload) {
                warn!("telemetry: file write failed: {err}");
            }
        }
        if !sockets.is_empty() {
            broadcast(&mut sockets, &payload);
        }
    }
}

fn apply_update(snapshot: &mut GameStateSnapshot, update: Update) {
    match update {
        Update::Screen(screen) => {
            snapshot.screen = screen.current_screen_file_name().to_string();
        }
        Update::Song { song, chart, music_rate } => {
            snapshot.song = song;
            snapshot.chart = chart;
            if music_rate.is_some() {
                snapshot.music_rate = music_rate;
            }
        }
        Update::Player { side, state } => {
            if snapshot.players.len() <= side {
                snapshot.players.resize(side + 1, None);
            }
            snapshot.players[side] = state;
        }
        Update::ClearPlayers => {
            snapshot.players.clear();
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/* ---------------- File backend ---------------- */

fn write_state_files(
    dir: &PathBuf,
    snapshot: &GameStateSnapshot,
    json: &[u8],
) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    write_atomic(&dir.join("state.json"), json)?;
    let nowplaying = format_now_playing(snapshot);
    write_atomic(&dir.join("nowplaying.txt"), nowplaying.as_bytes())?;
    Ok(())
}

fn write_atomic(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.clone();
    let mut name = tmp
        .file_name()
        .map(|n| n.to_owned())
        .unwrap_or_default();
    name.push(".tmp");
    tmp.set_file_name(name);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_data().ok();
    }
    fs::rename(&tmp, path)
}

fn format_now_playing(snapshot: &GameStateSnapshot) -> String {
    let Some(song) = snapshot.song.as_ref() else {
        return String::new();
    };
    let mut out = String::new();
    if !song.artist.is_empty() {
        out.push_str(&song.artist);
        out.push_str(" - ");
    }
    out.push_str(&song.title);
    if !song.subtitle.is_empty() {
        out.push(' ');
        out.push_str(&song.subtitle);
    }
    if let Some(chart) = snapshot.chart.as_ref() {
        out.push_str(" [");
        out.push_str(&chart.difficulty);
        if chart.meter > 0 {
            out.push(' ');
            out.push_str(&chart.meter.to_string());
        }
        out.push(']');
    }
    out.push('\n');
    out
}

/* ---------------- WebSocket backend ---------------- */

fn spawn_ws_acceptor(port: u16, sink: SyncSender<TcpStream>) -> std::io::Result<()> {
    let listener = TcpListener::bind((BIND_ADDR, port))?;
    listener.set_nonblocking(true)?;
    info!("telemetry: ws://{BIND_ADDR}:{port} listening");
    thread::Builder::new()
        .name("telemetry-accept".into())
        .spawn(move || acceptor_loop(listener, sink))?;
    Ok(())
}

fn acceptor_loop(listener: TcpListener, sink: SyncSender<TcpStream>) {
    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if sink.send(stream).is_err() {
                    return;
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(err) => {
                warn!("telemetry: accept failed: {err}");
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
        }
    }
}

fn handshake_blocking(stream: TcpStream) -> Result<WebSocket<TcpStream>, tungstenite::Error> {
    // Sockets produced by the listener inherit blocking mode (the default),
    // which is what tungstenite::accept needs for a one-shot handshake.
    stream.set_nonblocking(false).ok();
    let ws = tungstenite::accept(stream).map_err(|e| match e {
        tungstenite::HandshakeError::Failure(err) => err,
        tungstenite::HandshakeError::Interrupted(_) => tungstenite::Error::Io(
            std::io::Error::new(ErrorKind::WouldBlock, "handshake interrupted"),
        ),
    })?;
    // Switch to non-blocking writes after handshake so a wedged client
    // can't stall the publisher thread.
    ws.get_ref().set_nonblocking(true).ok();
    Ok(ws)
}

fn broadcast(sockets: &mut Vec<WebSocket<TcpStream>>, payload: &[u8]) {
    let text = match std::str::from_utf8(payload) {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    let mut idx = 0;
    while idx < sockets.len() {
        let drop = match sockets[idx].send(Message::Text(text.clone().into())) {
            Ok(()) => false,
            Err(tungstenite::Error::Io(e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted =>
            {
                false
            }
            Err(err) => {
                debug!("telemetry: dropping ws client: {err}");
                true
            }
        };
        if drop {
            let _ = sockets[idx].get_ref().shutdown(Shutdown::Both);
            sockets.swap_remove(idx);
        } else {
            // Also flush; ignore would-block / errors (will be retried or
            // detected on next send).
            let _ = sockets[idx].flush();
            idx += 1;
        }
    }
}
