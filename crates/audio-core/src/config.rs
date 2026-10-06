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
    #[serde(default)]
    pub startup_default_migrated: bool,
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
    pub microphone_to_remote: bool,
    pub monitor: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            mode: ProtectionMode::Automatic,
            start_with_windows: true,
            startup_default_migrated: true,
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
            microphone_to_remote: false,
            monitor: "none".into(),
        }
    }
}

impl AppConfig {
    pub fn migrate_startup_default(&mut self) -> bool {
        if self.startup_default_migrated { return false; }
        self.start_with_windows = true;
        self.startup_default_migrated = true;
        true
    }
}

#[cfg(test)] mod tests {
    #[test] fn startup_defaults_on_and_upgrade_preserves_later_opt_out() {
        use super::AppConfig;
        assert!(AppConfig::default().start_with_windows);
        let mut config:AppConfig=serde_json::from_str(r#"{"start_with_windows":false,"language":"es"}"#).unwrap();
        assert!(config.migrate_startup_default());
        assert!(config.start_with_windows);
        assert_eq!(config.language,"es");
        config.start_with_windows=false;
        let mut saved:AppConfig=serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(!saved.migrate_startup_default());
        assert!(!saved.start_with_windows);
    }
}
