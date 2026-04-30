use std::str::FromStr;

/// When the in-app updater should reach out to GitHub on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum UpdateCheckMode {
    /// Never run an automatic check; only manual "Check now" actions hit
    /// the network. Distro packagers and offline installs ship this way.
    Disabled,
    /// Check once per launch, after the runtime is ready.
    #[default]
    OnStartup,
    /// Check at most once every 24 hours, gated by `update_last_checked_at`.
    Daily,
}

impl UpdateCheckMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::OnStartup => "OnStartup",
            Self::Daily => "Daily",
        }
    }
}

impl FromStr for UpdateCheckMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "disabled" | "off" | "0" | "false" | "no" => Ok(Self::Disabled),
            "onstartup" | "startup" | "on_startup" | "on-startup" => Ok(Self::OnStartup),
            "daily" => Ok(Self::Daily),
            _ => Err(()),
        }
    }
}

pub const UPDATE_CHECK_MODE_VARIANTS: [UpdateCheckMode; 3] = [
    UpdateCheckMode::Disabled,
    UpdateCheckMode::OnStartup,
    UpdateCheckMode::Daily,
];

/// Which release channel the updater watches. Today only Stable is wired
/// up; Prerelease is reserved so existing config files don't need a
/// migration when we add it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum UpdateChannel {
    #[default]
    Stable,
    Prerelease,
}

impl UpdateChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Prerelease => "Prerelease",
        }
    }
}

impl FromStr for UpdateChannel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" | "release" => Ok(Self::Stable),
            "prerelease" | "pre-release" | "beta" => Ok(Self::Prerelease),
            _ => Err(()),
        }
    }
}
