use std::sync::Arc;

use crate::assets::i18n::{LookupKey, lookup_key, tr};
use crate::config;
use crate::engine::audio;
use crate::engine::display::MonitorSpec;
use crate::engine::gfx::{BackendType, PresentModePolicy};
use crate::game::{profile, scores};
use crate::screens::Screen;
use null_or_die::{BiasKernel, KernelTarget};

use super::*;

pub const ITEMS: &[Item] = &[
    // Top-level ScreenOptionsService rows, ordered to match Simply Love's LineNames.
    Item {
        id: ItemId::SystemOptions,
        name: lookup_key("Options", "SystemOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "SystemOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsSystem", "Game")),
            HelpEntry::Bullet(lookup_key("OptionsSystem", "Theme")),
            HelpEntry::Bullet(lookup_key("OptionsSystem", "Language")),
            HelpEntry::Bullet(lookup_key("OptionsSystem", "LogFile")),
            HelpEntry::Bullet(lookup_key("OptionsSystem", "DefaultNoteSkin")),
        ],
    },
    Item {
        id: ItemId::GraphicsOptions,
        name: lookup_key("Options", "GraphicsOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "GraphicsOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "VideoRenderer")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "DisplayMode")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "DisplayAspectRatio")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "DisplayResolution")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "RefreshRate")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "FullscreenType")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "VSync")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "PresentMode")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "MaxFps")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "ShowStats")),
            HelpEntry::Bullet(lookup_key("OptionsGraphics", "VisualDelay")),
        ],
    },
    Item {
        id: ItemId::SoundOptions,
        name: lookup_key("Options", "SoundOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "SoundOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "SoundDevice")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "AudioSampleRate")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "MasterVolume")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "SfxVolume")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "AssistTickVolume")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "MusicVolume")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "MineSounds")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "GlobalOffset")),
            HelpEntry::Bullet(lookup_key("OptionsSound", "RateModPreservesPitch")),
        ],
    },
    Item {
        id: ItemId::InputOptions,
        name: lookup_key("Options", "InputOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "InputOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "ConfigureMappings")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "TestInput")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "InputOptions")),
        ],
    },
    Item {
        id: ItemId::MachineOptions,
        name: lookup_key("Options", "MachineOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "MachineOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "SelectProfile")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "SelectColor")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "SelectStyle")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "SelectPlayMode")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "EvalSummary")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "NameEntry")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "GameoverScreen")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "MenuMusic")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "KeyboardFeatures")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "VideoBgs")),
            HelpEntry::Bullet(lookup_key("OptionsMachine", "WriteCurrentScreen")),
        ],
    },
    Item {
        id: ItemId::GameplayOptions,
        name: lookup_key("Options", "GameplayOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "GameplayOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsGameplay", "BgBrightness")),
            HelpEntry::Bullet(lookup_key("OptionsGameplay", "CenteredP1Notefield")),
            HelpEntry::Bullet(lookup_key("OptionsGameplay", "ZmodRatingBox")),
            HelpEntry::Bullet(lookup_key("OptionsGameplay", "BpmDecimal")),
        ],
    },
    Item {
        id: ItemId::SelectMusicOptions,
        name: lookup_key("Options", "SelectMusicOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "SelectMusicOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowBanners")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowVideoBanners")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowBreakdown")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowNativeLanguage")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "MusicWheelSpeed")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowCdTitles")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowWheelGrades")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowWheelLamps")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "NewPackBadge")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowPatternInfo")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ChartInfo")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "MusicPreviews")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowGameplayTimer")),
            HelpEntry::Bullet(lookup_key("OptionsSelectMusic", "ShowGsBox")),
        ],
    },
    Item {
        id: ItemId::AdvancedOptions,
        name: lookup_key("Options", "AdvancedOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "AdvancedOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "DefaultFailType")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "BannerCache")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "CdTitleCache")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "SongParsingThreads")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "CacheSongs")),
            HelpEntry::Bullet(lookup_key("OptionsAdvanced", "FastLoad")),
        ],
    },
    Item {
        id: ItemId::CourseOptions,
        name: lookup_key("Options", "CourseOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "CourseOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsCourse", "ShowRandomCourses")),
            HelpEntry::Bullet(lookup_key("OptionsCourse", "ShowMostPlayed")),
            HelpEntry::Bullet(lookup_key("OptionsCourse", "ShowIndividualScores")),
            HelpEntry::Bullet(lookup_key("OptionsCourse", "AutosubmitIndividual")),
        ],
    },
    Item {
        id: ItemId::ManageLocalProfiles,
        name: lookup_key("Options", "ManageLocalProfiles"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ManageLocalProfilesHelp",
        ))],
    },
    Item {
        id: ItemId::OnlineScoreServices,
        name: lookup_key("Options", "OnlineScoreServices"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "OnlineScoreServicesHelp")),
            HelpEntry::Bullet(lookup_key("OptionsOnlineScoring", "GsBsOptions")),
            HelpEntry::Bullet(lookup_key("OptionsOnlineScoring", "ArrowCloudOptions")),
            HelpEntry::Bullet(lookup_key("OptionsOnlineScoring", "ScoreImport")),
        ],
    },
    Item {
        id: ItemId::NullOrDieOptions,
        name: lookup_key("Options", "NullOrDieOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsHelp", "NullOrDieOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsOnlineScoring", "NullOrDieOptions")),
            HelpEntry::Bullet(lookup_key("OptionsOnlineScoring", "SyncPacks")),
        ],
    },
    Item {
        id: ItemId::ReloadSongsCourses,
        name: lookup_key("Options", "ReloadSongsCourses"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ReloadSongsCoursesHelp",
        ))],
    },
    Item {
        id: ItemId::Credits,
        name: lookup_key("Options", "Credits"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "CreditsHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key("OptionsHelp", "ExitHelp"))],
    },
];


// Local fade timing when swapping between main options list and System Options submenu.

/// Shorthand for `Choice::Localized(lookup_key(section, key))` in const arrays.
#[allow(non_snake_case)]
pub const fn localized_choice(section: &'static str, key: &'static str) -> Choice {
    Choice::Localized(lookup_key(section, key))
}

/// Shorthand for `Choice::Literal(s)` in const arrays.
pub const fn literal_choice(s: &'static str) -> Choice {
    Choice::Literal(s)
}

#[cfg(target_os = "windows")]
pub const INPUT_BACKEND_CHOICES: &[Choice] = &[
    literal_choice("W32 Raw Input"),
    literal_choice("WGI (compat)"),
];
#[cfg(target_os = "macos")]
pub const INPUT_BACKEND_CHOICES: &[Choice] = &[literal_choice("macOS IOHID")];
#[cfg(target_os = "linux")]
pub const INPUT_BACKEND_CHOICES: &[Choice] = &[literal_choice("Linux evdev")];
#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
pub const INPUT_BACKEND_CHOICES: &[Choice] = &[literal_choice("Platform Default")];
#[cfg(not(any(target_os = "windows", unix)))]
pub const INPUT_BACKEND_CHOICES: &[Choice] = &[literal_choice("Platform Default")];
#[cfg(target_os = "windows")]
pub const INPUT_BACKEND_INLINE: bool = true;
#[cfg(not(target_os = "windows"))]
pub const INPUT_BACKEND_INLINE: bool = false;
pub const SELECT_MUSIC_SCOREBOX_CYCLE_NUM_CHOICES: usize = 4;
pub const SELECT_MUSIC_CHART_INFO_NUM_CHOICES: usize = 2;

pub const SCORE_IMPORT_DONE_OVERLAY_SECONDS: f32 = 1.5;
pub const SCORE_IMPORT_ROW_ENDPOINT_INDEX: usize = 0;
pub const SCORE_IMPORT_ROW_PROFILE_INDEX: usize = 1;
pub const SCORE_IMPORT_ROW_PACK_INDEX: usize = 2;
pub const SCORE_IMPORT_ROW_ONLY_MISSING_INDEX: usize = 3;
pub const SYNC_PACK_ROW_PACK_INDEX: usize = 0;

#[cfg(target_os = "linux")]
pub const SOUND_LINUX_BACKEND_CHOICES: &[Choice] = &[localized_choice("Common", "Auto")];

pub fn discover_system_noteskin_choices() -> Vec<String> {
    let mut names = noteskin_parser::discover_itg_skins("dance");
    if names.is_empty() {
        names.push(profile::NoteSkin::DEFAULT_NAME.to_string());
    }
    names
}

pub fn build_sound_device_options() -> Vec<SoundDeviceOption> {
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
pub fn linux_backend_label(backend: config::LinuxAudioBackend) -> std::sync::Arc<str> {
    match backend {
        config::LinuxAudioBackend::Auto => tr("Common", "Auto"),
        config::LinuxAudioBackend::PipeWire => std::sync::Arc::from("PipeWire"),
        config::LinuxAudioBackend::PulseAudio => std::sync::Arc::from("PulseAudio"),
        config::LinuxAudioBackend::Jack => std::sync::Arc::from("JACK"),
        config::LinuxAudioBackend::Alsa => std::sync::Arc::from("ALSA"),
    }
}

#[cfg(target_os = "linux")]
pub fn build_linux_backend_choices() -> Vec<String> {
    audio::available_linux_backends()
        .into_iter()
        .map(|backend| linux_backend_label(backend).to_string())
        .collect()
}

pub const SYSTEM_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::Game,
        label: lookup_key("OptionsSystem", "Game"),
        choices: &[localized_choice("OptionsSystem", "DanceGame")],
        inline: false,
    },
    SubRow {
        id: SubRowId::Theme,
        label: lookup_key("OptionsSystem", "Theme"),
        choices: &[localized_choice("OptionsSystem", "SimplyLoveTheme")],
        inline: false,
    },
    SubRow {
        id: SubRowId::Language,
        label: lookup_key("OptionsSystem", "Language"),
        choices: &[
            localized_choice("OptionsSystem", "EnglishLanguage"),
            localized_choice("OptionsSystem", "SwedishLanguage"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::LogLevel,
        label: lookup_key("OptionsSystem", "LogLevel"),
        choices: &[
            localized_choice("OptionsSystem", "LogLevelError"),
            localized_choice("OptionsSystem", "LogLevelWarn"),
            localized_choice("OptionsSystem", "LogLevelInfo"),
            localized_choice("OptionsSystem", "LogLevelDebug"),
            localized_choice("OptionsSystem", "LogLevelTrace"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::LogFile,
        label: lookup_key("OptionsSystem", "LogFile"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::DefaultNoteSkin,
        label: lookup_key("OptionsSystem", "DefaultNoteSkin"),
        choices: &[literal_choice(profile::NoteSkin::DEFAULT_NAME)],
        inline: false,
    },
];

pub const SYSTEM_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SysGame,
        name: lookup_key("OptionsSystem", "Game"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "GameHelp",
        ))],
    },
    Item {
        id: ItemId::SysTheme,
        name: lookup_key("OptionsSystem", "Theme"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "ThemeHelp",
        ))],
    },
    Item {
        id: ItemId::SysLanguage,
        name: lookup_key("OptionsSystem", "Language"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LanguageHelp",
        ))],
    },
    Item {
        id: ItemId::SysLogLevel,
        name: lookup_key("OptionsSystem", "LogLevel"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LogLevelHelp",
        ))],
    },
    Item {
        id: ItemId::SysLogFile,
        name: lookup_key("OptionsSystem", "LogFile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "LogFileHelp",
        ))],
    },
    Item {
        id: ItemId::SysDefaultNoteSkin,
        name: lookup_key("OptionsSystem", "DefaultNoteSkin"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSystemHelp",
            "DefaultNoteSkinHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

#[cfg(all(target_os = "windows", not(target_pointer_width = "32")))]
pub const VIDEO_RENDERER_OPTIONS: &[(BackendType, &str)] = &[
    (BackendType::OpenGL, "OpenGL"),
    (BackendType::Vulkan, "Vulkan"),
    (BackendType::DirectX, "DirectX"),
    (BackendType::OpenGLWgpu, "OpenGL (wgpu)"),
    (BackendType::VulkanWgpu, "Vulkan (wgpu)"),
    (BackendType::Software, "Software"),
];
#[cfg(all(target_os = "windows", target_pointer_width = "32"))]
pub const VIDEO_RENDERER_OPTIONS: &[(BackendType, &str)] = &[
    (BackendType::OpenGL, "OpenGL"),
    (BackendType::DirectX, "DirectX"),
    (BackendType::OpenGLWgpu, "OpenGL (wgpu)"),
    (BackendType::Software, "Software"),
];
#[cfg(all(target_os = "macos", not(target_pointer_width = "32")))]
pub const VIDEO_RENDERER_OPTIONS: &[(BackendType, &str)] = &[
    (BackendType::OpenGL, "OpenGL"),
    (BackendType::Vulkan, "Vulkan"),
    (BackendType::Metal, "Metal (wgpu)"),
    (BackendType::OpenGLWgpu, "OpenGL (wgpu)"),
    (BackendType::VulkanWgpu, "Vulkan (wgpu)"),
    (BackendType::Software, "Software"),
];
#[cfg(all(
    not(any(target_os = "windows", target_os = "macos")),
    not(target_pointer_width = "32")
))]
pub const VIDEO_RENDERER_OPTIONS: &[(BackendType, &str)] = &[
    (BackendType::OpenGL, "OpenGL"),
    (BackendType::Vulkan, "Vulkan"),
    (BackendType::OpenGLWgpu, "OpenGL (wgpu)"),
    (BackendType::VulkanWgpu, "Vulkan (wgpu)"),
    (BackendType::Software, "Software"),
];
#[cfg(all(not(target_os = "windows"), target_pointer_width = "32"))]
pub const VIDEO_RENDERER_OPTIONS: &[(BackendType, &str)] = &[
    (BackendType::OpenGL, "OpenGL"),
    (BackendType::OpenGLWgpu, "OpenGL (wgpu)"),
    (BackendType::Software, "Software"),
];

