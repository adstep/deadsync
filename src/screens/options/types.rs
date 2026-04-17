use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::assets::i18n::{LookupKey, tr};
use crate::game::scores;

#[derive(Clone, Copy, Debug)]
pub struct RowTween {
    pub from_y: f32,
    pub to_y: f32,
    pub from_a: f32,
    pub to_a: f32,
    pub t: f32,
}

impl RowTween {
    #[inline(always)]
    pub fn y(&self) -> f32 {
        (self.to_y - self.from_y).mul_add(self.t, self.from_y)
    }

    #[inline(always)]
    pub fn a(&self) -> f32 {
        (self.to_a - self.from_a).mul_add(self.t, self.from_a)
    }
}

#[derive(Clone, Debug)]
pub struct SubmenuRowLayout {
    pub texts: Arc<[Arc<str>]>,
    pub widths: Arc<[f32]>,
    pub x_positions: Arc<[f32]>,
    pub centers: Arc<[f32]>,
    pub text_h: f32,
    pub inline_row: bool,
}

/// Typed identifier for each top-level Options menu row and submenu item.
/// Used for dispatch so that item selection is string-free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemId {
    // Top-level Options menu
    SystemOptions,
    GraphicsOptions,
    SoundOptions,
    InputOptions,
    MachineOptions,
    GameplayOptions,
    SelectMusicOptions,
    AdvancedOptions,
    CourseOptions,
    ManageLocalProfiles,
    OnlineScoreServices,
    NullOrDieOptions,
    ReloadSongsCourses,
    Credits,
    Exit,

    // System Options submenu
    SysGame,
    SysTheme,
    SysLanguage,
    SysLogLevel,
    SysLogFile,
    SysDefaultNoteSkin,

    // Graphics Options submenu
    GfxVideoRenderer,
    GfxSoftwareThreads,
    GfxDisplayMode,
    GfxDisplayAspectRatio,
    GfxDisplayResolution,
    GfxRefreshRate,
    GfxFullscreenType,
    GfxVSync,
    GfxPresentMode,
    GfxMaxFps,
    GfxMaxFpsValue,
    GfxShowStats,
    GfxValidationLayers,
    GfxVisualDelay,

    // Input Options submenu (launcher)
    InpConfigureMappings,
    InpTestInput,
    InpInputOptions,

    // Input Backend Options submenu
    InpGamepadBackend,
    InpMenuButtons,
    InpOptionsNavigation,
    InpMenuNavigation,
    InpDebounce,

    // Machine Options submenu
    MchSelectProfile,
    MchSelectColor,
    MchSelectStyle,
    MchPreferredStyle,
    MchSelectPlayMode,
    MchPreferredMode,
    MchEvalSummary,
    MchNameEntry,
    MchGameoverScreen,
    MchWriteCurrentScreen,
    MchMenuMusic,
    MchReplays,
    MchPerPlayerGlobalOffsets,
    MchKeyboardFeatures,
    MchVideoBgs,

    // Gameplay Options submenu
    GpBgBrightness,
    GpCenteredP1,
    GpZmodRatingBox,
    GpBpmDecimal,
    GpAutoScreenshot,

    // Sound Options submenu
    SndDevice,
    SndOutputMode,
    SndLinuxBackend,
    SndAlsaExclusive,
    SndSampleRate,
    SndMasterVolume,
    SndSfxVolume,
    SndAssistTickVolume,
    SndMusicVolume,
    SndMineSounds,
    SndGlobalOffset,
    SndRateModPitch,

    // Select Music Options submenu
    SmShowBanners,
    SmShowVideoBanners,
    SmShowBreakdown,
    SmBreakdownStyle,
    SmNativeLanguage,
    SmWheelSpeed,
    SmWheelStyle,
    SmCdTitles,
    SmWheelGrades,
    SmWheelLamps,
    SmWheelItl,
    SmNewPackBadge,
    SmPatternInfo,
    SmChartInfo,
    SmPreviews,
    SmPreviewMarker,
    SmPreviewLoop,
    SmGameplayTimer,
    SmShowRivals,
    SmScoreboxPlacement,
    SmScoreboxCycle,

    // Course Options submenu
    CrsShowRandom,
    CrsShowMostPlayed,
    CrsShowIndividualScores,
    CrsAutosubmitIndividual,

    // Advanced Options submenu
    AdvDefaultFailType,
    AdvBannerCache,
    AdvCdTitleCache,
    AdvSongParsingThreads,
    AdvCacheSongs,
    AdvFastLoad,

    // GrooveStats Options submenu
    GsEnable,
    GsEnableBoogie,
    GsSubmitFails,
    GsAutoPopulate,
    GsAutoDownloadUnlocks,
    GsSeparateUnlocks,

    // ArrowCloud Options submenu
    AcEnable,
    AcSubmitFails,

    // Online Scoring submenu (launcher)
    OsGsBsOptions,
    OsArrowCloudOptions,
    OsScoreImport,

    // Null-or-Die menu (launcher)
    NodOptions,
    NodSyncPacks,

    // Null-or-Die Settings submenu
    NodSyncGraph,
    NodSyncConfidence,
    NodPackSyncThreads,
    NodFingerprint,
    NodWindow,
    NodStep,
    NodMagicOffset,
    NodKernelTarget,
    NodKernelType,
    NodFullSpectrogram,

    // Sync Pack submenu
    SpPack,
    SpStart,

    // Score Import submenu
    SiEndpoint,
    SiProfile,
    SiPack,
    SiOnlyMissing,
    SiStart,
}

