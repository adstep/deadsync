use crate::config;

use super::*;

pub fn master_volume_choice_index(volume: u8) -> usize {
    let mut best_idx = 0usize;
    let mut best_diff = u8::MAX;
    for (idx, level) in SOUND_VOLUME_LEVELS.iter().enumerate() {
        let diff = volume.abs_diff(*level);
        if diff < best_diff {
            best_diff = diff;
            best_idx = idx;
        }
    }
    best_idx
}

pub fn master_volume_from_choice(idx: usize) -> u8 {
    SOUND_VOLUME_LEVELS
        .get(idx)
        .copied()
        .unwrap_or_else(|| *SOUND_VOLUME_LEVELS.last().unwrap_or(&100))
}

pub fn sound_row_index(id: SubRowId) -> Option<usize> {
    SOUND_OPTIONS_ROWS.iter().position(|row| row.id == id)
}

pub fn selected_sound_device_choice(state: &State) -> usize {
    sound_row_index(SubRowId::SoundDevice)
        .and_then(|idx| state.sub_choice_indices_sound.get(idx).copied())
        .unwrap_or(0)
}

pub fn sound_sample_rate_choices(state: &State) -> Vec<Option<u32>> {
    let mut choices = Vec::new();
    choices.push(None);
    let device_idx =
        selected_sound_device_choice(state).min(state.sound_device_options.len().saturating_sub(1));
    if let Some(option) = state.sound_device_options.get(device_idx) {
        for &hz in &option.sample_rates_hz {
            let rate = Some(hz);
            if !choices.contains(&rate) {
                choices.push(rate);
            }
        }
    }
    if choices.len() == 1 {
        choices.push(Some(44100));
        choices.push(Some(48000));
    }
    choices
}

pub fn sound_device_choice_index(options: &[SoundDeviceOption], config_index: Option<u16>) -> usize {
    let Some(target) = config_index else {
        return 0;
    };
    options
        .iter()
        .position(|opt| opt.config_index == Some(target))
        .unwrap_or(0)
}

pub fn sound_device_from_choice(state: &State, idx: usize) -> Option<u16> {
    state
        .sound_device_options
        .get(idx)
        .and_then(|opt| opt.config_index)
}

pub fn audio_output_mode_choice_index(mode: config::AudioOutputMode) -> usize {
    match mode {
        config::AudioOutputMode::Auto => 0,
        config::AudioOutputMode::Shared | config::AudioOutputMode::Exclusive => 1,
    }
}

pub fn audio_output_mode_from_choice(idx: usize) -> config::AudioOutputMode {
    match idx {
        1 => config::AudioOutputMode::Shared,
        _ => config::AudioOutputMode::Auto,
    }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub const fn alsa_exclusive_choice_index(mode: config::AudioOutputMode) -> usize {
    if matches!(mode, config::AudioOutputMode::Exclusive) {
        1
    } else {
        0
    }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub fn selected_audio_output_mode(state: &State) -> config::AudioOutputMode {
    sound_row_index(SubRowId::AudioOutputMode)
        .and_then(|idx| state.sub_choice_indices_sound.get(idx).copied())
        .map(audio_output_mode_from_choice)
        .unwrap_or(config::AudioOutputMode::Auto)
}

#[cfg(target_os = "linux")]
pub fn linux_audio_backend_choice_index(state: &State, backend: config::LinuxAudioBackend) -> usize {
    let target = linux_backend_label(backend).to_string();
    state
        .linux_backend_choices
        .iter()
        .position(|choice| *choice == target)
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
pub fn linux_audio_backend_from_choice(state: &State, idx: usize) -> config::LinuxAudioBackend {
    match state
        .linux_backend_choices
        .get(idx)
        .map(String::as_str)
        .unwrap_or("Auto")
    {
        "PipeWire" => config::LinuxAudioBackend::PipeWire,
        "PulseAudio" => config::LinuxAudioBackend::PulseAudio,
        "JACK" => config::LinuxAudioBackend::Jack,
        "ALSA" => config::LinuxAudioBackend::Alsa,
        _ => config::LinuxAudioBackend::Auto,
    }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub fn selected_linux_audio_backend(state: &State) -> config::LinuxAudioBackend {
    sound_row_index(SubRowId::LinuxAudioBackend)
        .and_then(|idx| state.sub_choice_indices_sound.get(idx).copied())
        .map(|idx| linux_audio_backend_from_choice(state, idx))
        .unwrap_or(config::LinuxAudioBackend::Auto)
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub fn sound_show_alsa_exclusive(state: &State) -> bool {
    matches!(
        selected_linux_audio_backend(state),
        config::LinuxAudioBackend::Alsa
    )
}

#[cfg(target_os = "linux")]
pub fn sound_parent_row(actual_idx: usize) -> Option<usize> {
    let child_idx = sound_row_index(SubRowId::AlsaExclusive)?;
    if actual_idx != child_idx {
        return None;
    }
    sound_row_index(SubRowId::LinuxAudioBackend)
}

pub fn set_sound_choice_index(state: &mut State, id: SubRowId, idx: usize) {
    let Some(row_idx) = sound_row_index(id) else {
        return;
    };
    if let Some(slot) = state.sub_choice_indices_sound.get_mut(row_idx) {
        *slot = idx;
    }
    if let Some(slot) = state.sub_cursor_indices_sound.get_mut(row_idx) {
        *slot = idx;
    }
}

pub fn sample_rate_choice_index(state: &State, rate: Option<u32>) -> usize {
    sound_sample_rate_choices(state)
        .iter()
        .position(|&r| r == rate)
        .unwrap_or(0)
}

pub fn sample_rate_from_choice(state: &State, idx: usize) -> Option<u32> {
    sound_sample_rate_choices(state).get(idx).copied().flatten()
}
