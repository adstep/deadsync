use super::super::*;

const MASTER_VOLUME_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.master_volume_pct,
    min: VOLUME_MIN_PERCENT,
    max: VOLUME_MAX_PERCENT,
    step: NumericStep::Ms,
    persist: |v| config::update_master_volume(v as u8),
};

const MINE_SOUNDS_BINDING: CycleBinding = CycleBinding::Bool(config::update_mine_hit_sound);
const RATE_MOD_PRESERVES_PITCH_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_rate_mod_preserves_pitch);

fn apply_sound_device(state: &mut State, new_idx: usize) -> Outcome {
    let device = sound_device_from_choice(state, new_idx);
    config::update_audio_output_device(device);
    let current_rate = config::get().audio_sample_rate_hz;
    let rate_choice = sample_rate_choice_index(state, current_rate);
    if current_rate.is_some() && rate_choice == 0 {
        config::update_audio_sample_rate(None);
    }
    set_sound_choice_index(state, RowId::SndSampleRate, rate_choice);
    Outcome::changed()
}

fn apply_audio_output_mode(_state: &mut State, new_idx: usize) -> Outcome {
    config::update_audio_output_mode(audio_output_mode_from_choice(new_idx));
    #[cfg(target_os = "linux")]
    set_sound_choice_index(_state, RowId::SndAlsaExclusive, 0);
    Outcome::changed()
}

fn apply_audio_sample_rate(state: &mut State, new_idx: usize) -> Outcome {
    let rate = sample_rate_from_choice(state, new_idx);
    config::update_audio_sample_rate(rate);
    Outcome::changed()
}

#[cfg(target_os = "linux")]
fn apply_linux_audio_backend(state: &mut State, new_idx: usize) -> Outcome {
    let backend = linux_audio_backend_from_choice(state, new_idx);
    config::update_linux_audio_backend(backend);
    if matches!(backend, config::LinuxAudioBackend::Alsa) {
        set_sound_choice_index(
            state,
            RowId::SndAlsaExclusive,
            alsa_exclusive_choice_index(config::get().audio_output_mode),
        );
    } else {
        if matches!(
            config::get().audio_output_mode,
            config::AudioOutputMode::Exclusive
        ) {
            config::update_audio_output_mode(selected_audio_output_mode(state));
        }
        set_sound_choice_index(state, RowId::SndAlsaExclusive, 0);
    }
    Outcome::changed()
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
fn apply_linux_audio_backend(_state: &mut State, _new_idx: usize) -> Outcome {
    Outcome::changed()
}

#[cfg(target_os = "linux")]
fn apply_alsa_exclusive(state: &mut State, new_idx: usize) -> Outcome {
    let mode = if new_idx == 1 {
        config::AudioOutputMode::Exclusive
    } else {
        selected_audio_output_mode(state)
    };
    config::update_audio_output_mode(mode);
    Outcome::changed()
}

const SOUND_DEVICE_BINDING: CustomBinding = CustomBinding { apply: apply_sound_device };
const AUDIO_OUTPUT_MODE_BINDING: CustomBinding = CustomBinding { apply: apply_audio_output_mode };
const AUDIO_SAMPLE_RATE_BINDING: CustomBinding = CustomBinding { apply: apply_audio_sample_rate };
#[cfg(target_os = "linux")]
const LINUX_AUDIO_BACKEND_BINDING: CustomBinding = CustomBinding { apply: apply_linux_audio_backend };
#[cfg(target_os = "linux")]
const ALSA_EXCLUSIVE_BINDING: CustomBinding = CustomBinding { apply: apply_alsa_exclusive };


const SFX_VOLUME_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.sfx_volume_pct,
    min: VOLUME_MIN_PERCENT,
    max: VOLUME_MAX_PERCENT,
    step: NumericStep::Ms,
    persist: |v| config::update_sfx_volume(v as u8),
};

const ASSIST_TICK_VOLUME_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.assist_tick_volume_pct,
    min: VOLUME_MIN_PERCENT,
    max: VOLUME_MAX_PERCENT,
    step: NumericStep::Ms,
    persist: |v| config::update_assist_tick_volume(v as u8),
};

const MUSIC_VOLUME_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.music_volume_pct,
    min: VOLUME_MIN_PERCENT,
    max: VOLUME_MAX_PERCENT,
    step: NumericStep::Ms,
    persist: |v| config::update_music_volume(v as u8),
};

