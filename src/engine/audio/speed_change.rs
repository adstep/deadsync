//! Pitch-preserving time-stretch (WSOLA), ported from ITGMania's
//! `RageSoundReader_SpeedChange` (Glenn Maynard, 2006).
//!
//! Changes the *tempo* of an audio stream by a speed ratio while keeping the
//! *pitch* constant, using correlation-based overlap-add. A speed ratio of `R`
//! consumes ~`R` source frames per output frame: `R > 1` plays faster, `R < 1`
//! slower; the original is reconstructed window-by-window so pitch is preserved.
//!
//! The original is a pull filter that reads from a source. Here the decoder
//! thread *pushes* decoded frames in (`push_input_i16`) and *pulls* stretched
//! frames out (`read_i16`). Callers must keep the input buffer topped up (see
//! [`SpeedChange::wants_input`]) so a [`SpeedRead::NeedInput`] only ever means
//! "feed me more", never a logic error, and signal end-of-stream with
//! [`SpeedChange::set_eof`] so the tail can be flushed.

const WINDOW_SIZE_MS: i64 = 30;

/// Result of a single [`SpeedChange::read_i16`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SpeedRead {
    /// Wrote this many frames to the output buffer.
    Produced(usize),
    /// Not enough buffered input to produce a window; push more, then retry.
    NeedInput,
    /// End of stream reached and fully drained.
    Eof,
}

struct ChannelInfo {
    // Working buffer (planar, mono). Mirrors ITGMania's per-channel m_DataBuffer.
    data: Vec<f32>,
    correlated_pos: i64,
    last_correlated_pos: i64,
}

pub(super) struct SpeedChange {
    channels: usize,
    sample_rate: u32,

    // Pushed, not-yet-consumed source audio (planar f32 in [-1, 1]).
    input: Vec<Vec<f32>>,
    input_start: usize,
    eof: bool,

    chans: Vec<ChannelInfo>,
    data_avail: i64,
    uncorrelated_pos: i64,
    pos: i64,

    speed_ratio: f32,
    trailing_speed_ratio: f32,
    error_frames: f32,
}

impl SpeedChange {
    pub(super) fn new(channels: usize, sample_rate: u32) -> Self {
        let channels = channels.max(1);
        let chans = (0..channels)
            .map(|_| ChannelInfo {
                data: Vec::new(),
                correlated_pos: 0,
                last_correlated_pos: 0,
            })
            .collect();
        Self {
            channels,
            sample_rate,
            input: (0..channels).map(|_| Vec::new()).collect(),
            input_start: 0,
            eof: false,
            chans,
            data_avail: 0,
            uncorrelated_pos: 0,
            pos: 0,
            speed_ratio: 1.0,
            trailing_speed_ratio: 1.0,
            error_frames: 0.0,
        }
    }

    pub(super) fn set_speed_ratio(&mut self, ratio: f32) {
        self.speed_ratio = ratio;
        // If nothing is in flight, apply the change immediately.
        if self.data_avail == 0 {
            self.trailing_speed_ratio = self.speed_ratio;
        }
    }

    fn window_size_frames(&self) -> i64 {
        ((WINDOW_SIZE_MS * i64::from(self.sample_rate)) / 1000).max(1)
    }

    fn tolerance_frames(&self) -> i64 {
        self.window_size_frames() / 4
    }

    /// Frames of buffered, not-yet-consumed input.
    fn input_avail(&self) -> usize {
        self.input[0].len() - self.input_start
    }

    /// Whether the caller should push more input before the next read. Returns
    /// false at EOF (the tail is flushed from whatever is buffered).
    pub(super) fn wants_input(&self) -> bool {
        if self.eof {
            return false;
        }
        let window = self.window_size_frames();
        let ratio = self.speed_ratio.max(1.0);
        // One Step can consume up to ~window*ratio source frames, plus it needs
        // window+tolerance of look-ahead for the correlation search. Keep a
        // window of slack on top so reads never starve mid-stream.
        let needed = (window as f32 * ratio).ceil() as i64 + window + self.tolerance_frames() + window;
        (self.input_avail() as i64) < needed
    }

