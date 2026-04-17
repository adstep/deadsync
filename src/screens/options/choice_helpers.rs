use crate::config::{
    self, BreakdownStyle, DefaultFailType, NewPackMode,
    SelectMusicItlWheelMode, SelectMusicPatternInfoMode,
    SelectMusicScoreboxPlacement, SelectMusicWheelStyle, SyncGraphMode, LogLevel,
    MachinePreferredPlayMode, MachinePreferredPlayStyle,
};
use null_or_die::{BiasKernel, KernelTarget};

use super::*;

pub fn bg_brightness_choice_index(brightness: f32) -> usize {
    ((brightness.clamp(0.0, 1.0) * 10.0).round() as i32).clamp(0, 10) as usize
}

pub fn bg_brightness_from_choice(idx: usize) -> f32 {
    idx.min(10) as f32 / 10.0
}

pub fn music_wheel_scroll_speed_choice_index(speed: u8) -> usize {
    let mut best_idx = 0usize;
    let mut best_diff = u8::MAX;
    for (idx, value) in MUSIC_WHEEL_SCROLL_SPEED_VALUES.iter().enumerate() {
        let diff = speed.abs_diff(*value);
        if diff < best_diff {
            best_diff = diff;
            best_idx = idx;
        }
    }
    best_idx
}

pub fn music_wheel_scroll_speed_from_choice(idx: usize) -> u8 {
    MUSIC_WHEEL_SCROLL_SPEED_VALUES
        .get(idx)
        .copied()
        .unwrap_or(15)
}

#[inline(always)]
pub const fn scorebox_cycle_mask(itg: bool, ex: bool, hard_ex: bool, tournaments: bool) -> u8 {
    (itg as u8) | ((ex as u8) << 1) | ((hard_ex as u8) << 2) | ((tournaments as u8) << 3)
}

#[inline(always)]
pub const fn auto_screenshot_cursor_index(mask: u8) -> usize {
    if (mask & config::AUTO_SS_PBS) != 0 {
        0
    } else if (mask & config::AUTO_SS_FAILS) != 0 {
        1
    } else if (mask & config::AUTO_SS_CLEARS) != 0 {
        2
    } else if (mask & config::AUTO_SS_QUADS) != 0 {
        3
    } else if (mask & config::AUTO_SS_QUINTS) != 0 {
        4
    } else {
        0
    }
}

#[inline(always)]
pub const fn scorebox_cycle_cursor_index(
    itg: bool,
    ex: bool,
    hard_ex: bool,
    tournaments: bool,
) -> usize {
    if itg {
        0
    } else if ex {
        1
    } else if hard_ex {
        2
    } else if tournaments {
        3
    } else {
        0
    }
}

#[inline(always)]
pub const fn scorebox_cycle_bit_from_choice(idx: usize) -> u8 {
    if idx < SELECT_MUSIC_SCOREBOX_CYCLE_NUM_CHOICES {
        1u8 << (idx as u8)
    } else {
        0
    }
}

#[inline(always)]
pub const fn scorebox_cycle_mask_from_config(cfg: &config::Config) -> u8 {
    scorebox_cycle_mask(
        cfg.select_music_scorebox_cycle_itg,
        cfg.select_music_scorebox_cycle_ex,
        cfg.select_music_scorebox_cycle_hard_ex,
        cfg.select_music_scorebox_cycle_tournaments,
    )
}

#[inline(always)]
pub fn apply_scorebox_cycle_mask(mask: u8) {
    config::update_select_music_scorebox_cycle_itg((mask & (1u8 << 0)) != 0);
    config::update_select_music_scorebox_cycle_ex((mask & (1u8 << 1)) != 0);
    config::update_select_music_scorebox_cycle_hard_ex((mask & (1u8 << 2)) != 0);
    config::update_select_music_scorebox_cycle_tournaments((mask & (1u8 << 3)) != 0);
}