const GLOBAL_OFFSET_BINDING: NumericBinding = NumericBinding {
    get_mut: |s: &mut State| &mut s.global_offset_ms,
    min: GLOBAL_OFFSET_MIN_MS,
    max: GLOBAL_OFFSET_MAX_MS,
    step: NumericStep::Ms,
    persist: |v| config::update_global_offset(v as f32 / 1000.0),
};

pub(in crate::screens::options) const SOUND_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::SndDevice,
        label: lookup_key("OptionsSound", "SoundDevice"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
        behavior: RowBehavior::Custom(SOUND_DEVICE_BINDING),
    },
    SubRow {
        id: RowId::SndOutputMode,
        label: lookup_key("OptionsSound", "AudioOutputMode"),
        choices: &[
            localized_choice("OptionsSound", "OutputModeAuto"),
            localized_choice("OptionsSound", "OutputModeShared"),
        ],
        inline: false,
        behavior: RowBehavior::Custom(AUDIO_OUTPUT_MODE_BINDING),
    },
    #[cfg(target_os = "linux")]
    SubRow {
        id: RowId::SndLinuxBackend,
        label: lookup_key("OptionsSound", "LinuxAudioBackend"),
        choices: SOUND_LINUX_BACKEND_CHOICES,
        inline: false,
        behavior: RowBehavior::Custom(LINUX_AUDIO_BACKEND_BINDING),
    },
    #[cfg(target_os = "linux")]
    SubRow {
        id: RowId::SndAlsaExclusive,
        label: lookup_key("OptionsSound", "AlsaExclusive"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Custom(ALSA_EXCLUSIVE_BINDING),
    },
    SubRow {
        id: RowId::SndSampleRate,
        label: lookup_key("OptionsSound", "AudioSampleRate"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
        behavior: RowBehavior::Custom(AUDIO_SAMPLE_RATE_BINDING),
    },
    SubRow {
        id: RowId::SndMasterVolume,
        label: lookup_key("OptionsSound", "MasterVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
        behavior: RowBehavior::Numeric(MASTER_VOLUME_BINDING),
    },
    SubRow {
        id: RowId::SndSfxVolume,
        label: lookup_key("OptionsSound", "SFXVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
        behavior: RowBehavior::Numeric(SFX_VOLUME_BINDING),
    },
    SubRow {
        id: RowId::SndAssistTickVolume,
        label: lookup_key("OptionsSound", "AssistTickVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
        behavior: RowBehavior::Numeric(ASSIST_TICK_VOLUME_BINDING),
    },
    SubRow {
        id: RowId::SndMusicVolume,
        label: lookup_key("OptionsSound", "MusicVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
        behavior: RowBehavior::Numeric(MUSIC_VOLUME_BINDING),
    },
    SubRow {
        id: RowId::SndMineSounds,
        label: lookup_key("OptionsSound", "MineSounds"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(MINE_SOUNDS_BINDING),
    },
    SubRow {
        id: RowId::SndGlobalOffset,
        label: lookup_key("OptionsSound", "GlobalOffset"),
        choices: &[literal_choice("0 ms")],
        inline: false,
        behavior: RowBehavior::Numeric(GLOBAL_OFFSET_BINDING),
    },
    SubRow {
        id: RowId::SndRateModPitch,
        label: lookup_key("OptionsSound", "RateModPreservesPitch"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(RATE_MOD_PRESERVES_PITCH_BINDING),
    },
];

pub(in crate::screens::options) const SOUND_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::SndDevice,
        name: lookup_key("OptionsSound", "SoundDevice"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "SoundDeviceHelp",
        ))],
    },
    Item {
        id: RowId::SndOutputMode,
        name: lookup_key("OptionsSound", "AudioOutputMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AudioOutputModeHelp",
        ))],
    },
    #[cfg(target_os = "linux")]
    Item {
        id: RowId::SndLinuxBackend,
        name: lookup_key("OptionsSound", "LinuxAudioBackend"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "LinuxAudioBackendHelp",
        ))],
    },
    #[cfg(target_os = "linux")]
    Item {
        id: RowId::SndAlsaExclusive,
        name: lookup_key("OptionsSound", "AlsaExclusive"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AlsaExclusiveHelp",
        ))],
    },
    Item {
        id: RowId::SndSampleRate,
        name: lookup_key("OptionsSound", "AudioSampleRate"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AudioSampleRateHelp",
        ))],
    },
    Item {
        id: RowId::SndMasterVolume,
        name: lookup_key("OptionsSound", "MasterVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MasterVolumeHelp",
        ))],
    },
    Item {
        id: RowId::SndSfxVolume,
        name: lookup_key("OptionsSound", "SFXVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "SfxVolumeHelp",
        ))],
    },
    Item {
        id: RowId::SndAssistTickVolume,
        name: lookup_key("OptionsSound", "AssistTickVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AssistTickVolumeHelp",
        ))],
    },
    Item {
        id: RowId::SndMusicVolume,
        name: lookup_key("OptionsSound", "MusicVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MusicVolumeHelp",
        ))],
    },
    Item {
        id: RowId::SndMineSounds,
        name: lookup_key("OptionsSound", "MineSounds"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MineSoundsHelp",
        ))],
    },
    Item {
        id: RowId::SndGlobalOffset,
        name: lookup_key("OptionsSound", "GlobalOffset"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "GlobalOffsetHelp",
        ))],
    },
    Item {
        id: RowId::SndRateModPitch,
        name: lookup_key("OptionsSound", "RateModPreservesPitch"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "RateModPreservesPitchHelp",
        ))],
    },
    Item {
        id: RowId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

#[derive(Clone, Debug)]
pub(in crate::screens::options) struct SoundDeviceOption {
    pub(in crate::screens::options) label: String,
    pub(in crate::screens::options) config_index: Option<u16>,
    pub(in crate::screens::options) sample_rates_hz: Vec<u32>,
}

#[cfg(target_os = "linux")]
pub(in crate::screens::options) const SOUND_LINUX_BACKEND_CHOICES: &[Choice] =
    &[localized_choice("Common", "Auto")];

pub(in crate::screens::options) fn build_sound_device_options() -> Vec<SoundDeviceOption> {
    let discovered = if audio::is_initialized() {
        audio::startup_output_devices()
    } else {
        Vec::new()
    };
    let default_rates = discovered
        .iter()
        .find(|dev| dev.is_default)
        .map(|dev| dev.sample_rates_hz.clone())
        .unwrap_or_default();
    let mut options = Vec::with_capacity(discovered.len() + 1);
    options.push(SoundDeviceOption {
        label: tr("Common", "Auto").to_string(),
        config_index: None,
        sample_rates_hz: default_rates,
    });
    for (idx, dev) in discovered.into_iter().enumerate() {
        let mut label = dev.name.clone();
        if dev.is_default {
            label.push_str(&tr("OptionsSound", "DefaultSuffix"));
        }
        options.push(SoundDeviceOption {
            label,
            config_index: Some(idx as u16),
            sample_rates_hz: dev.sample_rates_hz,
        });
    }
    options
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub(in crate::screens::options) fn linux_backend_label(
    backend: config::LinuxAudioBackend,
) -> std::sync::Arc<str> {
    match backend {
        config::LinuxAudioBackend::Auto => tr("Common", "Auto"),
        config::LinuxAudioBackend::PipeWire => std::sync::Arc::from("PipeWire"),
        config::LinuxAudioBackend::PulseAudio => std::sync::Arc::from("PulseAudio"),
        config::LinuxAudioBackend::Jack => std::sync::Arc::from("JACK"),
        config::LinuxAudioBackend::Alsa => std::sync::Arc::from("ALSA"),
    }
}

#[cfg(target_os = "linux")]
pub(in crate::screens::options) fn build_linux_backend_choices() -> Vec<String> {
    audio::available_linux_backends()
        .into_iter()
        .map(|backend| linux_backend_label(backend).to_string())
        .collect()
}

pub(in crate::screens::options) fn sound_device_choice_index(
    options: &[SoundDeviceOption],
    config_index: Option<u16>,
) -> usize {
    let Some(target) = config_index else {
        return 0;
    };
    options
        .iter()
        .position(|opt| opt.config_index == Some(target))
        .unwrap_or(0)
}

pub(in crate::screens::options) const SOUND_VOLUME_LEVELS: [u8; 6] = [0, 10, 25, 50, 75, 100];

pub(in crate::screens::options) fn master_volume_choice_index(volume: u8) -> usize {
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

pub(in crate::screens::options) fn master_volume_from_choice(idx: usize) -> u8 {
    SOUND_VOLUME_LEVELS
        .get(idx)
        .copied()
        .unwrap_or_else(|| *SOUND_VOLUME_LEVELS.last().unwrap_or(&100))
}

pub(in crate::screens::options) fn sound_row_index(id: RowId) -> Option<usize> {
    SOUND_OPTIONS_ROWS.iter().position(|row| row.id == id)
}

pub(in crate::screens::options) fn selected_sound_device_choice(state: &State) -> usize {
    sound_row_index(RowId::SndDevice)
        .and_then(|idx| state.sub[SubmenuKind::Sound].choice_indices.get(idx).copied())
        .unwrap_or(0)
}

pub(in crate::screens::options) fn sound_sample_rate_choices(state: &State) -> Vec<Option<u32>> {
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

pub(in crate::screens::options) fn sound_device_from_choice(
    state: &State,
    idx: usize,
) -> Option<u16> {
    state
        .sound_device_options
        .get(idx)
        .and_then(|opt| opt.config_index)
}

pub(in crate::screens::options) fn audio_output_mode_choice_index(
    mode: config::AudioOutputMode,
) -> usize {
    match mode {
        config::AudioOutputMode::Auto => 0,
        config::AudioOutputMode::Shared | config::AudioOutputMode::Exclusive => 1,
    }
}

pub(in crate::screens::options) fn audio_output_mode_from_choice(
    idx: usize,
) -> config::AudioOutputMode {
    match idx {
        1 => config::AudioOutputMode::Shared,
        _ => config::AudioOutputMode::Auto,
    }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub(in crate::screens::options) const fn alsa_exclusive_choice_index(
    mode: config::AudioOutputMode,
) -> usize {
    if matches!(mode, config::AudioOutputMode::Exclusive) {
        1
    } else {
        0
    }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub(in crate::screens::options) fn selected_audio_output_mode(state: &State) -> config::AudioOutputMode {
    sound_row_index(RowId::SndOutputMode)
        .and_then(|idx| state.sub[SubmenuKind::Sound].choice_indices.get(idx).copied())
        .map(audio_output_mode_from_choice)
        .unwrap_or(config::AudioOutputMode::Auto)
}

#[cfg(target_os = "linux")]
pub(in crate::screens::options) fn linux_audio_backend_choice_index(
    state: &State,
    backend: config::LinuxAudioBackend,
) -> usize {
    let target = linux_backend_label(backend).to_string();
    state
        .linux_backend_choices
        .iter()
        .position(|choice| *choice == target)
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
pub(in crate::screens::options) fn linux_audio_backend_from_choice(
    state: &State,
    idx: usize,
) -> config::LinuxAudioBackend {
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
pub(in crate::screens::options) fn selected_linux_audio_backend(state: &State) -> config::LinuxAudioBackend {
    sound_row_index(RowId::SndLinuxBackend)
        .and_then(|idx| state.sub[SubmenuKind::Sound].choice_indices.get(idx).copied())
        .map(|idx| linux_audio_backend_from_choice(state, idx))
        .unwrap_or(config::LinuxAudioBackend::Auto)
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub(in crate::screens::options) fn sound_show_alsa_exclusive(state: &State) -> bool {
    matches!(
        selected_linux_audio_backend(state),
        config::LinuxAudioBackend::Alsa
    )
}

#[cfg(target_os = "linux")]
pub(in crate::screens::options) fn sound_parent_row(actual_idx: usize) -> Option<usize> {
    let child_idx = sound_row_index(RowId::SndAlsaExclusive)?;
    if actual_idx != child_idx {
        return None;
    }
    sound_row_index(RowId::SndLinuxBackend)
}

pub(in crate::screens::options) fn set_sound_choice_index(state: &mut State, id: RowId, idx: usize) {
    let Some(row_idx) = sound_row_index(id) else {
        return;
    };
    if let Some(slot) = state.sub[SubmenuKind::Sound].choice_indices.get_mut(row_idx) {
        *slot = idx;
    }
    if let Some(slot) = state.sub[SubmenuKind::Sound].cursor_indices.get_mut(row_idx) {
        *slot = idx;
    }
}

pub(in crate::screens::options) fn sample_rate_choice_index(
    state: &State,
    rate: Option<u32>,
) -> usize {
    sound_sample_rate_choices(state)
        .iter()
        .position(|&r| r == rate)
        .unwrap_or(0)
}

pub(in crate::screens::options) fn sample_rate_from_choice(
    state: &State,
    idx: usize,
) -> Option<u32> {
    sound_sample_rate_choices(state).get(idx).copied().flatten()
}
