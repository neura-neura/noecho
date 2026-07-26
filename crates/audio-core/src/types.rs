use serde::{Deserialize, Serialize};

/// Persistent identity of an application family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppIdentity {
    /// Lowercase executable file name, e.g. `discord.exe`.
    pub exe_name: String,
    /// Optional full executable path for stronger matching.
    pub exe_path: Option<String>,
    /// Optional display name.
    pub display_name: Option<String>,
}

impl AppIdentity {
    pub fn from_exe(exe_name: impl Into<String>) -> Self {
        let exe_name = exe_name.into().to_ascii_lowercase();
        Self {
            exe_name,
            exe_path: None,
            display_name: None,
        }
    }

    pub fn matches_path(&self, path: &str) -> bool {
        let path_l = path.to_ascii_lowercase();
        if let Some(full) = &self.exe_path {
            if full.eq_ignore_ascii_case(path) {
                return true;
            }
        }
        std::path::Path::new(&path_l)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|name| name == self.exe_name)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Inactive,
    Active,
    Expired,
    Unknown,
}

impl PlaybackState {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Inactive => "Inactiva",
            Self::Active => "Reproduciendo audio",
            Self::Expired => "Reciente",
            Self::Unknown => "Desconocido",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionMode {
    #[default]
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceChangeBehavior {
    #[default]
    AutoFollow,
    Ask,
    KeepCurrent,
}