#[cfg(all(target_os = "windows", not(target_pointer_width = "32")))]
pub const VIDEO_RENDERER_LABELS: &[Choice] = &[
    localized_choice("OptionsGraphics", "RendererOpenGL"),
    localized_choice("OptionsGraphics", "RendererVulkan"),
    localized_choice("OptionsGraphics", "RendererDirectX"),
    localized_choice("OptionsGraphics", "RendererOpenGLWgpu"),
    localized_choice("OptionsGraphics", "RendererVulkanWgpu"),
    localized_choice("OptionsGraphics", "RendererSoftware"),
];
#[cfg(all(target_os = "windows", target_pointer_width = "32"))]
pub const VIDEO_RENDERER_LABELS: &[Choice] = &[
    localized_choice("OptionsGraphics", "RendererOpenGL"),
    localized_choice("OptionsGraphics", "RendererDirectX"),
    localized_choice("OptionsGraphics", "RendererOpenGLWgpu"),
    localized_choice("OptionsGraphics", "RendererSoftware"),
];
#[cfg(all(target_os = "macos", not(target_pointer_width = "32")))]
pub const VIDEO_RENDERER_LABELS: &[Choice] = &[
    localized_choice("OptionsGraphics", "RendererOpenGL"),
    localized_choice("OptionsGraphics", "RendererVulkan"),
    localized_choice("OptionsGraphics", "RendererMetal"),
    localized_choice("OptionsGraphics", "RendererOpenGLWgpu"),
    localized_choice("OptionsGraphics", "RendererVulkanWgpu"),
    localized_choice("OptionsGraphics", "RendererSoftware"),
];
#[cfg(all(
    not(any(target_os = "windows", target_os = "macos")),
    not(target_pointer_width = "32")
))]
pub const VIDEO_RENDERER_LABELS: &[Choice] = &[
    localized_choice("OptionsGraphics", "RendererOpenGL"),
    localized_choice("OptionsGraphics", "RendererVulkan"),
    localized_choice("OptionsGraphics", "RendererOpenGLWgpu"),
    localized_choice("OptionsGraphics", "RendererVulkanWgpu"),
    localized_choice("OptionsGraphics", "RendererSoftware"),
];
#[cfg(all(not(target_os = "windows"), target_pointer_width = "32"))]
pub const VIDEO_RENDERER_LABELS: &[Choice] = &[
    localized_choice("OptionsGraphics", "RendererOpenGL"),
    localized_choice("OptionsGraphics", "RendererOpenGLWgpu"),
    localized_choice("OptionsGraphics", "RendererSoftware"),
];

pub const DISPLAY_ASPECT_RATIO_CHOICES: &[Choice] = &[
    literal_choice("16:9"),
    literal_choice("16:10"),
    literal_choice("4:3"),
    literal_choice("1:1"),
];

pub const VIDEO_RENDERER_ROW_INDEX: usize = 0;
pub const SOFTWARE_THREADS_ROW_INDEX: usize = 1;
pub const DISPLAY_MODE_ROW_INDEX: usize = 2;
pub const DISPLAY_ASPECT_RATIO_ROW_INDEX: usize = 3;
pub const DISPLAY_RESOLUTION_ROW_INDEX: usize = 4;
pub const REFRESH_RATE_ROW_INDEX: usize = 5;
pub const FULLSCREEN_TYPE_ROW_INDEX: usize = 6;
pub const VSYNC_ROW_INDEX: usize = 7;
pub const PRESENT_MODE_ROW_INDEX: usize = 8;
pub const MAX_FPS_ENABLED_ROW_INDEX: usize = 9;
pub const MAX_FPS_VALUE_ROW_INDEX: usize = 10;
pub const SELECT_MUSIC_SHOW_BANNERS_ROW_INDEX: usize = 0;
pub const SELECT_MUSIC_SHOW_VIDEO_BANNERS_ROW_INDEX: usize = 1;
pub const SELECT_MUSIC_SHOW_BREAKDOWN_ROW_INDEX: usize = 2;
pub const SELECT_MUSIC_BREAKDOWN_STYLE_ROW_INDEX: usize = 3;
pub const SELECT_MUSIC_MUSIC_PREVIEWS_ROW_INDEX: usize = 14;
pub const SELECT_MUSIC_CHART_INFO_ROW_INDEX: usize = 13;
pub const SELECT_MUSIC_PREVIEW_LOOP_ROW_INDEX: usize = 16;
pub const SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX: usize = 18;
pub const SELECT_MUSIC_SCOREBOX_PLACEMENT_ROW_INDEX: usize = 19;
pub const SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX: usize = 20;
pub const MACHINE_SELECT_STYLE_ROW_INDEX: usize = 2;
pub const MACHINE_PREFERRED_STYLE_ROW_INDEX: usize = 3;
pub const MACHINE_SELECT_PLAY_MODE_ROW_INDEX: usize = 4;
pub const MACHINE_PREFERRED_MODE_ROW_INDEX: usize = 5;
pub const ADVANCED_SONG_PARSING_THREADS_ROW_INDEX: usize = 3;

pub const MAX_FPS_MIN: u16 = 5;
pub const MAX_FPS_MAX: u16 = 1000;
pub const MAX_FPS_STEP: u16 = 5;
pub const MAX_FPS_DEFAULT: u16 = 60;
pub const MUSIC_WHEEL_SCROLL_SPEED_VALUES: [u8; 7] = [5, 10, 15, 25, 30, 45, 100];

pub const DEFAULT_RESOLUTION_CHOICES: &[(u32, u32)] = &[
    (1920, 1080),
    (1600, 900),
    (1280, 720),
    (1024, 768),
    (800, 600),
];

pub fn build_display_mode_choices(monitor_specs: &[MonitorSpec]) -> Vec<String> {
    if monitor_specs.is_empty() {
        return vec![
            tr("OptionsGraphics", "Screen1Fallback").to_string(),
            tr("OptionsGraphics", "Windowed").to_string(),
        ];
    }
    let mut out = Vec::with_capacity(monitor_specs.len() + 1);
    for spec in monitor_specs {
        out.push(spec.name.clone());
    }
    out.push(tr("OptionsGraphics", "Windowed").to_string());
    out
}