/// An entry in the help/description pane for an option item.
#[derive(Clone, Copy)]
pub enum HelpEntry {
    /// Description paragraph text.
    Paragraph(LookupKey),
    /// Bullet point item (rendered with "•" prefix).
    Bullet(LookupKey),
}

/// A simple item model with help text for the description box.
pub struct Item {
    pub id: ItemId,
    pub name: LookupKey,
    pub help: &'static [HelpEntry],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuKind {
    System,
    Graphics,
    Input,
    InputBackend,
    OnlineScoring,
    NullOrDie,
    NullOrDieOptions,
    SyncPacks,
    Machine,
    Advanced,
    Course,
    Gameplay,
    Sound,
    SelectMusic,
    GrooveStats,
    ArrowCloud,
    ScoreImport,
}

#[inline(always)]
pub const fn is_launcher_submenu(kind: SubmenuKind) -> bool {
    matches!(
        kind,
        SubmenuKind::Input | SubmenuKind::OnlineScoring | SubmenuKind::NullOrDie
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsView {
    Main,
    Submenu(SubmenuKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptionCacheKey {
    Main(usize),
    Submenu(SubmenuKind, usize),
}

/// A pre-wrapped block of text in the description pane, ready for rendering.
#[derive(Clone, Debug)]
pub enum RenderedHelpBlock {
    Paragraph { text: Arc<str>, line_count: usize },
    Bullet { text: Arc<str>, line_count: usize },
}

#[derive(Clone, Debug)]
pub struct DescriptionLayout {
    pub key: DescriptionCacheKey,
    pub blocks: Vec<RenderedHelpBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuTransition {
    None,
    FadeOutToSubmenu,
    FadeInSubmenu,
    FadeOutToMain,
    FadeInMain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadPhase {
    Songs,
    Courses,
}

#[derive(Debug)]
pub enum ReloadMsg {
    Phase(ReloadPhase),
    Song {
        done: usize,
        total: usize,
        pack: String,
        song: String,
    },
    Course {
        done: usize,
        total: usize,
        group: String,
        course: String,
    },
    Done,
}

pub struct ReloadUiState {
    pub phase: ReloadPhase,
    pub line2: String,
    pub line3: String,
    pub songs_done: usize,
    pub songs_total: usize,
    pub courses_done: usize,
    pub courses_total: usize,
    pub done: bool,
    pub started_at: Instant,
    pub rx: std::sync::mpsc::Receiver<ReloadMsg>,
}

impl ReloadUiState {
    pub fn new(rx: std::sync::mpsc::Receiver<ReloadMsg>) -> Self {
        Self {
            phase: ReloadPhase::Songs,
            line2: String::new(),
            line3: String::new(),
            songs_done: 0,
            songs_total: 0,
            courses_done: 0,
            courses_total: 0,
            done: false,
            started_at: Instant::now(),
            rx,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScoreImportProfileConfig {
    pub id: String,
    pub display_name: String,
    pub gs_api_key: String,
    pub gs_username: String,
    pub ac_api_key: String,
}

#[derive(Clone, Debug)]
pub struct ScoreImportSelection {
    pub endpoint: scores::ScoreImportEndpoint,
    pub profile: ScoreImportProfileConfig,
    pub pack_group: Option<String>,
    pub pack_label: String,
    pub only_missing_gs_scores: bool,
}

#[derive(Debug)]
pub enum ScoreImportMsg {
    Progress(scores::ScoreImportProgress),
    Done(Result<scores::ScoreBulkImportSummary, String>),
}

pub struct ScoreImportUiState {
    pub endpoint: scores::ScoreImportEndpoint,
    pub profile_name: String,
    pub pack_label: String,
    pub total_charts: usize,
    pub processed_charts: usize,
    pub imported_scores: usize,
    pub missing_scores: usize,
    pub failed_requests: usize,
    pub detail_line: String,
    pub done: bool,
    pub done_message: String,
    pub done_since: Option<Instant>,
    pub cancel_requested: Arc<AtomicBool>,
    pub rx: std::sync::mpsc::Receiver<ScoreImportMsg>,
}

impl ScoreImportUiState {
    pub fn new(
        endpoint: scores::ScoreImportEndpoint,
        profile_name: String,
        pack_label: String,
        cancel_requested: Arc<AtomicBool>,
        rx: std::sync::mpsc::Receiver<ScoreImportMsg>,
    ) -> Self {
        Self {
            endpoint,
            profile_name,
            pack_label,
            total_charts: 0,
            processed_charts: 0,
            imported_scores: 0,
            missing_scores: 0,
            failed_requests: 0,
            detail_line: tr("OptionsScoreImport", "PreparingImport").to_string(),
            done: false,
            done_message: String::new(),
            done_since: None,
            cancel_requested,
            rx,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScoreImportConfirmState {
    pub selection: ScoreImportSelection,
    pub active_choice: u8, // 0 = Yes, 1 = No
}

#[derive(Clone, Debug)]
pub struct SyncPackSelection {
    pub pack_group: Option<String>,
    pub pack_label: String,
}

#[derive(Clone, Debug)]
pub struct SyncPackConfirmState {
    pub selection: SyncPackSelection,
    pub active_choice: u8, // 0 = Yes, 1 = No
}

#[derive(Clone, Debug)]
pub struct SoundDeviceOption {
    pub label: String,
    pub config_index: Option<u16>,
    pub sample_rates_hz: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubRowId {
    // System Options
    Game,
    Theme,
    Language,
    LogLevel,
    LogFile,
    DefaultNoteSkin,
    // Graphics Options
    VideoRenderer,
    SoftwareRendererThreads,
    DisplayMode,
    DisplayAspectRatio,
    DisplayResolution,
    RefreshRate,
    FullscreenType,
    VSync,
    PresentMode,
    MaxFps,
    MaxFpsValue,
    ShowStats,
    ValidationLayers,
    VisualDelay,
    // Sound Options
    SoundDevice,
    AudioOutputMode,
    AudioSampleRate,
    MasterVolume,
    SfxVolume,
    AssistTickVolume,
    MusicVolume,
    MineSounds,
    GlobalOffset,
    RateModPreservesPitch,
    #[cfg(target_os = "linux")]
    LinuxAudioBackend,
    #[cfg(target_os = "linux")]
    AlsaExclusive,
    // Input Options (launcher)
    ConfigureMappings,
    TestInput,
    InputOptions,
    // Input Backend Options
    GamepadBackend,
    MenuNavigation,
    OptionsNavigation,
    MenuButtons,
    Debounce,
    // Machine Options
    SelectProfile,
    SelectColor,
    SelectStyle,
    PreferredStyle,
    SelectPlayMode,
    PreferredMode,
    EvalSummary,
    NameEntry,
    GameoverScreen,
    WriteCurrentScreen,
    MenuMusic,
    Replays,
    PerPlayerGlobalOffsets,
    KeyboardFeatures,
    VideoBgs,
    // Gameplay Options
    BgBrightness,
    CenteredP1Notefield,
    ZmodRatingBox,
    BpmDecimal,
    AutoScreenshot,
    // Select Music Options
    ShowBanners,
    ShowVideoBanners,
    ShowBreakdown,
    BreakdownStyle,
    ShowNativeLanguage,
    MusicWheelSpeed,
    MusicWheelStyle,
    ShowCdTitles,
    ShowWheelGrades,
    ShowWheelLamps,
    ItlWheelData,
    NewPackBadge,
    ShowPatternInfo,
    ChartInfo,
    MusicPreviews,
    PreviewMarker,
    LoopMusic,
    ShowGameplayTimer,
    ShowGsBox,
    GsBoxPlacement,
    GsBoxLeaderboards,
    // Course Options
    ShowRandomCourses,
    ShowMostPlayed,
    ShowIndividualScores,
    AutosubmitIndividual,
    // Advanced Options
    DefaultFailType,
    BannerCache,
    CdTitleCache,
    SongParsingThreads,
    CacheSongs,
    FastLoad,
    // GrooveStats Options
    EnableGrooveStats,
    EnableBoogieStats,
    GsSubmitFails,
    AutoPopulateScores,
    AutoDownloadUnlocks,
    SeparateUnlocksByPlayer,
    // ArrowCloud Options
    EnableArrowCloud,
    ArrowCloudSubmitFails,
    // Online Scoring (launcher)
    GsBsOptions,
    ArrowCloudOptions,
    ScoreImport,
    // Null-or-Die (launcher)
    NullOrDieOptions,
    SyncPacks,
    // Null-or-Die Settings
    SyncGraph,
    SyncConfidence,
    PackSyncThreads,
    Fingerprint,
    Window,
    Step,
    MagicOffset,
    KernelTarget,
    KernelType,
    FullSpectrogram,
    // Sync Pack
    SyncPackPack,
    SyncPackStart,
    // Score Import
    ScoreImportEndpoint,
    ScoreImportProfile,
    ScoreImportPack,
    ScoreImportOnlyMissing,
    ScoreImportStart,
}

pub struct SubRow {
    pub id: SubRowId,
    pub label: LookupKey,
    pub choices: &'static [Choice],
    pub inline: bool, // whether to lay out choices inline (vs single centered value)
}

/// Choice values — some are localizable, some are format-specific literals.
#[derive(Clone, Copy)]
pub enum Choice {
    /// Translatable text (e.g., "Windowed", "On", "Off").
    Localized(LookupKey),
    /// Format-specific literal that should never be translated (e.g., "16:9", "1920x1080").
    Literal(&'static str),
}

impl Choice {
    pub fn get(&self) -> Arc<str> {
        match self {
            Choice::Localized(lkey) => lkey.get(),
            Choice::Literal(s) => Arc::from(*s),
        }
    }

    pub fn as_str_static(&self) -> Option<&'static str> {
        match self {
            Choice::Literal(s) => Some(s),
            Choice::Localized(_) => None,
        }
    }
}