    pub(super) fn push_input_i16(&mut self, interleaved: &[i16]) {
        if interleaved.is_empty() {
            return;
        }
        self.compact_input();
        let frames = interleaved.len() / self.channels;
        for ch in 0..self.channels {
            let dst = &mut self.input[ch];
            dst.reserve(frames);
            let mut idx = ch;
            for _ in 0..frames {
                dst.push(f32::from(interleaved[idx]) / 32768.0);
                idx += self.channels;
            }
        }
    }

    pub(super) fn set_eof(&mut self) {
        self.eof = true;
    }

    /// Reset the stretch state but keep buffered input. Used when seeking
    /// within the same stream.
    pub(super) fn reset(&mut self) {
        self.trailing_speed_ratio = self.speed_ratio;
        self.data_avail = 0;
        for c in &mut self.chans {
            c.correlated_pos = 0;
            c.last_correlated_pos = 0;
        }
        self.uncorrelated_pos = 0;
        self.pos = 0;
        self.error_frames = 0.0;
    }

    /// Full clear, including buffered input and the EOF flag. Used when the
    /// decoder reopens the source (looping).
    pub(super) fn clear(&mut self) {
        self.reset();
        for ch in &mut self.input {
            ch.clear();
        }
        self.input_start = 0;
        self.eof = false;
    }

    fn compact_input(&mut self) {
        if self.input_start == 0 {
            return;
        }
        // Only shift once the consumed prefix is large, to amortise the cost.
        if self.input_start < 8192 || self.input_start * 2 < self.input[0].len() {
            return;
        }
        for ch in &mut self.input {
            ch.drain(0..self.input_start);
        }
        self.input_start = 0;
    }

    /// Move `frames` from the input FIFO into each channel's working buffer at
    /// `data_avail`. Returns the number of frames actually moved.
    fn pull_from_input(&mut self, frames: usize) -> usize {
        let got = frames.min(self.input_avail());
        if got == 0 {
            return 0;
        }
        let base = self.data_avail as usize;
        let src_start = self.input_start;
        for ch in 0..self.channels {
            let dst = &mut self.chans[ch].data;
            if dst.len() < base + got {
                dst.resize(base + got, 0.0);
            }
            dst[base..base + got].copy_from_slice(&self.input[ch][src_start..src_start + got]);
        }
        self.input_start += got;
        self.data_avail += got as i64;
        got
    }

    /// Fill the working buffer up to `max_frames`. Returns `Some(data_avail)`
    /// (possibly < max_frames at EOF), or `None` if more input is required and
    /// the stream has not ended.
    fn fill_data(&mut self, max_frames: i64) -> Option<i64> {
        // Ensure buffers can hold the target so correlation slices stay in bounds.
        if max_frames > 0 {
            let target = max_frames as usize;
            for c in &mut self.chans {
                if c.data.len() < target {
                    c.data.resize(target, 0.0);
                }
            }
        }
        while self.data_avail < max_frames {
            let want = (max_frames - self.data_avail) as usize;
            let got = self.pull_from_input(want);
            if got == 0 {
                if self.eof {
                    break;
                }
                return None;
            }
        }
        Some(self.data_avail)
    }

    fn erase_data(&mut self, frames: i64) {
        if frames <= 0 {
            return;
        }
        let frames_us = frames as usize;
        debug_assert!(frames <= self.data_avail);
        debug_assert!(frames <= self.uncorrelated_pos);
        let to_move = (self.data_avail - frames) as usize;
        self.data_avail -= frames;
        self.uncorrelated_pos -= frames;
        for c in &mut self.chans {
            if to_move > 0 {
                c.data.copy_within(frames_us..frames_us + to_move, 0);
            }
            debug_assert!(c.correlated_pos >= frames);
            c.correlated_pos -= frames;
        }
    }