pub const GRAPHICS_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::VideoRenderer,
        label: lookup_key("OptionsGraphics", "VideoRenderer"),
        choices: VIDEO_RENDERER_LABELS,
        inline: false,
    },
    SubRow {
        id: SubRowId::SoftwareRendererThreads,
        label: lookup_key("OptionsGraphics", "SoftwareRendererThreads"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
    },
    SubRow {
        id: SubRowId::DisplayMode,
        label: lookup_key("OptionsGraphics", "DisplayMode"),
        choices: &[
            localized_choice("OptionsGraphics", "Windowed"),
            localized_choice("OptionsGraphics", "Fullscreen"),
            localized_choice("OptionsGraphics", "Borderless"),
        ], // Replaced dynamically
        inline: true,
    },
    SubRow {
        id: SubRowId::DisplayAspectRatio,
        label: lookup_key("OptionsGraphics", "DisplayAspectRatio"),
        choices: DISPLAY_ASPECT_RATIO_CHOICES,
        inline: true,
    },
    SubRow {
        id: SubRowId::DisplayResolution,
        label: lookup_key("OptionsGraphics", "DisplayResolution"),
        choices: &[
            literal_choice("1920x1080"),
            literal_choice("1600x900"),
            literal_choice("1280x720"),
            literal_choice("1024x768"),
            literal_choice("800x600"),
        ], // Replaced dynamically
        inline: false,
    },
    SubRow {
        id: SubRowId::RefreshRate,
        label: lookup_key("OptionsGraphics", "RefreshRate"),
        choices: &[
            localized_choice("Common", "Default"),
            literal_choice("60 Hz"),
            literal_choice("75 Hz"),
            literal_choice("120 Hz"),
            literal_choice("144 Hz"),
            literal_choice("165 Hz"),
            literal_choice("240 Hz"),
            literal_choice("360 Hz"),
        ], // Replaced dynamically
        inline: false,
    },
    SubRow {
        id: SubRowId::FullscreenType,
        label: lookup_key("OptionsGraphics", "FullscreenType"),
        choices: &[
            localized_choice("OptionsGraphics", "FullscreenExclusive"),
            localized_choice("OptionsGraphics", "Borderless"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::VSync,
        label: lookup_key("OptionsGraphics", "VSync"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::PresentMode,
        label: lookup_key("OptionsGraphics", "PresentMode"),
        choices: &[literal_choice("Mailbox"), literal_choice("Immediate")],
        inline: true,
    },
    SubRow {
        id: SubRowId::MaxFps,
        label: lookup_key("OptionsGraphics", "MaxFps"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MaxFpsValue,
        label: lookup_key("OptionsGraphics", "MaxFpsValue"),
        choices: &[localized_choice("Common", "Off")], // Replaced dynamically
        inline: false,
    },
    SubRow {
        id: SubRowId::ShowStats,
        label: lookup_key("OptionsGraphics", "ShowStats"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("OptionsGraphics", "ShowStatsFPS"),
            localized_choice("OptionsGraphics", "ShowStatsFPSStutter"),
            localized_choice("OptionsGraphics", "ShowStatsFPSStutterTiming"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ValidationLayers,
        label: lookup_key("OptionsGraphics", "ValidationLayers"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::VisualDelay,
        label: lookup_key("OptionsGraphics", "VisualDelay"),
        choices: &[literal_choice("0 ms")],
        inline: false,
    },
];

pub const GRAPHICS_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::GfxVideoRenderer,
        name: lookup_key("OptionsGraphics", "VideoRenderer"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "VideoRendererHelp",
        ))],
    },
    Item {
        id: ItemId::GfxSoftwareThreads,
        name: lookup_key("OptionsGraphics", "SoftwareRendererThreads"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "SoftwareRendererThreadsHelp",
        ))],
    },
    Item {
        id: ItemId::GfxDisplayMode,
        name: lookup_key("OptionsGraphics", "DisplayMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "DisplayModeHelp",
        ))],
    },
    Item {
        id: ItemId::GfxDisplayAspectRatio,
        name: lookup_key("OptionsGraphics", "DisplayAspectRatio"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "DisplayAspectRatioHelp",
        ))],
    },
    Item {
        id: ItemId::GfxDisplayResolution,
        name: lookup_key("OptionsGraphics", "DisplayResolution"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "DisplayResolutionHelp",
        ))],
    },
    Item {
        id: ItemId::GfxRefreshRate,
        name: lookup_key("OptionsGraphics", "RefreshRate"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "RefreshRateHelp",
        ))],
    },
    Item {
        id: ItemId::GfxFullscreenType,
        name: lookup_key("OptionsGraphics", "FullscreenType"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "FullscreenTypeHelp",
        ))],
    },
    Item {
        id: ItemId::GfxVSync,
        name: lookup_key("OptionsGraphics", "VSync"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "VSyncHelp",
        ))],
    },
    Item {
        id: ItemId::GfxPresentMode,
        name: lookup_key("OptionsGraphics", "PresentMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "PresentModeHelp",
        ))],
    },
    Item {
        id: ItemId::GfxMaxFps,
        name: lookup_key("OptionsGraphics", "MaxFps"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "MaxFpsHelp",
        ))],
    },
    Item {
        id: ItemId::GfxMaxFpsValue,
        name: lookup_key("OptionsGraphics", "MaxFpsValue"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "MaxFpsValueHelp",
        ))],
    },
    Item {
        id: ItemId::GfxShowStats,
        name: lookup_key("OptionsGraphics", "ShowStats"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "ShowStatsHelp",
        ))],
    },
    Item {
        id: ItemId::GfxValidationLayers,
        name: lookup_key("OptionsGraphics", "ValidationLayers"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "ValidationLayersHelp",
        ))],
    },
    Item {
        id: ItemId::GfxVisualDelay,
        name: lookup_key("OptionsGraphics", "VisualDelay"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGraphicsHelp",
            "VisualDelayHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const INPUT_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ConfigureMappings,
        label: lookup_key("OptionsInput", "ConfigureMappings"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
    },
    SubRow {
        id: SubRowId::TestInput,
        label: lookup_key("OptionsInput", "TestInput"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
    },
    SubRow {
        id: SubRowId::InputOptions,
        label: lookup_key("OptionsInput", "InputOptions"),
        choices: &[localized_choice("Common", "Open")],
        inline: false,
    },
];

pub const INPUT_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::InpConfigureMappings,
        name: lookup_key("OptionsInput", "ConfigureMappings"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "ConfigureMappingsHelp",
        ))],
    },
    Item {
        id: ItemId::InpTestInput,
        name: lookup_key("OptionsInput", "TestInput"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "TestInputHelp",
        ))],
    },
    Item {
        id: ItemId::InpInputOptions,
        name: lookup_key("OptionsInput", "InputOptions"),
        help: &[
            HelpEntry::Paragraph(lookup_key("OptionsInputHelp", "InputOptionsHelp")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "GamepadBackend")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "MenuNavigation")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "OptionsNavigation")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "MenuButtons")),
            HelpEntry::Bullet(lookup_key("OptionsInput", "Debounce")),
        ],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const INPUT_BACKEND_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::GamepadBackend,
        label: lookup_key("OptionsInput", "GamepadBackend"),
        choices: INPUT_BACKEND_CHOICES,
        inline: INPUT_BACKEND_INLINE,
    },
    SubRow {
        id: SubRowId::MenuNavigation,
        label: lookup_key("OptionsInput", "MenuNavigation"),
        choices: &[
            localized_choice("OptionsInput", "MenuNavigationFiveKey"),
            localized_choice("OptionsInput", "MenuNavigationThreeKey"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::OptionsNavigation,
        label: lookup_key("OptionsInput", "OptionsNavigation"),
        choices: &[
            localized_choice("OptionsInput", "OptionsNavigationStepMania"),
            localized_choice("OptionsInput", "OptionsNavigationArcade"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MenuButtons,
        label: lookup_key("OptionsInput", "MenuButtons"),
        choices: &[
            localized_choice("OptionsInput", "DedicatedMenuButtonsGameplay"),
            localized_choice("OptionsInput", "DedicatedMenuButtonsOnly"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::Debounce,
        label: lookup_key("OptionsInput", "Debounce"),
        choices: &[literal_choice("20ms")],
        inline: true,
    },
];

pub const INPUT_BACKEND_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::InpGamepadBackend,
        name: lookup_key("OptionsInput", "GamepadBackend"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "GamepadBackendHelp",
        ))],
    },
    Item {
        id: ItemId::InpMenuButtons,
        name: lookup_key("OptionsInput", "MenuButtons"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuButtonsHelp",
        ))],
    },
    Item {
        id: ItemId::InpOptionsNavigation,
        name: lookup_key("OptionsInput", "OptionsNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "OptionsNavigationHelp",
        ))],
    },
    Item {
        id: ItemId::InpMenuNavigation,
        name: lookup_key("OptionsInput", "MenuNavigation"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "MenuNavigationHelp",
        ))],
    },
    Item {
        id: ItemId::InpDebounce,
        name: lookup_key("OptionsInput", "Debounce"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsInputHelp",
            "DebounceHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const MACHINE_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::SelectProfile,
        label: lookup_key("OptionsMachine", "SelectProfile"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::SelectColor,
        label: lookup_key("OptionsMachine", "SelectColor"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::SelectStyle,
        label: lookup_key("OptionsMachine", "SelectStyle"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::PreferredStyle,
        label: lookup_key("OptionsMachine", "PreferredStyle"),
        choices: &[
            localized_choice("OptionsMachine", "PreferredStyleSingle"),
            localized_choice("OptionsMachine", "PreferredStyleVersus"),
            localized_choice("OptionsMachine", "PreferredStyleDouble"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::SelectPlayMode,
        label: lookup_key("OptionsMachine", "SelectPlayMode"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::PreferredMode,
        label: lookup_key("OptionsMachine", "PreferredMode"),
        choices: &[
            localized_choice("OptionsMachine", "PreferredModeRegular"),
            localized_choice("OptionsMachine", "PreferredModeMarathon"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::EvalSummary,
        label: lookup_key("OptionsMachine", "EvalSummary"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::NameEntry,
        label: lookup_key("OptionsMachine", "NameEntry"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::GameoverScreen,
        label: lookup_key("OptionsMachine", "GameoverScreen"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::WriteCurrentScreen,
        label: lookup_key("OptionsMachine", "WriteCurrentScreen"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MenuMusic,
        label: lookup_key("OptionsMachine", "MenuMusic"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::Replays,
        label: lookup_key("OptionsMachine", "Replays"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::PerPlayerGlobalOffsets,
        label: lookup_key("OptionsMachine", "PerPlayerGlobalOffsets"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::KeyboardFeatures,
        label: lookup_key("OptionsMachine", "KeyboardFeatures"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::VideoBgs,
        label: lookup_key("OptionsMachine", "VideoBgs"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
];

pub const MACHINE_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::MchSelectProfile,
        name: lookup_key("OptionsMachine", "SelectProfile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectProfileHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectColor,
        name: lookup_key("OptionsMachine", "SelectColor"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectColorHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectStyle,
        name: lookup_key("OptionsMachine", "SelectStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectStyleHelp",
        ))],
    },
    Item {
        id: ItemId::MchPreferredStyle,
        name: lookup_key("OptionsMachine", "PreferredStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PreferredStyleHelp",
        ))],
    },
    Item {
        id: ItemId::MchSelectPlayMode,
        name: lookup_key("OptionsMachine", "SelectPlayMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "SelectPlayModeHelp",
        ))],
    },
    Item {
        id: ItemId::MchPreferredMode,
        name: lookup_key("OptionsMachine", "PreferredMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PreferredModeHelp",
        ))],
    },
    Item {
        id: ItemId::MchEvalSummary,
        name: lookup_key("OptionsMachine", "EvalSummary"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "EvalSummaryHelp",
        ))],
    },
    Item {
        id: ItemId::MchNameEntry,
        name: lookup_key("OptionsMachine", "NameEntry"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "NameEntryHelp",
        ))],
    },
    Item {
        id: ItemId::MchGameoverScreen,
        name: lookup_key("OptionsMachine", "GameoverScreen"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "GameoverScreenHelp",
        ))],
    },
    Item {
        id: ItemId::MchWriteCurrentScreen,
        name: lookup_key("OptionsMachine", "WriteCurrentScreen"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "WriteCurrentScreenHelp",
        ))],
    },
    Item {
        id: ItemId::MchMenuMusic,
        name: lookup_key("OptionsMachine", "MenuMusic"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "MenuMusicHelp",
        ))],
    },
    Item {
        id: ItemId::MchReplays,
        name: lookup_key("OptionsMachine", "Replays"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "ReplaysHelp",
        ))],
    },
    Item {
        id: ItemId::MchPerPlayerGlobalOffsets,
        name: lookup_key("OptionsMachine", "PerPlayerGlobalOffsets"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "PerPlayerGlobalOffsetsHelp",
        ))],
    },
    Item {
        id: ItemId::MchKeyboardFeatures,
        name: lookup_key("OptionsMachine", "KeyboardFeatures"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "KeyboardFeaturesHelp",
        ))],
    },
    Item {
        id: ItemId::MchVideoBgs,
        name: lookup_key("OptionsMachine", "VideoBgs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsMachineHelp",
            "VideoBgsHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const COURSE_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ShowRandomCourses,
        label: lookup_key("OptionsCourse", "ShowRandomCourses"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowMostPlayed,
        label: lookup_key("OptionsCourse", "ShowMostPlayed"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowIndividualScores,
        label: lookup_key("OptionsCourse", "ShowIndividualScores"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::AutosubmitIndividual,
        label: lookup_key("OptionsCourse", "AutosubmitIndividual"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
];

pub const COURSE_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::CrsShowRandom,
        name: lookup_key("OptionsCourse", "ShowRandomCourses"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowRandomCoursesHelp",
        ))],
    },
    Item {
        id: ItemId::CrsShowMostPlayed,
        name: lookup_key("OptionsCourse", "ShowMostPlayed"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowMostPlayedHelp",
        ))],
    },
    Item {
        id: ItemId::CrsShowIndividualScores,
        name: lookup_key("OptionsCourse", "ShowIndividualScores"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowIndividualScoresHelp",
        ))],
    },
    Item {
        id: ItemId::CrsAutosubmitIndividual,
        name: lookup_key("OptionsCourse", "AutosubmitIndividual"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "AutosubmitIndividualHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const GAMEPLAY_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::BgBrightness,
        label: lookup_key("OptionsGameplay", "BgBrightness"),
        choices: &[
            literal_choice("0%"),
            literal_choice("10%"),
            literal_choice("20%"),
            literal_choice("30%"),
            literal_choice("40%"),
            literal_choice("50%"),
            literal_choice("60%"),
            literal_choice("70%"),
            literal_choice("80%"),
            literal_choice("90%"),
            literal_choice("100%"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::CenteredP1Notefield,
        label: lookup_key("OptionsGameplay", "CenteredP1Notefield"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ZmodRatingBox,
        label: lookup_key("OptionsGameplay", "ZmodRatingBox"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::BpmDecimal,
        label: lookup_key("OptionsGameplay", "BpmDecimal"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::AutoScreenshot,
        label: lookup_key("OptionsGameplay", "AutoScreenshot"),
        choices: &[
            literal_choice("PBs"),
            literal_choice("Fails"),
            literal_choice("Clears"),
            literal_choice("Quads"),
            literal_choice("Quints"),
        ],
        inline: true,
    },
];

pub const GAMEPLAY_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::GpBgBrightness,
        name: lookup_key("OptionsGameplay", "BgBrightness"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGameplayHelp",
            "BgBrightnessHelp",
        ))],
    },
    Item {
        id: ItemId::GpCenteredP1,
        name: lookup_key("OptionsGameplay", "CenteredP1Notefield"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGameplayHelp",
            "CenteredP1NotefieldHelp",
        ))],
    },
    Item {
        id: ItemId::GpZmodRatingBox,
        name: lookup_key("OptionsGameplay", "ZmodRatingBox"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGameplayHelp",
            "ZmodRatingBoxHelp",
        ))],
    },
    Item {
        id: ItemId::GpBpmDecimal,
        name: lookup_key("OptionsGameplay", "BpmDecimal"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGameplayHelp",
            "BpmDecimalHelp",
        ))],
    },
    Item {
        id: ItemId::GpAutoScreenshot,
        name: lookup_key("OptionsGameplay", "AutoScreenshot"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGameplayHelp",
            "AutoScreenshotHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const SOUND_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::SoundDevice,
        label: lookup_key("OptionsSound", "SoundDevice"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
    },
    SubRow {
        id: SubRowId::AudioOutputMode,
        label: lookup_key("OptionsSound", "AudioOutputMode"),
        choices: &[
            localized_choice("OptionsSound", "OutputModeAuto"),
            localized_choice("OptionsSound", "OutputModeShared"),
        ],
        inline: false,
    },
    #[cfg(target_os = "linux")]
    SubRow {
        id: SubRowId::LinuxAudioBackend,
        label: lookup_key("OptionsSound", "LinuxAudioBackend"),
        choices: SOUND_LINUX_BACKEND_CHOICES,
        inline: false,
    },
    #[cfg(target_os = "linux")]
    SubRow {
        id: SubRowId::AlsaExclusive,
        label: lookup_key("OptionsSound", "AlsaExclusive"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::AudioSampleRate,
        label: lookup_key("OptionsSound", "AudioSampleRate"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
    },
    SubRow {
        id: SubRowId::MasterVolume,
        label: lookup_key("OptionsSound", "MasterVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
    },
    SubRow {
        id: SubRowId::SfxVolume,
        label: lookup_key("OptionsSound", "SfxVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
    },
    SubRow {
        id: SubRowId::AssistTickVolume,
        label: lookup_key("OptionsSound", "AssistTickVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
    },
    SubRow {
        id: SubRowId::MusicVolume,
        label: lookup_key("OptionsSound", "MusicVolume"),
        choices: &[literal_choice("100%")],
        inline: false,
    },
    SubRow {
        id: SubRowId::MineSounds,
        label: lookup_key("OptionsSound", "MineSounds"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::GlobalOffset,
        label: lookup_key("OptionsSound", "GlobalOffset"),
        choices: &[literal_choice("0 ms")],
        inline: false,
    },
    SubRow {
        id: SubRowId::RateModPreservesPitch,
        label: lookup_key("OptionsSound", "RateModPreservesPitch"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
];

pub const SOUND_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SndDevice,
        name: lookup_key("OptionsSound", "SoundDevice"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "SoundDeviceHelp",
        ))],
    },
    Item {
        id: ItemId::SndOutputMode,
        name: lookup_key("OptionsSound", "AudioOutputMode"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AudioOutputModeHelp",
        ))],
    },
    #[cfg(target_os = "linux")]
    Item {
        id: ItemId::SndLinuxBackend,
        name: lookup_key("OptionsSound", "LinuxAudioBackend"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "LinuxAudioBackendHelp",
        ))],
    },
    #[cfg(target_os = "linux")]
    Item {
        id: ItemId::SndAlsaExclusive,
        name: lookup_key("OptionsSound", "AlsaExclusive"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AlsaExclusiveHelp",
        ))],
    },
    Item {
        id: ItemId::SndSampleRate,
        name: lookup_key("OptionsSound", "AudioSampleRate"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AudioSampleRateHelp",
        ))],
    },
    Item {
        id: ItemId::SndMasterVolume,
        name: lookup_key("OptionsSound", "MasterVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MasterVolumeHelp",
        ))],
    },
    Item {
        id: ItemId::SndSfxVolume,
        name: lookup_key("OptionsSound", "SfxVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "SfxVolumeHelp",
        ))],
    },
    Item {
        id: ItemId::SndAssistTickVolume,
        name: lookup_key("OptionsSound", "AssistTickVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "AssistTickVolumeHelp",
        ))],
    },
    Item {
        id: ItemId::SndMusicVolume,
        name: lookup_key("OptionsSound", "MusicVolume"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MusicVolumeHelp",
        ))],
    },
    Item {
        id: ItemId::SndMineSounds,
        name: lookup_key("OptionsSound", "MineSounds"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "MineSoundsHelp",
        ))],
    },
    Item {
        id: ItemId::SndGlobalOffset,
        name: lookup_key("OptionsSound", "GlobalOffset"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "GlobalOffsetHelp",
        ))],
    },
    Item {
        id: ItemId::SndRateModPitch,
        name: lookup_key("OptionsSound", "RateModPreservesPitch"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSoundHelp",
            "RateModPreservesPitchHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const SELECT_MUSIC_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ShowBanners,
        label: lookup_key("OptionsSelectMusic", "ShowBanners"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowVideoBanners,
        label: lookup_key("OptionsSelectMusic", "ShowVideoBanners"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowBreakdown,
        label: lookup_key("OptionsSelectMusic", "ShowBreakdown"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::BreakdownStyle,
        label: lookup_key("OptionsSelectMusic", "BreakdownStyle"),
        choices: &[literal_choice("SL"), literal_choice("SN")],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowNativeLanguage,
        label: lookup_key("OptionsSelectMusic", "ShowNativeLanguage"),
        choices: &[
            localized_choice("OptionsSelectMusic", "NativeLanguageTranslit"),
            localized_choice("OptionsSelectMusic", "NativeLanguageNative"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MusicWheelSpeed,
        label: lookup_key("OptionsSelectMusic", "MusicWheelSpeed"),
        choices: &[
            localized_choice("OptionsSelectMusic", "WheelSpeedSlow"),
            localized_choice("OptionsSelectMusic", "WheelSpeedNormal"),
            localized_choice("OptionsSelectMusic", "WheelSpeedFast"),
            localized_choice("OptionsSelectMusic", "WheelSpeedFaster"),
            localized_choice("OptionsSelectMusic", "WheelSpeedRidiculous"),
            localized_choice("OptionsSelectMusic", "WheelSpeedLudicrous"),
            localized_choice("OptionsSelectMusic", "WheelSpeedPlaid"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MusicWheelStyle,
        label: lookup_key("OptionsSelectMusic", "MusicWheelStyle"),
        choices: &[literal_choice("ITG"), literal_choice("IIDX")],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowCdTitles,
        label: lookup_key("OptionsSelectMusic", "ShowCdTitles"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowWheelGrades,
        label: lookup_key("OptionsSelectMusic", "ShowWheelGrades"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowWheelLamps,
        label: lookup_key("OptionsSelectMusic", "ShowWheelLamps"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ItlWheelData,
        label: lookup_key("OptionsSelectMusic", "ItlWheelData"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("OptionsSelectMusic", "ItlWheelScore"),
            localized_choice("OptionsSelectMusic", "ItlWheelPointsScore"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::NewPackBadge,
        label: lookup_key("OptionsSelectMusic", "NewPackBadge"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("OptionsSelectMusic", "NewPackOpenPack"),
            localized_choice("OptionsSelectMusic", "NewPackHasScore"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowPatternInfo,
        label: lookup_key("OptionsSelectMusic", "ShowPatternInfo"),
        choices: &[
            localized_choice("Common", "Auto"),
            localized_choice("OptionsSelectMusic", "PatternInfoTech"),
            localized_choice("OptionsSelectMusic", "PatternInfoStamina"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ChartInfo,
        label: lookup_key("OptionsSelectMusic", "ChartInfo"),
        choices: &[
            localized_choice("OptionsSelectMusic", "ChartInfoPeakNPS"),
            localized_choice("OptionsSelectMusic", "ChartInfoMatrixRating"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::MusicPreviews,
        label: lookup_key("OptionsSelectMusic", "MusicPreviews"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::PreviewMarker,
        label: lookup_key("OptionsSelectMusic", "PreviewMarker"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::LoopMusic,
        label: lookup_key("OptionsSelectMusic", "LoopMusic"),
        choices: &[
            localized_choice("OptionsSelectMusic", "LoopMusicPlayOnce"),
            localized_choice("OptionsSelectMusic", "LoopMusicLoop"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowGameplayTimer,
        label: lookup_key("OptionsSelectMusic", "ShowGameplayTimer"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ShowGsBox,
        label: lookup_key("OptionsSelectMusic", "ShowGsBox"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::GsBoxPlacement,
        label: lookup_key("OptionsSelectMusic", "GsBoxPlacement"),
        choices: &[
            localized_choice("Common", "Auto"),
            localized_choice("OptionsSelectMusic", "GsBoxStepPane"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::GsBoxLeaderboards,
        label: lookup_key("OptionsSelectMusic", "GsBoxLeaderboards"),
        choices: &[
            localized_choice("OptionsSelectMusic", "ScoreboxCycleITG"),
            localized_choice("OptionsSelectMusic", "ScoreboxCycleEX"),
            localized_choice("OptionsSelectMusic", "ScoreboxCycleHEX"),
            localized_choice("OptionsSelectMusic", "ScoreboxCycleTournaments"),
        ],
        inline: true,
    },
];

pub const SELECT_MUSIC_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SmShowBanners,
        name: lookup_key("OptionsSelectMusic", "ShowBanners"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowBannersHelp",
        ))],
    },
    Item {
        id: ItemId::SmShowVideoBanners,
        name: lookup_key("OptionsSelectMusic", "ShowVideoBanners"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowVideoBannersHelp",
        ))],
    },
    Item {
        id: ItemId::SmShowBreakdown,
        name: lookup_key("OptionsSelectMusic", "ShowBreakdown"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowBreakdownHelp",
        ))],
    },
    Item {
        id: ItemId::SmBreakdownStyle,
        name: lookup_key("OptionsSelectMusic", "BreakdownStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "BreakdownStyleHelp",
        ))],
    },
    Item {
        id: ItemId::SmNativeLanguage,
        name: lookup_key("OptionsSelectMusic", "ShowNativeLanguage"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowNativeLanguageHelp",
        ))],
    },
    Item {
        id: ItemId::SmWheelSpeed,
        name: lookup_key("OptionsSelectMusic", "MusicWheelSpeed"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "MusicWheelSpeedHelp",
        ))],
    },
    Item {
        id: ItemId::SmWheelStyle,
        name: lookup_key("OptionsSelectMusic", "MusicWheelStyle"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "MusicWheelStyleHelp",
        ))],
    },
    Item {
        id: ItemId::SmCdTitles,
        name: lookup_key("OptionsSelectMusic", "ShowCdTitles"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowCdTitlesHelp",
        ))],
    },
    Item {
        id: ItemId::SmWheelGrades,
        name: lookup_key("OptionsSelectMusic", "ShowWheelGrades"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowWheelGradesHelp",
        ))],
    },
    Item {
        id: ItemId::SmWheelLamps,
        name: lookup_key("OptionsSelectMusic", "ShowWheelLamps"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowWheelLampsHelp",
        ))],
    },
    Item {
        id: ItemId::SmWheelItl,
        name: lookup_key("OptionsSelectMusic", "ItlWheelData"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ItlWheelDataHelp",
        ))],
    },
    Item {
        id: ItemId::SmNewPackBadge,
        name: lookup_key("OptionsSelectMusic", "NewPackBadge"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "NewPackBadgeHelp",
        ))],
    },
    Item {
        id: ItemId::SmPatternInfo,
        name: lookup_key("OptionsSelectMusic", "ShowPatternInfo"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowPatternInfoHelp",
        ))],
    },
    Item {
        id: ItemId::SmChartInfo,
        name: lookup_key("OptionsSelectMusic", "ChartInfo"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ChartInfoHelp",
        ))],
    },
    Item {
        id: ItemId::SmPreviews,
        name: lookup_key("OptionsSelectMusic", "MusicPreviews"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "MusicPreviewsHelp",
        ))],
    },
    Item {
        id: ItemId::SmPreviewMarker,
        name: lookup_key("OptionsSelectMusic", "PreviewMarker"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "PreviewMarkerHelp",
        ))],
    },
    Item {
        id: ItemId::SmPreviewLoop,
        name: lookup_key("OptionsSelectMusic", "LoopMusic"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "LoopMusicHelp",
        ))],
    },
    Item {
        id: ItemId::SmGameplayTimer,
        name: lookup_key("OptionsSelectMusic", "ShowGameplayTimer"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowGameplayTimerHelp",
        ))],
    },
    Item {
        id: ItemId::SmShowRivals,
        name: lookup_key("OptionsSelectMusic", "ShowGsBox"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "ShowGsBoxHelp",
        ))],
    },
    Item {
        id: ItemId::SmScoreboxPlacement,
        name: lookup_key("OptionsSelectMusic", "GsBoxPlacement"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "GsBoxPlacementHelp",
        ))],
    },
    Item {
        id: ItemId::SmScoreboxCycle,
        name: lookup_key("OptionsSelectMusic", "GsBoxLeaderboards"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSelectMusicHelp",
            "GsBoxLeaderboardsHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const ADVANCED_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::DefaultFailType,
        label: lookup_key("OptionsAdvanced", "DefaultFailType"),
        choices: &[
            localized_choice("OptionsAdvanced", "FailImmediate"),
            localized_choice("OptionsAdvanced", "FailImmediateContinue"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::BannerCache,
        label: lookup_key("OptionsAdvanced", "BannerCache"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::CdTitleCache,
        label: lookup_key("OptionsAdvanced", "CdTitleCache"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::SongParsingThreads,
        label: lookup_key("OptionsAdvanced", "SongParsingThreads"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
    },
    SubRow {
        id: SubRowId::CacheSongs,
        label: lookup_key("OptionsAdvanced", "CacheSongs"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::FastLoad,
        label: lookup_key("OptionsAdvanced", "FastLoad"),
        choices: &[
            localized_choice("Common", "Off"),
            localized_choice("Common", "On"),
        ],
        inline: true,
    },
];

pub const ADVANCED_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::AdvDefaultFailType,
        name: lookup_key("OptionsAdvanced", "DefaultFailType"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "DefaultFailTypeHelp",
        ))],
    },
    Item {
        id: ItemId::AdvBannerCache,
        name: lookup_key("OptionsAdvanced", "BannerCache"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "BannerCacheHelp",
        ))],
    },
    Item {
        id: ItemId::AdvCdTitleCache,
        name: lookup_key("OptionsAdvanced", "CdTitleCache"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "CdTitleCacheHelp",
        ))],
    },
    Item {
        id: ItemId::AdvSongParsingThreads,
        name: lookup_key("OptionsAdvanced", "SongParsingThreads"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "SongParsingThreadsHelp",
        ))],
    },
    Item {
        id: ItemId::AdvCacheSongs,
        name: lookup_key("OptionsAdvanced", "CacheSongs"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "CacheSongsHelp",
        ))],
    },
    Item {
        id: ItemId::AdvFastLoad,
        name: lookup_key("OptionsAdvanced", "FastLoad"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsAdvancedHelp",
            "FastLoadHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const GROOVESTATS_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::EnableGrooveStats,
        label: lookup_key("OptionsGrooveStats", "EnableGrooveStats"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::EnableBoogieStats,
        label: lookup_key("OptionsGrooveStats", "EnableBoogieStats"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::GsSubmitFails,
        label: lookup_key("OptionsGrooveStats", "GsSubmitFails"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::AutoPopulateScores,
        label: lookup_key("OptionsGrooveStats", "AutoPopulateScores"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::AutoDownloadUnlocks,
        label: lookup_key("OptionsGrooveStats", "AutoDownloadUnlocks"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::SeparateUnlocksByPlayer,
        label: lookup_key("OptionsGrooveStats", "SeparateUnlocksByPlayer"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
];

pub const ARROWCLOUD_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::EnableArrowCloud,
        label: lookup_key("OptionsGrooveStats", "EnableArrowCloud"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ArrowCloudSubmitFails,
        label: lookup_key("OptionsGrooveStats", "ArrowCloudSubmitFails"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
];

pub const ONLINE_SCORING_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::GsBsOptions,
        label: lookup_key("OptionsOnlineScoring", "GsBsOptions"),
        choices: &[],
        inline: false,
    },
    SubRow {
        id: SubRowId::ArrowCloudOptions,
        label: lookup_key("OptionsOnlineScoring", "ArrowCloudOptions"),
        choices: &[],
        inline: false,
    },
    SubRow {
        id: SubRowId::ScoreImport,
        label: lookup_key("OptionsOnlineScoring", "ScoreImport"),
        choices: &[],
        inline: false,
    },
];

pub const NULL_OR_DIE_MENU_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::NullOrDieOptions,
        label: lookup_key("OptionsOnlineScoring", "NullOrDieOptions"),
        choices: &[],
        inline: false,
    },
    SubRow {
        id: SubRowId::SyncPacks,
        label: lookup_key("OptionsOnlineScoring", "SyncPacks"),
        choices: &[],
        inline: false,
    },
];

pub const NULL_OR_DIE_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::SyncGraph,
        label: lookup_key("OptionsNullOrDie", "SyncGraph"),
        choices: &[
            localized_choice("OptionsNullOrDie", "SyncGraphFrequency"),
            localized_choice("OptionsNullOrDie", "SyncGraphBeatIndex"),
            localized_choice("OptionsNullOrDie", "SyncGraphPostKernel"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::SyncConfidence,
        label: lookup_key("OptionsNullOrDie", "SyncConfidence"),
        choices: &[
            literal_choice("0%"),
            literal_choice("5%"),
            literal_choice("10%"),
            literal_choice("15%"),
            literal_choice("20%"),
            literal_choice("25%"),
            literal_choice("30%"),
            literal_choice("35%"),
            literal_choice("40%"),
            literal_choice("45%"),
            literal_choice("50%"),
            literal_choice("55%"),
            literal_choice("60%"),
            literal_choice("65%"),
            literal_choice("70%"),
            literal_choice("75%"),
            literal_choice("80%"),
            literal_choice("85%"),
            literal_choice("90%"),
            literal_choice("95%"),
            literal_choice("100%"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::PackSyncThreads,
        label: lookup_key("OptionsNullOrDie", "PackSyncThreads"),
        choices: &[localized_choice("Common", "Auto")],
        inline: false,
    },
    SubRow {
        id: SubRowId::Fingerprint,
        label: lookup_key("OptionsNullOrDie", "Fingerprint"),
        choices: &[literal_choice("50.0 ms")],
        inline: false,
    },
    SubRow {
        id: SubRowId::Window,
        label: lookup_key("OptionsNullOrDie", "Window"),
        choices: &[literal_choice("10.0 ms")],
        inline: false,
    },
    SubRow {
        id: SubRowId::Step,
        label: lookup_key("OptionsNullOrDie", "Step"),
        choices: &[literal_choice("0.2 ms")],
        inline: false,
    },
    SubRow {
        id: SubRowId::MagicOffset,
        label: lookup_key("OptionsNullOrDie", "MagicOffset"),
        choices: &[literal_choice("0.0 ms")],
        inline: false,
    },
    SubRow {
        id: SubRowId::KernelTarget,
        label: lookup_key("OptionsNullOrDie", "KernelTarget"),
        choices: &[
            localized_choice("OptionsNullOrDie", "KernelTargetDigest"),
            localized_choice("OptionsNullOrDie", "KernelTargetAccumulator"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::KernelType,
        label: lookup_key("OptionsNullOrDie", "KernelType"),
        choices: &[
            localized_choice("OptionsNullOrDie", "KernelTypeRising"),
            localized_choice("OptionsNullOrDie", "KernelTypeLoudest"),
        ],
        inline: false,
    },
    SubRow {
        id: SubRowId::FullSpectrogram,
        label: lookup_key("OptionsNullOrDie", "FullSpectrogram"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: false,
    },
];

pub const SYNC_PACK_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::SyncPackPack,
        label: lookup_key("OptionsSyncPack", "SyncPackPack"),
        choices: &[localized_choice("OptionsSyncPack", "AllPacks")],
        inline: false,
    },
    SubRow {
        id: SubRowId::SyncPackStart,
        label: lookup_key("OptionsSyncPack", "SyncPackStart"),
        choices: &[localized_choice("Common", "Start")],
        inline: false,
    },
];

pub const SCORE_IMPORT_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: SubRowId::ScoreImportEndpoint,
        label: lookup_key("OptionsScoreImport", "ScoreImportEndpoint"),
        choices: &[
            literal_choice("GrooveStats"),
            literal_choice("BoogieStats"),
            literal_choice("ArrowCloud"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ScoreImportProfile,
        label: lookup_key("OptionsScoreImport", "ScoreImportProfile"),
        choices: &[localized_choice("OptionsScoreImport", "NoEligibleProfiles")],
        inline: false,
    },
    SubRow {
        id: SubRowId::ScoreImportPack,
        label: lookup_key("OptionsScoreImport", "ScoreImportPack"),
        choices: &[localized_choice("OptionsScoreImport", "AllPacks")],
        inline: false,
    },
    SubRow {
        id: SubRowId::ScoreImportOnlyMissing,
        label: lookup_key("OptionsScoreImport", "ScoreImportOnlyMissing"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
    },
    SubRow {
        id: SubRowId::ScoreImportStart,
        label: lookup_key("OptionsScoreImport", "ScoreImportStart"),
        choices: &[localized_choice("Common", "Start")],
        inline: false,
    },
];

pub const GROOVESTATS_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::GsEnable,
        name: lookup_key("OptionsGrooveStats", "EnableGrooveStats"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableGrooveStatsHelp",
        ))],
    },
    Item {
        id: ItemId::GsEnableBoogie,
        name: lookup_key("OptionsGrooveStats", "EnableBoogieStats"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableBoogieStatsHelp",
        ))],
    },
    Item {
        id: ItemId::GsSubmitFails,
        name: lookup_key("OptionsGrooveStats", "GsSubmitFails"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "GsSubmitFailsHelp",
        ))],
    },
    Item {
        id: ItemId::GsAutoPopulate,
        name: lookup_key("OptionsGrooveStats", "AutoPopulateScores"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "AutoPopulateScoresHelp",
        ))],
    },
    Item {
        id: ItemId::GsAutoDownloadUnlocks,
        name: lookup_key("OptionsGrooveStats", "AutoDownloadUnlocks"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "AutoDownloadUnlocksHelp",
        ))],
    },
    Item {
        id: ItemId::GsSeparateUnlocks,
        name: lookup_key("OptionsGrooveStats", "SeparateUnlocksByPlayer"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "SeparateUnlocksByPlayerHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const ARROWCLOUD_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::AcEnable,
        name: lookup_key("OptionsGrooveStats", "EnableArrowCloud"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "EnableArrowCloudHelp",
        ))],
    },
    Item {
        id: ItemId::AcSubmitFails,
        name: lookup_key("OptionsGrooveStats", "ArrowCloudSubmitFails"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsGrooveStatsHelp",
            "ArrowCloudSubmitFailsHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const ONLINE_SCORING_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::OsGsBsOptions,
        name: lookup_key("OptionsOnlineScoring", "GsBsOptions"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "GsBsOptionsHelp",
        ))],
    },
    Item {
        id: ItemId::OsArrowCloudOptions,
        name: lookup_key("OptionsOnlineScoring", "ArrowCloudOptions"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "ArrowCloudOptionsHelp",
        ))],
    },
    Item {
        id: ItemId::OsScoreImport,
        name: lookup_key("OptionsOnlineScoring", "ScoreImport"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "ScoreImportHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const NULL_OR_DIE_MENU_ITEMS: &[Item] = &[
    Item {
        id: ItemId::NodOptions,
        name: lookup_key("OptionsOnlineScoring", "NullOrDieOptions"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "NullOrDieOptionsHelp",
        ))],
    },
    Item {
        id: ItemId::NodSyncPacks,
        name: lookup_key("OptionsOnlineScoring", "SyncPacks"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsOnlineScoringHelp",
            "SyncPacksHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const NULL_OR_DIE_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::NodSyncGraph,
        name: lookup_key("OptionsNullOrDie", "SyncGraph"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "SyncGraphHelp",
        ))],
    },
    Item {
        id: ItemId::NodSyncConfidence,
        name: lookup_key("OptionsNullOrDie", "SyncConfidence"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "SyncConfidenceHelp",
        ))],
    },
    Item {
        id: ItemId::NodPackSyncThreads,
        name: lookup_key("OptionsNullOrDie", "PackSyncThreads"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "PackSyncThreadsHelp",
        ))],
    },
    Item {
        id: ItemId::NodFingerprint,
        name: lookup_key("OptionsNullOrDie", "Fingerprint"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "FingerprintHelp",
        ))],
    },
    Item {
        id: ItemId::NodWindow,
        name: lookup_key("OptionsNullOrDie", "Window"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "WindowHelp",
        ))],
    },
    Item {
        id: ItemId::NodStep,
        name: lookup_key("OptionsNullOrDie", "Step"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "StepHelp",
        ))],
    },
    Item {
        id: ItemId::NodMagicOffset,
        name: lookup_key("OptionsNullOrDie", "MagicOffset"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "MagicOffsetHelp",
        ))],
    },
    Item {
        id: ItemId::NodKernelTarget,
        name: lookup_key("OptionsNullOrDie", "KernelTarget"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "KernelTargetHelp",
        ))],
    },
    Item {
        id: ItemId::NodKernelType,
        name: lookup_key("OptionsNullOrDie", "KernelType"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "KernelTypeHelp",
        ))],
    },
    Item {
        id: ItemId::NodFullSpectrogram,
        name: lookup_key("OptionsNullOrDie", "FullSpectrogram"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsNullOrDieHelp",
            "FullSpectrogramHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const SYNC_PACK_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SpPack,
        name: lookup_key("OptionsSyncPack", "SyncPackPack"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSyncPackHelp",
            "SyncPackPackHelp",
        ))],
    },
    Item {
        id: ItemId::SpStart,
        name: lookup_key("OptionsSyncPack", "SyncPackStart"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsSyncPackHelp",
            "SyncPackStartHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

pub const SCORE_IMPORT_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: ItemId::SiEndpoint,
        name: lookup_key("OptionsScoreImport", "ScoreImportEndpoint"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportEndpointHelp",
        ))],
    },
    Item {
        id: ItemId::SiProfile,
        name: lookup_key("OptionsScoreImport", "ScoreImportProfile"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportProfileHelp",
        ))],
    },
    Item {
        id: ItemId::SiPack,
        name: lookup_key("OptionsScoreImport", "ScoreImportPack"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportPackHelp",
        ))],
    },
    Item {
        id: ItemId::SiOnlyMissing,
        name: lookup_key("OptionsScoreImport", "ScoreImportOnlyMissing"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportOnlyMissingHelp",
        ))],
    },
    Item {
        id: ItemId::SiStart,
        name: lookup_key("OptionsScoreImport", "ScoreImportStart"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsScoreImportHelp",
            "ScoreImportStartHelp",
        ))],
    },
    Item {
        id: ItemId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];

