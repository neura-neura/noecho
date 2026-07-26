use crate::types::{DeviceChangeBehavior, ProtectionMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub theme: ThemePreference,
    pub mode: ProtectionMode,
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub restore_on_exit: bool,
    pub auto_recover_on_start: bool,
    pub show_inactive_recent: bool,
    pub preferred_physical_device_id: Option<String>,
    pub preferred_shared_device_id: Option<String>,
    pub device_change_behavior: DeviceChangeBehavior,
    pub excluded_apps: Vec<crate::types::AppIdentity>,
    pub language: String,
    pub language_migrated: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            mode: ProtectionMode::Automatic,
            start_with_windows: false,
            minimize_to_tray: true,
            close_to_tray: true,
            restore_on_exit: true,
            auto_recover_on_start: true,
            show_inactive_recent: false,
            preferred_physical_device_id: None,
            preferred_shared_device_id: None,
            device_change_behavior: DeviceChangeBehavior::AutoFollow,
            excluded_apps: Vec::new(),
            language: "en".into(),
            language_migrated: false,
        }
    }
}