    /// Returns `Some(data_avail)` after preparing the next block, or `None` if
    /// more input is needed.
    fn step(&mut self) -> Option<i64> {
        let window = self.window_size_frames();

        if self.data_avail == 0 {
            return self.fill_data(window);
        }

        if self.pos != 0 {
            for c in &mut self.chans {
                debug_assert!(c.correlated_pos + self.pos <= self.data_avail);
                c.correlated_pos += self.pos;
            }
            let mut advance = window as f32 * self.trailing_speed_ratio;
            advance += self.error_frames;
            let trailing_delta = (advance + 0.5).floor() as i64;
            self.error_frames = advance - trailing_delta as f32;
            self.uncorrelated_pos += trailing_delta;
            self.pos = 0;
        }

        self.trailing_speed_ratio = self.speed_ratio;

        // Drop data before the earlier of the uncorrelated/correlated cursors.
        let mut to_delete = self.uncorrelated_pos;
        for c in &self.chans {
            to_delete = to_delete.min(c.correlated_pos);
        }
        self.erase_data(to_delete);

        // Fill enough for the search and the copy that follows.
        let mut max_needed = self.uncorrelated_pos + self.tolerance_frames() + window;
        for c in &self.chans {
            max_needed = max_needed.max(c.correlated_pos + window);
        }
        let avail = self.fill_data(max_needed)?;
        if max_needed > avail {
            // EOF: flush whatever remains.
            self.uncorrelated_pos = self.chans[0].correlated_pos;
            return Some(avail);
        }

        let correlated_to_match = window / 4;
        let uncorrelated_to_match = self.tolerance_frames() + correlated_to_match;
        let stride = self.channels;
        let uncorrelated_pos = self.uncorrelated_pos as usize;
        for c in &mut self.chans {
            let correlated_pos = c.correlated_pos as usize;
            let best = find_closest_match(
                &c.data[uncorrelated_pos..],
                uncorrelated_to_match as usize,
                &c.data[correlated_pos..],
                correlated_to_match as usize,
                stride,
            );
            c.last_correlated_pos = c.correlated_pos;
            c.correlated_pos = best as i64 + self.uncorrelated_pos;
            debug_assert!(c.correlated_pos + window <= self.data_avail);
        }
        Some(self.data_avail)
    }

    fn cursor_avail(&self) -> i64 {
        let mut avail = self.window_size_frames() - self.pos;
        for c in &self.chans {
            let for_channel = (self.data_avail - c.correlated_pos) - self.pos;
            avail = avail.min(for_channel);
        }
        avail.max(0)
    }

    /// Pull up to `max_frames` stretched frames, appending interleaved i16 to
    /// `out`. Produces at most one window's worth per call; loop until
    /// `NeedInput`/`Eof`.
    pub(super) fn read_i16(&mut self, out: &mut Vec<i16>, max_frames: usize) -> SpeedRead {
        if max_frames == 0 {
            return SpeedRead::Produced(0);
        }
        loop {
            // Fast path: idle buffer at unity speed copies input straight through.
            if self.data_avail == 0
                && self.trailing_speed_ratio == self.speed_ratio
                && self.speed_ratio == 1.0
            {
                let avail = self.input_avail();
                if avail == 0 {
                    return if self.eof {
                        SpeedRead::Eof
                    } else {
                        SpeedRead::NeedInput
                    };
                }
                let n = avail.min(max_frames);
                let start = self.input_start;
                out.reserve(n * self.channels);
                for f in 0..n {
                    for ch in 0..self.channels {
                        out.push(to_i16(self.input[ch][start + f]));
                    }
                }
                self.input_start += n;
                return SpeedRead::Produced(n);
            }

            let avail = self.cursor_avail();
            if avail == 0 {
                match self.step() {
                    None => return SpeedRead::NeedInput,
                    Some(_) => {}
                }
                if self.cursor_avail() == 0 {
                    return SpeedRead::Eof;
                }
                continue;
            }

            let window = self.window_size_frames();
            let n = (avail as usize).min(max_frames);
            out.reserve(n * self.channels);
            for _ in 0..n {
                let pos = self.pos;
                for c in &self.chans {
                    let i1 = c.data[(c.correlated_pos + pos) as usize];
                    let i2 = c.data[(c.last_correlated_pos + pos) as usize];
                    out.push(to_i16(scale(pos, 0, window, i2, i1)));
                }
                self.pos += 1;
            }
            return SpeedRead::Produced(n);
        }
    }
}