/// Returns `true` when the given submenu row should be treated as disabled
/// (non-interactive and visually dimmed). Add new cases here for any row
/// that should be conditionally locked based on runtime state.
pub fn is_submenu_row_disabled(kind: SubmenuKind, id: SubRowId) -> bool {
    match (kind, id) {
        (SubmenuKind::InputBackend, SubRowId::MenuButtons) => {
            !crate::engine::input::any_player_has_dedicated_menu_buttons_for_mode(
                config::get().three_key_navigation,
            )
        }
        _ => false,
    }
}

pub const fn submenu_rows(kind: SubmenuKind) -> &'static [SubRow] {
    match kind {
        SubmenuKind::System => SYSTEM_OPTIONS_ROWS,
        SubmenuKind::Graphics => GRAPHICS_OPTIONS_ROWS,
        SubmenuKind::Input => INPUT_OPTIONS_ROWS,
        SubmenuKind::InputBackend => INPUT_BACKEND_OPTIONS_ROWS,
        SubmenuKind::OnlineScoring => ONLINE_SCORING_OPTIONS_ROWS,
        SubmenuKind::NullOrDie => NULL_OR_DIE_MENU_ROWS,
        SubmenuKind::NullOrDieOptions => NULL_OR_DIE_OPTIONS_ROWS,
        SubmenuKind::SyncPacks => SYNC_PACK_OPTIONS_ROWS,
        SubmenuKind::Machine => MACHINE_OPTIONS_ROWS,
        SubmenuKind::Advanced => ADVANCED_OPTIONS_ROWS,
        SubmenuKind::Course => COURSE_OPTIONS_ROWS,
        SubmenuKind::Gameplay => GAMEPLAY_OPTIONS_ROWS,
        SubmenuKind::Sound => SOUND_OPTIONS_ROWS,
        SubmenuKind::SelectMusic => SELECT_MUSIC_OPTIONS_ROWS,
        SubmenuKind::GrooveStats => GROOVESTATS_OPTIONS_ROWS,
        SubmenuKind::ArrowCloud => ARROWCLOUD_OPTIONS_ROWS,
        SubmenuKind::ScoreImport => SCORE_IMPORT_OPTIONS_ROWS,
    }
}