pub fn toggle_select_music_scorebox_cycle_option(state: &mut State, choice_idx: usize) {
    let bit = scorebox_cycle_bit_from_choice(choice_idx);
    if bit == 0 {
        return;
    }
    let mut mask = scorebox_cycle_mask_from_config(&config::get());
    if (mask & bit) != 0 {
        mask &= !bit;
    } else {
        mask |= bit;
    }
    apply_scorebox_cycle_mask(mask);

    let clamped = choice_idx.min(SELECT_MUSIC_SCOREBOX_CYCLE_NUM_CHOICES.saturating_sub(1));
    if let Some(slot) = state
        .sub_choice_indices_select_music
        .get_mut(SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX)
    {
        *slot = clamped;
    }
    if let Some(slot) = state
        .sub_cursor_indices_select_music
        .get_mut(SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX)
    {
        *slot = clamped;
    }
    audio::play_sfx("assets/sounds/change_value.ogg");
}

#[inline(always)]
pub fn select_music_scorebox_cycle_enabled_mask() -> u8 {
    scorebox_cycle_mask_from_config(&config::get())
}

#[inline(always)]
pub const fn auto_screenshot_bit_from_choice(idx: usize) -> u8 {
    config::auto_screenshot_bit(idx)
}

#[inline(always)]
pub fn auto_screenshot_enabled_mask() -> u8 {
    config::get().auto_screenshot_eval
}

pub fn toggle_auto_screenshot_option(state: &mut State, choice_idx: usize) {
    let bit = auto_screenshot_bit_from_choice(choice_idx);
    if bit == 0 {
        return;
    }
    let mut mask = config::get().auto_screenshot_eval;
    if (mask & bit) != 0 {
        mask &= !bit;
    } else {
        mask |= bit;
    }
    config::update_auto_screenshot_eval(mask);

    let clamped = choice_idx.min(config::AUTO_SS_NUM_FLAGS.saturating_sub(1));
    set_choice_by_id(
        &mut state.sub_choice_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::AutoScreenshot,
        clamped,
    );
    set_choice_by_id(
        &mut state.sub_cursor_indices_gameplay,
        GAMEPLAY_OPTIONS_ROWS,
        SubRowId::AutoScreenshot,
        clamped,
    );
    audio::play_sfx("assets/sounds/change_value.ogg");
}

pub const fn breakdown_style_choice_index(style: BreakdownStyle) -> usize {
    match style {
        BreakdownStyle::Sl => 0,
        BreakdownStyle::Sn => 1,
    }
}

pub const fn breakdown_style_from_choice(idx: usize) -> BreakdownStyle {
    match idx {
        1 => BreakdownStyle::Sn,
        _ => BreakdownStyle::Sl,
    }
}

pub const fn default_fail_type_choice_index(fail_type: DefaultFailType) -> usize {
    match fail_type {
        DefaultFailType::Immediate => 0,
        DefaultFailType::ImmediateContinue => 1,
    }
}

pub const fn default_fail_type_from_choice(idx: usize) -> DefaultFailType {
    match idx {
        0 => DefaultFailType::Immediate,
        _ => DefaultFailType::ImmediateContinue,
    }
}

pub const fn sync_graph_mode_choice_index(mode: SyncGraphMode) -> usize {
    match mode {
        SyncGraphMode::Frequency => 0,
        SyncGraphMode::BeatIndex => 1,
        SyncGraphMode::PostKernelFingerprint => 2,
    }
}

pub const fn sync_graph_mode_from_choice(idx: usize) -> SyncGraphMode {
    match idx {
        0 => SyncGraphMode::Frequency,
        1 => SyncGraphMode::BeatIndex,
        _ => SyncGraphMode::PostKernelFingerprint,
    }
}

pub const fn sync_confidence_choice_index(percent: u8) -> usize {
    let capped = if percent > 100 { 100 } else { percent };
    ((capped as usize) + 2) / 5
}

pub const fn sync_confidence_from_choice(idx: usize) -> u8 {
    let capped = if idx > 20 { 20 } else { idx };
    capped as u8 * 5
}

pub const fn null_or_die_kernel_target_choice_index(target: KernelTarget) -> usize {
    match target {
        KernelTarget::Digest => 0,
        KernelTarget::Accumulator => 1,
    }
}

pub const fn null_or_die_kernel_target_from_choice(idx: usize) -> KernelTarget {
    match idx {
        1 => KernelTarget::Accumulator,
        _ => KernelTarget::Digest,
    }
}