#[inline]
fn to_i16(v: f32) -> i16 {
    (v * 32768.0).clamp(-32768.0, 32767.0) as i16
}

/// Linear interpolation matching ITGMania's `SCALE` macro:
/// map `x` from `[l1, h1]` onto `[l2, h2]`.
#[inline]
fn scale(x: i64, l1: i64, h1: i64, l2: f32, h2: f32) -> f32 {
    if h1 == l1 {
        return l2;
    }
    let t = (x - l1) as f32 / (h1 - l1) as f32;
    l2 + (h2 - l2) * t
}

/// Search `buffer` for the offset whose `correlate`-length window best matches
/// `correlate` (lowest summed absolute difference). `stride` subsamples the
/// comparison for speed (one frame per channel for interleaved-style data).
fn find_closest_match(
    buffer: &[f32],
    buffer_size: usize,
    correlate: &[f32],
    correlate_size: usize,
    stride: usize,
) -> usize {
    if buffer_size <= correlate_size {
        return 0;
    }
    let stride = stride.max(1);
    let distance = buffer_size - correlate_size;
    let mut best_offset = 0usize;
    let mut best_score = 0.0f32;
    let mut i = 0usize;
    while i < distance {
        let mut score = 0.0f32;
        let frames = &buffer[i..];
        let mut j = 0usize;
        while j < correlate_size {
            score += (frames[j] - correlate[j]).abs();
            j += stride;
        }
        if i == 0 || score < best_score {
            best_score = score;
            best_offset = i;
        }
        i += stride;
    }
    best_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(sc: &mut SpeedChange, max_total: usize) -> Vec<i16> {
        let mut out = Vec::new();
        loop {
            match sc.read_i16(&mut out, 512) {
                SpeedRead::Produced(_) => {
                    if out.len() / sc.channels >= max_total {
                        break;
                    }
                }
                SpeedRead::NeedInput | SpeedRead::Eof => break,
            }
        }
        out
    }

    fn sine_i16(frames: usize, freq: f32, sr: u32) -> Vec<i16> {
        (0..frames)
            .map(|i| {
                let t = i as f32 / sr as f32;
                ((t * freq * std::f32::consts::TAU).sin() * 16000.0) as i16
            })
            .collect()
    }

    #[test]
    fn unity_speed_passes_through() {
        let mut sc = SpeedChange::new(1, 48_000);
        sc.set_speed_ratio(1.0);
        let input = sine_i16(2048, 440.0, 48_000);
        sc.push_input_i16(&input);
        sc.set_eof();
        let out = drain(&mut sc, usize::MAX);
        // Unity speed uses the fast path: output equals input exactly.
        assert_eq!(out, input);
    }

    #[test]
    fn faster_speed_shortens_output() {
        let mut sc = SpeedChange::new(1, 48_000);
        sc.set_speed_ratio(1.5);
        let frames = 48_000;
        let input = sine_i16(frames, 440.0, 48_000);
        sc.push_input_i16(&input);
        sc.set_eof();
        let out = drain(&mut sc, usize::MAX);
        let out_frames = out.len();
        let expected = frames as f32 / 1.5;
        // WSOLA is block-based, so allow a window of slack.
        let tol = 0.05 * expected;
        assert!(
            (out_frames as f32 - expected).abs() < tol,
            "1.5x: got {out_frames} frames, expected ~{expected}"
        );
    }

    #[test]
    fn slower_speed_lengthens_output() {
        let mut sc = SpeedChange::new(1, 48_000);
        sc.set_speed_ratio(0.75);
        let frames = 48_000;
        let input = sine_i16(frames, 440.0, 48_000);
        sc.push_input_i16(&input);
        sc.set_eof();
        let out = drain(&mut sc, usize::MAX);
        let out_frames = out.len();
        let expected = frames as f32 / 0.75;
        let tol = 0.05 * expected;
        assert!(
            (out_frames as f32 - expected).abs() < tol,
            "0.75x: got {out_frames} frames, expected ~{expected}"
        );
    }

    #[test]
    fn stereo_faster_speed_shortens_output() {
        let mut sc = SpeedChange::new(2, 48_000);
        sc.set_speed_ratio(2.0);
        let frames = 96_000;
        let mut input = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let t = i as f32 / 48_000.0;
            let v = ((t * 330.0 * std::f32::consts::TAU).sin() * 16000.0) as i16;
            input.push(v);
            input.push(v / 2);
        }
        sc.push_input_i16(&input);
        sc.set_eof();
        let out = drain(&mut sc, usize::MAX);
        let out_frames = out.len() / 2;
        let expected = frames as f32 / 2.0;
        // Block-based stretching plus the first-window passthrough and EOF flush
        // inflate the count slightly; allow generous slack over a long clip.
        let tol = 0.1 * expected;
        assert!(
            out_frames < frames,
            "2.0x stereo should be shorter than input: {out_frames} vs {frames}"
        );
        assert!(
            (out_frames as f32 - expected).abs() < tol,
            "2.0x stereo: got {out_frames} frames, expected ~{expected}"
        );
    }

    #[test]
    fn find_closest_match_identical_is_zero() {
        let buf: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();
        assert_eq!(find_closest_match(&buf, 80, &buf, 20, 1), 0);
    }

    #[test]
    fn find_closest_match_locates_shift() {
        // Distinctive non-periodic pattern embedded at a known offset in noise.
        let pattern: Vec<f32> = (0..20).map(|i| ((i * 7 % 13) as f32 - 6.0) * 0.1).collect();
        let mut buffer = vec![0.0f32; 140];
        // Deterministic pseudo-random filler (LCG) so no accidental better match.
        let mut state: u32 = 0x1234_5678;
        for v in buffer.iter_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v = ((state >> 8) as f32 / u32::MAX as f32) - 0.5;
        }
        let shift = 17usize;
        buffer[shift..shift + pattern.len()].copy_from_slice(&pattern);
        let best = find_closest_match(&buffer, 120, &pattern, pattern.len(), 1);
        assert_eq!(best, shift, "expected exact match at {shift}, got {best}");
    }

    #[test]
    fn needs_input_before_eof() {
        let mut sc = SpeedChange::new(1, 48_000);
        sc.set_speed_ratio(1.5);
        // Not enough buffered to produce a window and not at EOF yet.
        let mut out = Vec::new();
        assert_eq!(sc.read_i16(&mut out, 512), SpeedRead::NeedInput);
        assert!(out.is_empty());
    }

    #[test]
    fn eof_drain_preserves_short_tails() {
        // Regression for the decoder EOF path: when the source ends, draining
        // the stretcher must flush a complete tail without dropping it or
        // looping forever — including clips shorter than, around, and longer
        // than one 30 ms window (window = 30 * 48000 / 1000 = 1440 frames).
        let window = 30 * 48_000 / 1000;
        for &frames in &[64usize, window, 5_000] {
            let mut sc = SpeedChange::new(1, 48_000);
            sc.set_speed_ratio(1.5);
            let input = sine_i16(frames, 440.0, 48_000);
            sc.push_input_i16(&input);
            sc.set_eof();
            // Cap well above any plausible output to catch a runaway loop.
            let out = drain(&mut sc, frames * 4 + window);
            let out_frames = out.len();
            assert!(
                out_frames > 0,
                "EOF drain dropped the whole tail for {frames}-frame clip"
            );
            // Output may not exceed the input by more than one passthrough
            // window; a larger value means the EOF flush ran away.
            assert!(
                out_frames <= frames + window,
                "EOF drain produced {out_frames} frames for {frames}-frame clip (runaway)"
            );
            // After the flush the stretcher is exhausted.
            let mut more = Vec::new();
            assert_eq!(sc.read_i16(&mut more, 512), SpeedRead::Eof);
            assert!(more.is_empty());
        }
    }

    #[test]
    fn reset_clears_working_state() {
        let mut sc = SpeedChange::new(1, 48_000);
        sc.set_speed_ratio(1.5);
        sc.push_input_i16(&sine_i16(48_000, 440.0, 48_000));
        let mut out = Vec::new();
        let _ = sc.read_i16(&mut out, 512);
        sc.clear();
        assert_eq!(sc.data_avail, 0);
        assert_eq!(sc.input_avail(), 0);
        assert!(!sc.eof);
    }
}