pub const fn submenu_items(kind: SubmenuKind) -> &'static [Item] {
    match kind {
        SubmenuKind::System => SYSTEM_OPTIONS_ITEMS,
        SubmenuKind::Graphics => GRAPHICS_OPTIONS_ITEMS,
        SubmenuKind::Input => INPUT_OPTIONS_ITEMS,
        SubmenuKind::InputBackend => INPUT_BACKEND_OPTIONS_ITEMS,
        SubmenuKind::OnlineScoring => ONLINE_SCORING_OPTIONS_ITEMS,
        SubmenuKind::NullOrDie => NULL_OR_DIE_MENU_ITEMS,
        SubmenuKind::NullOrDieOptions => NULL_OR_DIE_OPTIONS_ITEMS,
        SubmenuKind::SyncPacks => SYNC_PACK_OPTIONS_ITEMS,
        SubmenuKind::Machine => MACHINE_OPTIONS_ITEMS,
        SubmenuKind::Advanced => ADVANCED_OPTIONS_ITEMS,
        SubmenuKind::Course => COURSE_OPTIONS_ITEMS,
        SubmenuKind::Gameplay => GAMEPLAY_OPTIONS_ITEMS,
        SubmenuKind::Sound => SOUND_OPTIONS_ITEMS,
        SubmenuKind::SelectMusic => SELECT_MUSIC_OPTIONS_ITEMS,
        SubmenuKind::GrooveStats => GROOVESTATS_OPTIONS_ITEMS,
        SubmenuKind::ArrowCloud => ARROWCLOUD_OPTIONS_ITEMS,
        SubmenuKind::ScoreImport => SCORE_IMPORT_OPTIONS_ITEMS,
    }
}