pub const fn null_or_die_kernel_type_choice_index(kind: BiasKernel) -> usize {
    match kind {
        BiasKernel::Rising => 0,
        BiasKernel::Loudest => 1,
    }
}

pub const fn null_or_die_kernel_type_from_choice(idx: usize) -> BiasKernel {
    match idx {
        1 => BiasKernel::Loudest,
        _ => BiasKernel::Rising,
    }
}

pub const fn yes_no_choice_index(enabled: bool) -> usize {
    if enabled { 1 } else { 0 }
}

pub const fn yes_no_from_choice(idx: usize) -> bool {
    idx == 1
}

pub const fn translated_titles_choice_index(translated_titles: bool) -> usize {
    if translated_titles { 0 } else { 1 }
}

pub const fn translated_titles_from_choice(idx: usize) -> bool {
    idx == 0
}

pub const fn language_choice_index(flag: config::LanguageFlag) -> usize {
    match flag {
        config::LanguageFlag::Auto | config::LanguageFlag::English => 0,
        config::LanguageFlag::Swedish => 1,
        config::LanguageFlag::Pseudo => 2,
    }
}

pub const fn language_flag_from_choice(idx: usize) -> config::LanguageFlag {
    match idx {
        1 => config::LanguageFlag::Swedish,
        2 => config::LanguageFlag::Pseudo,
        _ => config::LanguageFlag::English,
    }
}

pub const fn select_music_pattern_info_choice_index(mode: SelectMusicPatternInfoMode) -> usize {
    match mode {
        SelectMusicPatternInfoMode::Auto => 0,
        SelectMusicPatternInfoMode::Tech => 1,
        SelectMusicPatternInfoMode::Stamina => 2,
    }
}

pub const fn select_music_pattern_info_from_choice(idx: usize) -> SelectMusicPatternInfoMode {
    match idx {
        1 => SelectMusicPatternInfoMode::Tech,
        2 => SelectMusicPatternInfoMode::Stamina,
        _ => SelectMusicPatternInfoMode::Auto,
    }
}

#[inline(always)]
pub const fn select_music_chart_info_mask(peak_nps: bool, matrix_rating: bool) -> u8 {
    (peak_nps as u8) | ((matrix_rating as u8) << 1)
}

#[inline(always)]
pub const fn select_music_chart_info_cursor_index(peak_nps: bool, matrix_rating: bool) -> usize {
    if peak_nps {
        0
    } else if matrix_rating {
        1
    } else {
        0
    }
}

#[inline(always)]
pub const fn select_music_chart_info_bit_from_choice(idx: usize) -> u8 {
    if idx < SELECT_MUSIC_CHART_INFO_NUM_CHOICES {
        1u8 << (idx as u8)
    } else {
        0
    }
}

#[inline(always)]
pub const fn select_music_chart_info_mask_from_config(cfg: &config::Config) -> u8 {
    select_music_chart_info_mask(
        cfg.select_music_chart_info_peak_nps,
        cfg.select_music_chart_info_matrix_rating,
    )
}

#[inline(always)]
pub fn apply_select_music_chart_info_mask(mask: u8) {
    config::update_select_music_chart_info_peak_nps((mask & (1u8 << 0)) != 0);
    config::update_select_music_chart_info_matrix_rating((mask & (1u8 << 1)) != 0);
}

pub fn toggle_select_music_chart_info_option(state: &mut State, choice_idx: usize) {
    let bit = select_music_chart_info_bit_from_choice(choice_idx);
    if bit == 0 {
        return;
    }
    let mut mask = select_music_chart_info_mask_from_config(&config::get());
    if (mask & bit) != 0 {
        if (mask & !bit) == 0 {
            return;
        }
        mask &= !bit;
    } else {
        mask |= bit;
    }
    apply_select_music_chart_info_mask(mask);

    let clamped = choice_idx.min(SELECT_MUSIC_CHART_INFO_NUM_CHOICES.saturating_sub(1));
    if let Some(slot) = state
        .sub_choice_indices_select_music
        .get_mut(SELECT_MUSIC_CHART_INFO_ROW_INDEX)
    {
        *slot = clamped;
    }
    if let Some(slot) = state
        .sub_cursor_indices_select_music
        .get_mut(SELECT_MUSIC_CHART_INFO_ROW_INDEX)
    {
        *slot = clamped;
    }
    audio::play_sfx("assets/sounds/change_value.ogg");
}

