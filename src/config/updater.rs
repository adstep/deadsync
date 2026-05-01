use std::str::FromStr;

/// Which release channel the updater watches.Today only Stable is wired
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