pub const fn submenu_title(kind: SubmenuKind) -> &'static str {
    match kind {
        SubmenuKind::System => "SYSTEM OPTIONS",
        SubmenuKind::Graphics => "GRAPHICS OPTIONS",
        SubmenuKind::Input => "INPUT OPTIONS",
        SubmenuKind::InputBackend => "INPUT OPTIONS",
        SubmenuKind::OnlineScoring => "ONLINE SCORE SERVICES",
        SubmenuKind::NullOrDie => "NULL-OR-DIE OPTIONS",
        SubmenuKind::NullOrDieOptions => "NULL-OR-DIE OPTIONS",
        SubmenuKind::SyncPacks => "SYNC PACKS",
        SubmenuKind::Machine => "MACHINE OPTIONS",
        SubmenuKind::Advanced => "ADVANCED OPTIONS",
        SubmenuKind::Course => "COURSE OPTIONS",
        SubmenuKind::Gameplay => "GAMEPLAY OPTIONS",
        SubmenuKind::Sound => "SOUND OPTIONS",
        SubmenuKind::SelectMusic => "SELECT MUSIC OPTIONS",
        SubmenuKind::GrooveStats => "GROOVESTATS OPTIONS",
        SubmenuKind::ArrowCloud => "ARROWCLOUD OPTIONS",
        SubmenuKind::ScoreImport => "SCORE IMPORT",
    }
}

pub fn backend_to_renderer_choice_index(backend: BackendType) -> usize {
    VIDEO_RENDERER_OPTIONS
        .iter()
        .position(|(b, _)| *b == backend)
        .unwrap_or(0)
}

pub fn renderer_choice_index_to_backend(idx: usize) -> BackendType {
    VIDEO_RENDERER_OPTIONS
        .get(idx)
        .map_or_else(|| VIDEO_RENDERER_OPTIONS[0].0, |(backend, _)| *backend)
}

pub fn selected_video_renderer(state: &State) -> BackendType {
    let choice_idx = state
        .sub_choice_indices_graphics
        .get(VIDEO_RENDERER_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    renderer_choice_index_to_backend(choice_idx)
}

pub fn build_software_thread_choices() -> Vec<u8> {
    let max_threads = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(8)
        .clamp(2, 32);
    let mut out = Vec::with_capacity(max_threads + 1);
    out.push(0); // Auto
    for n in 1..=max_threads {
        out.push(n as u8);
    }
    out
}

pub fn software_thread_choice_labels(values: &[u8]) -> Vec<String> {
    values
        .iter()
        .map(|v| {
            if *v == 0 {
                tr("Common", "Auto").to_string()
            } else {
                v.to_string()
            }
        })
        .collect()
}

pub fn software_thread_choice_index(values: &[u8], thread_count: u8) -> usize {
    values
        .iter()
        .position(|&v| v == thread_count)
        .unwrap_or_else(|| {
            values
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.abs_diff(thread_count))
                .map_or(0, |(idx, _)| idx)
        })
}

pub fn software_thread_from_choice(values: &[u8], idx: usize) -> u8 {
    values.get(idx).copied().unwrap_or(0)
}

pub fn build_max_fps_choices() -> Vec<u16> {
    let mut out = Vec::with_capacity(
        1 + usize::from(MAX_FPS_MAX.saturating_sub(MAX_FPS_MIN)) / usize::from(MAX_FPS_STEP),
    );
    let mut fps = MAX_FPS_MIN;
    while fps <= MAX_FPS_MAX {
        out.push(fps);
        fps = fps.saturating_add(MAX_FPS_STEP);
    }
    out
}

pub fn max_fps_choice_labels(values: &[u16]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[inline(always)]
pub const fn clamped_max_fps(max_fps: u16) -> u16 {
    if max_fps < MAX_FPS_MIN {
        MAX_FPS_MIN
    } else if max_fps > MAX_FPS_MAX {
        MAX_FPS_MAX
    } else {
        max_fps
    }
}

pub fn max_fps_choice_index(values: &[u16], max_fps: u16) -> usize {
    let target = clamped_max_fps(max_fps);
    values.iter().position(|&v| v == target).unwrap_or_else(|| {
        values
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.abs_diff(target))
            .map_or(0, |(idx, _)| idx)
    })
}

pub fn max_fps_from_choice(values: &[u16], idx: usize) -> u16 {
    values.get(idx).copied().unwrap_or(MAX_FPS_DEFAULT)
}

#[inline(always)]
pub const fn present_mode_choice_index(mode: PresentModePolicy) -> usize {
    match mode {
        PresentModePolicy::Mailbox => 0,
        PresentModePolicy::Immediate => 1,
    }
}

#[inline(always)]
pub const fn present_mode_from_choice(idx: usize) -> PresentModePolicy {
    match idx {
        1 => PresentModePolicy::Immediate,
        _ => PresentModePolicy::Mailbox,
    }
}

pub fn selected_present_mode_policy(state: &State) -> PresentModePolicy {
    state
        .sub_choice_indices_graphics
        .get(PRESENT_MODE_ROW_INDEX)
        .copied()
        .map_or(state.present_mode_policy_at_load, present_mode_from_choice)
}