#[inline(always)]
pub fn select_music_chart_info_enabled_mask() -> u8 {
    let mask = select_music_chart_info_mask_from_config(&config::get());
    if mask == 0 { 1 } else { mask }
}

pub const fn select_music_itl_wheel_choice_index(mode: SelectMusicItlWheelMode) -> usize {
    match mode {
        SelectMusicItlWheelMode::Off => 0,
        SelectMusicItlWheelMode::Score => 1,
        SelectMusicItlWheelMode::PointsAndScore => 2,
    }
}

pub const fn select_music_itl_wheel_from_choice(idx: usize) -> SelectMusicItlWheelMode {
    match idx {
        1 => SelectMusicItlWheelMode::Score,
        2 => SelectMusicItlWheelMode::PointsAndScore,
        _ => SelectMusicItlWheelMode::Off,
    }
}

pub const fn select_music_wheel_style_choice_index(style: SelectMusicWheelStyle) -> usize {
    match style {
        SelectMusicWheelStyle::Itg => 0,
        SelectMusicWheelStyle::Iidx => 1,
    }
}

pub const fn select_music_wheel_style_from_choice(idx: usize) -> SelectMusicWheelStyle {
    match idx {
        1 => SelectMusicWheelStyle::Iidx,
        _ => SelectMusicWheelStyle::Itg,
    }
}

pub const fn new_pack_mode_choice_index(mode: NewPackMode) -> usize {
    match mode {
        NewPackMode::Disabled => 0,
        NewPackMode::OpenPack => 1,
        NewPackMode::HasScore => 2,
    }
}

pub const fn new_pack_mode_from_choice(idx: usize) -> NewPackMode {
    match idx {
        1 => NewPackMode::OpenPack,
        2 => NewPackMode::HasScore,
        _ => NewPackMode::Disabled,
    }
}

pub const fn select_music_scorebox_placement_choice_index(
    placement: SelectMusicScoreboxPlacement,
) -> usize {
    match placement {
        SelectMusicScoreboxPlacement::Auto => 0,
        SelectMusicScoreboxPlacement::StepPane => 1,
    }
}

pub const fn select_music_scorebox_placement_from_choice(idx: usize) -> SelectMusicScoreboxPlacement {
    match idx {
        1 => SelectMusicScoreboxPlacement::StepPane,
        _ => SelectMusicScoreboxPlacement::Auto,
    }
}

pub const fn machine_preferred_style_choice_index(style: MachinePreferredPlayStyle) -> usize {
    match style {
        MachinePreferredPlayStyle::Single => 0,
        MachinePreferredPlayStyle::Versus => 1,
        MachinePreferredPlayStyle::Double => 2,
    }
}

pub const fn machine_preferred_style_from_choice(idx: usize) -> MachinePreferredPlayStyle {
    match idx {
        1 => MachinePreferredPlayStyle::Versus,
        2 => MachinePreferredPlayStyle::Double,
        _ => MachinePreferredPlayStyle::Single,
    }
}

pub const fn machine_preferred_mode_choice_index(mode: MachinePreferredPlayMode) -> usize {
    match mode {
        MachinePreferredPlayMode::Regular => 0,
        MachinePreferredPlayMode::Marathon => 1,
    }
}

pub const fn machine_preferred_mode_from_choice(idx: usize) -> MachinePreferredPlayMode {
    match idx {
        1 => MachinePreferredPlayMode::Marathon,
        _ => MachinePreferredPlayMode::Regular,
    }
}

pub const fn log_level_choice_index(level: LogLevel) -> usize {
    match level {
        LogLevel::Error => 0,
        LogLevel::Warn => 1,
        LogLevel::Info => 2,
        LogLevel::Debug => 3,
        LogLevel::Trace => 4,
    }
}

pub const fn log_level_from_choice(idx: usize) -> LogLevel {
    match idx {
        0 => LogLevel::Error,
        1 => LogLevel::Warn,
        2 => LogLevel::Info,
        3 => LogLevel::Debug,
        _ => LogLevel::Trace,
    }
}