#[inline(always)]
pub fn set_max_fps_enabled_choice(state: &mut State, enabled: bool) {
    let idx = yes_no_choice_index(enabled);
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(MAX_FPS_ENABLED_ROW_INDEX)
    {
        *slot = idx;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(MAX_FPS_ENABLED_ROW_INDEX)
    {
        *slot = idx;
    }
}

#[inline(always)]
pub fn set_max_fps_value_choice_index(state: &mut State, idx: usize) {
    let max_idx = state.max_fps_choices.len().saturating_sub(1);
    let clamped = idx.min(max_idx);
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(MAX_FPS_VALUE_ROW_INDEX)
    {
        *slot = clamped;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(MAX_FPS_VALUE_ROW_INDEX)
    {
        *slot = clamped;
    }
}

#[inline(always)]
pub fn graphics_show_software_threads(state: &State) -> bool {
    selected_video_renderer(state) == BackendType::Software
}

#[inline(always)]
pub fn graphics_show_present_mode(state: &State) -> bool {
    state
        .sub_choice_indices_graphics
        .get(VSYNC_ROW_INDEX)
        .copied()
        .is_some_and(|idx| !yes_no_from_choice(idx))
}

#[inline(always)]
pub fn graphics_show_max_fps(state: &State) -> bool {
    graphics_show_present_mode(state)
}

#[inline(always)]
pub fn max_fps_enabled(state: &State) -> bool {
    state
        .sub_choice_indices_graphics
        .get(MAX_FPS_ENABLED_ROW_INDEX)
        .copied()
        .is_some_and(yes_no_from_choice)
}

#[inline(always)]
pub fn graphics_show_max_fps_value(state: &State) -> bool {
    graphics_show_max_fps(state) && max_fps_enabled(state)
}

pub fn submenu_visible_row_indices(state: &State, kind: SubmenuKind, rows: &[SubRow]) -> Vec<usize> {
    match kind {
        SubmenuKind::Graphics => {
            let show_sw = graphics_show_software_threads(state);
            let show_present_mode = graphics_show_present_mode(state);
            let show_max_fps = graphics_show_max_fps(state);
            let show_max_fps_value = graphics_show_max_fps_value(state);
            rows.iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.id == SubRowId::SoftwareRendererThreads && !show_sw {
                        None
                    } else if row.id == SubRowId::PresentMode && !show_present_mode {
                        None
                    } else if row.id == SubRowId::MaxFps && !show_max_fps {
                        None
                    } else if row.id == SubRowId::MaxFpsValue && !show_max_fps_value {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        SubmenuKind::Advanced => rows.iter().enumerate().map(|(idx, _)| idx).collect(),
        SubmenuKind::SelectMusic => {
            let show_banners = state
                .sub_choice_indices_select_music
                .get(SELECT_MUSIC_SHOW_BANNERS_ROW_INDEX)
                .copied()
                .unwrap_or_else(|| yes_no_choice_index(true));
            let show_banners = yes_no_from_choice(show_banners);
            let show_breakdown = state
                .sub_choice_indices_select_music
                .get(SELECT_MUSIC_SHOW_BREAKDOWN_ROW_INDEX)
                .copied()
                .unwrap_or_else(|| yes_no_choice_index(true));
            let show_breakdown = yes_no_from_choice(show_breakdown);
            let show_previews = state
                .sub_choice_indices_select_music
                .get(SELECT_MUSIC_MUSIC_PREVIEWS_ROW_INDEX)
                .copied()
                .unwrap_or_else(|| yes_no_choice_index(true));
            let show_previews = yes_no_from_choice(show_previews);
            let show_scorebox = state
                .sub_choice_indices_select_music
                .get(SELECT_MUSIC_SHOW_SCOREBOX_ROW_INDEX)
                .copied()
                .unwrap_or_else(|| yes_no_choice_index(true));
            let show_scorebox = yes_no_from_choice(show_scorebox);
            rows.iter()
                .enumerate()
                .filter_map(|(idx, _)| {
                    if idx == SELECT_MUSIC_SHOW_VIDEO_BANNERS_ROW_INDEX && !show_banners {
                        None
                    } else if idx == SELECT_MUSIC_BREAKDOWN_STYLE_ROW_INDEX && !show_breakdown {
                        None
                    } else if idx == SELECT_MUSIC_PREVIEW_LOOP_ROW_INDEX && !show_previews {
                        None
                    } else if idx == SELECT_MUSIC_SCOREBOX_PLACEMENT_ROW_INDEX && !show_scorebox {
                        None
                    } else if idx == SELECT_MUSIC_SCOREBOX_CYCLE_ROW_INDEX && !show_scorebox {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        SubmenuKind::Machine => {
            let show_preferred_style = state
                .sub_choice_indices_machine
                .get(MACHINE_SELECT_STYLE_ROW_INDEX)
                .copied()
                .unwrap_or(1)
                == 0;
            let show_preferred_mode = state
                .sub_choice_indices_machine
                .get(MACHINE_SELECT_PLAY_MODE_ROW_INDEX)
                .copied()
                .unwrap_or(1)
                == 0;
            rows.iter()
                .enumerate()
                .filter_map(|(idx, _)| {
                    if idx == MACHINE_PREFERRED_STYLE_ROW_INDEX && !show_preferred_style {
                        None
                    } else if idx == MACHINE_PREFERRED_MODE_ROW_INDEX && !show_preferred_mode {
                        None
                    } else {
                        Some(idx)
                    }
                })
                .collect()
        }
        #[cfg(target_os = "linux")]
        SubmenuKind::Sound => rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                if row.id == SubRowId::AlsaExclusive && !sound_show_alsa_exclusive(state) {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect(),
        _ => (0..rows.len()).collect(),
    }
}

pub fn submenu_total_rows(state: &State, kind: SubmenuKind) -> usize {
    let rows = submenu_rows(kind);
    submenu_visible_row_indices(state, kind, rows).len() + 1
}

pub fn submenu_visible_row_to_actual(
    state: &State,
    kind: SubmenuKind,
    visible_row_idx: usize,
) -> Option<usize> {
    let rows = submenu_rows(kind);
    let visible_rows = submenu_visible_row_indices(state, kind, rows);
    visible_rows.get(visible_row_idx).copied()
}

#[cfg(target_os = "windows")]
pub const fn windows_backend_choice_index(backend: WindowsPadBackend) -> usize {
    match backend {
        WindowsPadBackend::Auto | WindowsPadBackend::RawInput => 0,
        WindowsPadBackend::Wgi => 1,
    }
}

#[cfg(target_os = "windows")]
pub const fn windows_backend_from_choice(idx: usize) -> WindowsPadBackend {
    match idx {
        0 => WindowsPadBackend::RawInput,
        _ => WindowsPadBackend::Wgi,
    }
}

pub const fn fullscreen_type_to_choice_index(fullscreen_type: FullscreenType) -> usize {
    match fullscreen_type {
        FullscreenType::Exclusive => 0,
        FullscreenType::Borderless => 1,
    }
}

pub const fn choice_index_to_fullscreen_type(idx: usize) -> FullscreenType {
    match idx {
        1 => FullscreenType::Borderless,
        _ => FullscreenType::Exclusive,
    }
}

pub fn selected_fullscreen_type(state: &State) -> FullscreenType {
    state
        .sub_choice_indices_graphics
        .get(FULLSCREEN_TYPE_ROW_INDEX)
        .copied()
        .map_or(FullscreenType::Exclusive, choice_index_to_fullscreen_type)
}

pub fn selected_display_mode(state: &State) -> DisplayMode {
    let display_choice = state
        .sub_choice_indices_graphics
        .get(DISPLAY_MODE_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    let windowed_idx = state.display_mode_choices.len().saturating_sub(1);
    if windowed_idx == 0 || display_choice >= windowed_idx {
        DisplayMode::Windowed
    } else {
        DisplayMode::Fullscreen(selected_fullscreen_type(state))
    }
}

pub fn selected_display_monitor(state: &State) -> usize {
    let display_choice = state
        .sub_choice_indices_graphics
        .get(DISPLAY_MODE_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    let windowed_idx = state.display_mode_choices.len().saturating_sub(1);
    if windowed_idx == 0 || display_choice >= windowed_idx {
        0
    } else {
        display_choice.min(windowed_idx.saturating_sub(1))
    }
}

pub fn selected_refresh_rate_millihertz(state: &State) -> u32 {
    let idx = state
        .sub_choice_indices_graphics
        .get(REFRESH_RATE_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    state.refresh_rate_choices.get(idx).copied().unwrap_or(0)
}

pub fn max_fps_seed_value(state: &State, max_fps: u16) -> u16 {
    if max_fps != 0 {
        return clamped_max_fps(max_fps);
    }

    let selected_refresh_mhz = selected_refresh_rate_millihertz(state);
    let refresh_mhz = if selected_refresh_mhz != 0 {
        selected_refresh_mhz
    } else if let Some(spec) = state.monitor_specs.get(selected_display_monitor(state)) {
        if matches!(selected_display_mode(state), DisplayMode::Fullscreen(_)) {
            let (width, height) = selected_resolution(state);
            display::supported_refresh_rates(Some(spec), width, height)
                .into_iter()
                .max()
                .or_else(|| {
                    spec.modes
                        .iter()
                        .map(|mode| mode.refresh_rate_millihertz)
                        .max()
                })
                .unwrap_or(60_000)
        } else {
            spec.modes
                .iter()
                .map(|mode| mode.refresh_rate_millihertz)
                .max()
                .unwrap_or(60_000)
        }
    } else {
        60_000
    };

    clamped_max_fps(((refresh_mhz + 500) / 1000) as u16)
}

pub fn seed_max_fps_value_choice(state: &mut State, max_fps: u16) {
    let seeded = max_fps_seed_value(state, max_fps);
    let idx = max_fps_choice_index(&state.max_fps_choices, seeded);
    set_max_fps_value_choice_index(state, idx);
}

pub fn selected_max_fps(state: &State) -> u16 {
    if !max_fps_enabled(state) {
        return 0;
    }
    let idx = state
        .sub_choice_indices_graphics
        .get(MAX_FPS_VALUE_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    max_fps_from_choice(&state.max_fps_choices, idx)
}

pub fn ensure_display_mode_choices(state: &mut State) {
    state.display_mode_choices = build_display_mode_choices(&state.monitor_specs);
    // If current selection is out of bounds, reset it.
    if let Some(idx) = state
        .sub_choice_indices_graphics
        .get_mut(DISPLAY_MODE_ROW_INDEX)
        && *idx >= state.display_mode_choices.len()
    {
        *idx = 0;
    }
    if let Some(choice_idx) = state
        .sub_choice_indices_graphics
        .get(DISPLAY_MODE_ROW_INDEX)
        .copied()
        && let Some(cursor_idx) = state
            .sub_cursor_indices_graphics
            .get_mut(DISPLAY_MODE_ROW_INDEX)
    {
        *cursor_idx = choice_idx;
    }
    // Also re-run logic that depends on the selected monitor.
    let current_res = selected_resolution(state);
    rebuild_resolution_choices(state, current_res.0, current_res.1);
}

pub fn update_monitor_specs(state: &mut State, specs: Vec<MonitorSpec>) {
    state.monitor_specs = specs;
    ensure_display_mode_choices(state);
    // Keep the Display Mode row aligned with the actual current mode after monitors refresh.
    set_display_mode_row_selection(
        state,
        state.monitor_specs.len(),
        state.display_mode_at_load,
        state.display_monitor_at_load,
    );
    if state.max_fps_at_load == 0 && !max_fps_enabled(state) {
        seed_max_fps_value_choice(state, 0);
    }
    clear_render_cache(state);
}

pub fn set_display_mode_row_selection(
    state: &mut State,
    _monitor_count: usize, // Ignored, we use stored monitor_specs now
    mode: DisplayMode,
    monitor: usize,
) {
    // Ensure choices are up to date.
    ensure_display_mode_choices(state);
    let windowed_idx = state.display_mode_choices.len().saturating_sub(1);
    let idx = match mode {
        DisplayMode::Windowed => windowed_idx,
        DisplayMode::Fullscreen(_) => {
            let max_idx = windowed_idx.saturating_sub(1);
            if max_idx == 0 {
                0
            } else {
                monitor.min(max_idx)
            }
        }
    };
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(DISPLAY_MODE_ROW_INDEX)
    {
        *slot = idx;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(DISPLAY_MODE_ROW_INDEX)
    {
        *slot = idx;
    }
    // Re-trigger resolution rebuild based on the potentially new monitor selection.
    let current_res = selected_resolution(state);
    rebuild_resolution_choices(state, current_res.0, current_res.1);
}

pub fn selected_aspect_label(state: &State) -> &'static str {
    let idx = state
        .sub_choice_indices_graphics
        .get(DISPLAY_ASPECT_RATIO_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    DISPLAY_ASPECT_RATIO_CHOICES
        .get(idx)
        .or(Some(&DISPLAY_ASPECT_RATIO_CHOICES[0]))
        .and_then(|c| c.as_str_static())
        .unwrap_or("16:9")
}

pub fn inferred_aspect_choice(width: u32, height: u32) -> usize {
    if height == 0 {
        return 0;
    }

    if let Some(idx) = DISPLAY_ASPECT_RATIO_CHOICES.iter().position(|c| {
        c.as_str_static()
            .map_or(false, |label| aspect_matches(width, height, label))
    }) {
        return idx;
    }

    let ratio = width as f32 / height as f32;
    let mut best_idx = 0;
    let mut best_delta = f32::INFINITY;
    for (idx, choice) in DISPLAY_ASPECT_RATIO_CHOICES.iter().enumerate() {
        let Some(label) = choice.as_str_static() else {
            continue;
        };
        let target = match label {
            "16:9" => 16.0 / 9.0,
            "16:10" => 16.0 / 10.0,
            "4:3" => 4.0 / 3.0,
            "1:1" => 1.0,
            _ => continue,
        };
        let delta = (ratio - target).abs();
        if delta < best_delta {
            best_delta = delta;
            best_idx = idx;
        }
    }
    best_idx
}

pub fn sync_display_aspect_ratio(state: &mut State, width: u32, height: u32) {
    let idx = inferred_aspect_choice(width, height);
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(DISPLAY_ASPECT_RATIO_ROW_INDEX)
    {
        *slot = idx;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(DISPLAY_ASPECT_RATIO_ROW_INDEX)
    {
        *slot = idx;
    }
}

pub fn push_unique_resolution(target: &mut Vec<(u32, u32)>, width: u32, height: u32) {
    if !target.iter().any(|&(w, h)| w == width && h == height) {
        target.push((width, height));
    }
}

pub fn preset_resolutions_for_aspect(label: &str) -> Vec<(u32, u32)> {
    match label.to_ascii_lowercase().as_str() {
        "16:9" => vec![(1280, 720), (1600, 900), (1920, 1080)],
        "16:10" => vec![(1280, 800), (1440, 900), (1680, 1050), (1920, 1200)],
        "4:3" => vec![
            (640, 480),
            (800, 600),
            (1024, 768),
            (1280, 960),
            (1600, 1200),
        ],
        "1:1" => vec![(342, 342), (456, 456), (608, 608), (810, 810), (1080, 1080)],
        _ => DEFAULT_RESOLUTION_CHOICES.to_vec(),
    }
}

pub fn aspect_matches(width: u32, height: u32, label: &str) -> bool {
    let ratio = width as f32 / height as f32;
    match label {
        "16:9" => (ratio - 1.7777).abs() < 0.05,
        "16:10" => (ratio - 1.6).abs() < 0.05,
        "4:3" => (ratio - 1.3333).abs() < 0.05,
        "1:1" => (ratio - 1.0).abs() < 0.05,
        _ => true,
    }
}

pub fn selected_resolution(state: &State) -> (u32, u32) {
    let idx = state
        .sub_choice_indices_graphics
        .get(DISPLAY_RESOLUTION_ROW_INDEX)
        .copied()
        .unwrap_or(0);
    state
        .resolution_choices
        .get(idx)
        .copied()
        .or_else(|| state.resolution_choices.first().copied())
        .unwrap_or((state.display_width_at_load, state.display_height_at_load))
}

pub fn rebuild_refresh_rate_choices(state: &mut State) {
    if matches!(selected_display_mode(state), DisplayMode::Windowed) {
        state.refresh_rate_choices = vec![0];
        if let Some(slot) = state
            .sub_choice_indices_graphics
            .get_mut(REFRESH_RATE_ROW_INDEX)
        {
            *slot = 0;
        }
        if let Some(slot) = state
            .sub_cursor_indices_graphics
            .get_mut(REFRESH_RATE_ROW_INDEX)
        {
            *slot = 0;
        }
        return;
    }

    let (width, height) = selected_resolution(state);
    let mon_idx = selected_display_monitor(state);
    let mut rates = Vec::new();

    // Default choice is always available (0).
    rates.push(0);

    let supported_rates =
        display::supported_refresh_rates(state.monitor_specs.get(mon_idx), width, height);
    rates.extend(supported_rates);

    // Add common fallback rates if list is empty (besides Default)
    if rates.len() == 1 {
        rates.extend_from_slice(&[60000, 75000, 120000, 144000, 165000, 240000]);
    }

    // Preserve current selection if possible, else default to "Default".
    let current_rate = if let Some(idx) = state
        .sub_choice_indices_graphics
        .get(REFRESH_RATE_ROW_INDEX)
    {
        state.refresh_rate_choices.get(*idx).copied().unwrap_or(0)
    } else {
        0
    };

    state.refresh_rate_choices = rates;

    let next_idx = state
        .refresh_rate_choices
        .iter()
        .position(|&r| r == current_rate)
        .unwrap_or(0);
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(REFRESH_RATE_ROW_INDEX)
    {
        *slot = next_idx;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(REFRESH_RATE_ROW_INDEX)
    {
        *slot = next_idx;
    }
    if state.max_fps_at_load == 0 && !max_fps_enabled(state) {
        seed_max_fps_value_choice(state, 0);
    }
}

pub fn rebuild_resolution_choices(state: &mut State, width: u32, height: u32) {
    let aspect_label = selected_aspect_label(state);
    let mon_idx = selected_display_monitor(state);

    let mut list: Vec<(u32, u32)> =
        display::supported_resolutions(state.monitor_specs.get(mon_idx))
            .into_iter()
            .filter(|(w, h)| aspect_matches(*w, *h, aspect_label))
            .collect();

    // 2. If list is empty (e.g. no monitor data or Aspect filter too strict), use presets.
    if list.is_empty() {
        list = preset_resolutions_for_aspect(aspect_label);
    }

    // 3. Keep the current resolution only if it matches the selected aspect.
    if aspect_matches(width, height, aspect_label) {
        push_unique_resolution(&mut list, width, height);
    }

    // Sort descending by width then height (typical UI preference).
    list.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    state.resolution_choices = list;
    let next_idx = state
        .resolution_choices
        .iter()
        .position(|&(w, h)| w == width && h == height)
        .unwrap_or(0);
    if let Some(slot) = state
        .sub_choice_indices_graphics
        .get_mut(DISPLAY_RESOLUTION_ROW_INDEX)
    {
        *slot = next_idx;
    }
    if let Some(slot) = state
        .sub_cursor_indices_graphics
        .get_mut(DISPLAY_RESOLUTION_ROW_INDEX)
    {
        *slot = next_idx;
    }

    // Rebuild refresh rates since available rates depend on resolution.
    rebuild_refresh_rate_choices(state);
}

#[inline(always)]
pub const fn score_import_endpoint_from_choice_index(idx: usize) -> scores::ScoreImportEndpoint {
    match idx {
        1 => scores::ScoreImportEndpoint::BoogieStats,
        2 => scores::ScoreImportEndpoint::ArrowCloud,
        _ => scores::ScoreImportEndpoint::GrooveStats,
    }
}

#[inline(always)]
pub fn score_import_selected_endpoint(state: &State) -> scores::ScoreImportEndpoint {
    let idx = state
        .sub_choice_indices_score_import
        .get(SCORE_IMPORT_ROW_ENDPOINT_INDEX)
        .copied()
        .unwrap_or(0);
    score_import_endpoint_from_choice_index(idx)
}

pub fn installed_pack_options(all_label: &str) -> (Vec<String>, Vec<Option<String>>) {
    let cache = crate::game::song::get_song_cache();
    let mut packs: Vec<(String, String)> = Vec::with_capacity(cache.len());
    let mut seen_groups: HashSet<String> = HashSet::with_capacity(cache.len());

    for pack in cache.iter() {
        let group_name = pack.group_name.trim();
        if group_name.is_empty() {
            continue;
        }
        let group_key = group_name.to_ascii_lowercase();
        if !seen_groups.insert(group_key) {
            continue;
        }
        let display_name = if pack.name.trim().is_empty() {
            group_name.to_string()
        } else {
            pack.name.trim().to_string()
        };
        packs.push((display_name, group_name.to_string()));
    }

    packs.sort_by(|a, b| {
        a.0.to_ascii_lowercase()
            .cmp(&b.0.to_ascii_lowercase())
            .then_with(|| a.1.cmp(&b.1))
    });

    let mut choices = Vec::with_capacity(packs.len() + 1);
    let mut filters = Vec::with_capacity(packs.len() + 1);
    choices.push(all_label.to_string());
    filters.push(None);
    for (display_name, group_name) in packs {
        choices.push(display_name);
        filters.push(Some(group_name));
    }
    (choices, filters)
}

pub fn score_import_pack_options() -> (Vec<String>, Vec<Option<String>>) {
    installed_pack_options(&tr("OptionsScoreImport", "AllPacks"))
}

pub fn sync_pack_options() -> (Vec<String>, Vec<Option<String>>) {
    installed_pack_options(&tr("OptionsSyncPack", "AllPacks"))
}

pub fn load_score_import_profiles() -> Vec<ScoreImportProfileConfig> {
    let mut profiles = Vec::new();
    for summary in profile::scan_local_profiles() {
        let profile_dir = dirs::app_dirs().profiles_root().join(summary.id.as_str());
        let mut gs = SimpleIni::new();
        let mut ac = SimpleIni::new();
        let gs_api_key = if gs.load(profile_dir.join("groovestats.ini")).is_ok() {
            gs.get("GrooveStats", "ApiKey")
                .map_or_else(String::new, |v| v.trim().to_string())
        } else {
            String::new()
        };
        let gs_username = if gs_api_key.is_empty() {
            String::new()
        } else {
            gs.get("GrooveStats", "Username")
                .map_or_else(String::new, |v| v.trim().to_string())
        };
        let ac_api_key = if ac.load(profile_dir.join("arrowcloud.ini")).is_ok() {
            ac.get("ArrowCloud", "ApiKey")
                .map_or_else(String::new, |v| v.trim().to_string())
        } else {
            String::new()
        };
        profiles.push(ScoreImportProfileConfig {
            id: summary.id,
            display_name: summary.display_name.trim().to_string(),
            gs_api_key,
            gs_username,
            ac_api_key,
        });
    }
    profiles.sort_by(|a, b| {
        let al = a.display_name.to_ascii_lowercase();
        let bl = b.display_name.to_ascii_lowercase();
        al.cmp(&bl).then_with(|| a.id.cmp(&b.id))
    });
    profiles
}

#[inline(always)]
pub fn score_import_profile_eligible(
    endpoint: scores::ScoreImportEndpoint,
    profile_cfg: &ScoreImportProfileConfig,
) -> bool {
    match endpoint {
        scores::ScoreImportEndpoint::GrooveStats | scores::ScoreImportEndpoint::BoogieStats => {
            !profile_cfg.gs_api_key.is_empty() && !profile_cfg.gs_username.is_empty()
        }
        scores::ScoreImportEndpoint::ArrowCloud => !profile_cfg.ac_api_key.is_empty(),
    }
}

pub fn refresh_score_import_profile_options(state: &mut State) {
    state.score_import_profile_choices.clear();
    state.score_import_profile_ids.clear();

    let endpoint = score_import_selected_endpoint(state);
    for profile_cfg in &state.score_import_profiles {
        if !score_import_profile_eligible(endpoint, profile_cfg) {
            continue;
        }
        let label = if profile_cfg.display_name.is_empty() {
            profile_cfg.id.clone()
        } else {
            format!("{} ({})", profile_cfg.display_name, profile_cfg.id)
        };
        state.score_import_profile_choices.push(label);
        state
            .score_import_profile_ids
            .push(Some(profile_cfg.id.clone()));
    }
    if state.score_import_profile_choices.is_empty() {
        state
            .score_import_profile_choices
            .push(tr("OptionsScoreImport", "NoEligibleProfiles").to_string());
        state.score_import_profile_ids.push(None);
    }

    let max_idx = state.score_import_profile_choices.len().saturating_sub(1);
    if let Some(slot) = state
        .sub_choice_indices_score_import
        .get_mut(SCORE_IMPORT_ROW_PROFILE_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
    if let Some(slot) = state
        .sub_cursor_indices_score_import
        .get_mut(SCORE_IMPORT_ROW_PROFILE_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
}

pub fn refresh_score_import_pack_options(state: &mut State) {
    let (choices, filters) = score_import_pack_options();
    state.score_import_pack_choices = choices;
    state.score_import_pack_filters = filters;
    let max_idx = state.score_import_pack_choices.len().saturating_sub(1);
    if let Some(slot) = state
        .sub_choice_indices_score_import
        .get_mut(SCORE_IMPORT_ROW_PACK_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
    if let Some(slot) = state
        .sub_cursor_indices_score_import
        .get_mut(SCORE_IMPORT_ROW_PACK_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
}

pub fn refresh_sync_pack_options(state: &mut State) {
    let (choices, filters) = sync_pack_options();
    state.sync_pack_choices = choices;
    state.sync_pack_filters = filters;
    let max_idx = state.sync_pack_choices.len().saturating_sub(1);
    if let Some(slot) = state
        .sub_choice_indices_sync_packs
        .get_mut(SYNC_PACK_ROW_PACK_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
    if let Some(slot) = state
        .sub_cursor_indices_sync_packs
        .get_mut(SYNC_PACK_ROW_PACK_INDEX)
    {
        *slot = (*slot).min(max_idx);
    }
}

pub fn refresh_score_import_options(state: &mut State) {
    state.score_import_profiles = load_score_import_profiles();
    refresh_score_import_profile_options(state);
    refresh_score_import_pack_options(state);
}

pub fn refresh_null_or_die_options(state: &mut State) {
    refresh_sync_pack_options(state);
}

pub fn selected_score_import_pack_group(state: &State) -> Option<String> {
    let pack_idx = state
        .sub_choice_indices_score_import
        .get(SCORE_IMPORT_ROW_PACK_INDEX)
        .copied()
        .unwrap_or(0)
        .min(state.score_import_pack_filters.len().saturating_sub(1));
    state
        .score_import_pack_filters
        .get(pack_idx)
        .cloned()
        .flatten()
}

pub fn selected_score_import_profile(state: &State) -> Option<ScoreImportProfileConfig> {
    let profile_idx = state
        .sub_choice_indices_score_import
        .get(SCORE_IMPORT_ROW_PROFILE_INDEX)
        .copied()
        .unwrap_or(0)
        .min(state.score_import_profile_ids.len().saturating_sub(1));
    let profile_id = state
        .score_import_profile_ids
        .get(profile_idx)
        .cloned()
        .flatten()?;
    state
        .score_import_profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
}

#[inline(always)]
pub fn score_import_only_missing_gs_scores(state: &State) -> bool {
    yes_no_from_choice(
        state
            .sub_choice_indices_score_import
            .get(SCORE_IMPORT_ROW_ONLY_MISSING_INDEX)
            .copied()
            .unwrap_or_else(|| yes_no_choice_index(false)),
    )
}

pub fn selected_score_import_selection(state: &State) -> Option<ScoreImportSelection> {
    let endpoint = score_import_selected_endpoint(state);
    let profile_cfg = selected_score_import_profile(state)?;
    if !score_import_profile_eligible(endpoint, &profile_cfg) {
        return None;
    }
    let pack_group = selected_score_import_pack_group(state);
    let pack_label = pack_group
        .as_ref()
        .cloned()
        .unwrap_or_else(|| tr("OptionsScoreImport", "AllPacks").to_string());
    let only_missing_gs_scores = score_import_only_missing_gs_scores(state);
    Some(ScoreImportSelection {
        endpoint,
        profile: profile_cfg,
        pack_group,
        pack_label,
        only_missing_gs_scores,
    })
}

pub fn selected_sync_pack_selection(state: &State) -> SyncPackSelection {
    let pack_idx = state
        .sub_choice_indices_sync_packs
        .get(SYNC_PACK_ROW_PACK_INDEX)
        .copied()
        .unwrap_or(0)
        .min(state.sync_pack_filters.len().saturating_sub(1));
    let pack_group = state.sync_pack_filters.get(pack_idx).cloned().flatten();
    let pack_label = state
        .sync_pack_choices
        .get(pack_idx)
        .cloned()
        .unwrap_or_else(|| tr("OptionsSyncPack", "AllPacks").to_string());
    SyncPackSelection {
        pack_group,
        pack_label,
    }
}

pub fn row_choices(
    state: &State,
    kind: SubmenuKind,
    rows: &[SubRow],
    row_idx: usize,
) -> Vec<Cow<'static, str>> {
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::System)
        && row.id == SubRowId::DefaultNoteSkin
    {
        return state
            .system_noteskin_choices
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::Graphics)
    {
        if row.id == SubRowId::SoftwareRendererThreads {
            return state
                .software_thread_labels
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
        if row.id == SubRowId::MaxFpsValue {
            return state
                .max_fps_labels
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
        if row.id == SubRowId::DisplayMode {
            return state
                .display_mode_choices
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
        if row.id == SubRowId::DisplayResolution {
            return state
                .resolution_choices
                .iter()
                .map(|&(w, h)| Cow::Owned(format!("{w}x{h}")))
                .collect();
        }
        if row.id == SubRowId::RefreshRate {
            return state
                .refresh_rate_choices
                .iter()
                .map(|&mhz| {
                    if mhz == 0 {
                        Cow::Owned(tr("Common", "Default").to_string())
                    } else {
                        // Format nicely: 60000 -> "60 Hz", 59940 -> "59.94 Hz"
                        let hz = mhz as f32 / 1000.0;
                        if (hz.fract()).abs() < 0.01 {
                            Cow::Owned(format!("{hz:.0}Hz"))
                        } else {
                            Cow::Owned(format!("{hz:.2}Hz"))
                        }
                    }
                })
                .collect();
        }
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::Advanced)
        && row.id == SubRowId::SongParsingThreads
    {
        return state
            .software_thread_labels
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::NullOrDieOptions)
        && row.id == SubRowId::PackSyncThreads
    {
        return state
            .software_thread_labels
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::Sound)
    {
        if row.id == SubRowId::SoundDevice {
            return state
                .sound_device_options
                .iter()
                .map(|opt| Cow::Owned(opt.label.clone()))
                .collect();
        }
        if row.id == SubRowId::AudioSampleRate {
            return sound_sample_rate_choices(state)
                .into_iter()
                .map(|rate| match rate {
                    None => Cow::Owned(tr("Common", "Auto").to_string()),
                    Some(hz) => Cow::Owned(format!("{hz} Hz")),
                })
                .collect();
        }
        #[cfg(target_os = "linux")]
        if row.id == SubRowId::LinuxAudioBackend {
            return state
                .linux_backend_choices
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::ScoreImport)
    {
        if row.id == SubRowId::ScoreImportProfile {
            return state
                .score_import_profile_choices
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
        if row.id == SubRowId::ScoreImportPack {
            return state
                .score_import_pack_choices
                .iter()
                .cloned()
                .map(Cow::Owned)
                .collect();
        }
    }
    if let Some(row) = rows.get(row_idx)
        && matches!(kind, SubmenuKind::SyncPacks)
        && row.id == SubRowId::SyncPackPack
    {
        return state
            .sync_pack_choices
            .iter()
            .cloned()
            .map(Cow::Owned)
            .collect();
    }
    rows.get(row_idx)
        .map(|row| {
            row.choices
                .iter()
                .map(|c| Cow::Owned(c.get().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn submenu_display_choice_texts(
    state: &State,
    kind: SubmenuKind,
    rows: &[SubRow],
    row_idx: usize,
) -> Vec<Cow<'static, str>> {
    let mut choice_texts = row_choices(state, kind, rows, row_idx);
    let Some(row) = rows.get(row_idx) else {
        return choice_texts;
    };
    if choice_texts.is_empty() {
        return choice_texts;
    }
    if row.id == SubRowId::GlobalOffset {
        choice_texts[0] = Cow::Owned(format_ms(state.global_offset_ms));
    } else if row.id == SubRowId::MasterVolume {
        choice_texts[0] = Cow::Owned(format_percent(state.master_volume_pct));
    } else if row.id == SubRowId::SfxVolume {
        choice_texts[0] = Cow::Owned(format_percent(state.sfx_volume_pct));
    } else if row.id == SubRowId::AssistTickVolume {
        choice_texts[0] = Cow::Owned(format_percent(state.assist_tick_volume_pct));
    } else if row.id == SubRowId::MusicVolume {
        choice_texts[0] = Cow::Owned(format_percent(state.music_volume_pct));
    } else if row.id == SubRowId::VisualDelay {
        choice_texts[0] = Cow::Owned(format_ms(state.visual_delay_ms));
    } else if row.id == SubRowId::Debounce {
        choice_texts[0] = Cow::Owned(format_ms(state.input_debounce_ms));
    } else if row.id == SubRowId::Fingerprint {
        choice_texts[0] = Cow::Owned(format_tenths_ms(state.null_or_die_fingerprint_tenths));
    } else if row.id == SubRowId::Window {
        choice_texts[0] = Cow::Owned(format_tenths_ms(state.null_or_die_window_tenths));
    } else if row.id == SubRowId::Step {
        choice_texts[0] = Cow::Owned(format_tenths_ms(state.null_or_die_step_tenths));
    } else if row.id == SubRowId::MagicOffset {
        choice_texts[0] = Cow::Owned(format_tenths_ms(state.null_or_die_magic_offset_tenths));
    }
    choice_texts
}