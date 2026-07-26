use crate::error::Result;
use crate::grouping::AppAudioGroup;
use crate::persist::PersistedState;
use crate::protection::ProtectionStatus;
use crate::sessions::AudioSessionInfo;
use crate::devices::AudioDevice;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub generated_at: String,
    pub app_version: String,
    pub os: String,
    pub protection: ProtectionStatus,
    pub devices: Vec<AudioDevice>,
    pub sessions: Vec<AudioSessionInfo>,
    pub groups: Vec<AppAudioGroup>,
    pub incomplete_session: bool,
    pub notes: Vec<String>,
}

impl DiagnosticReport {
    pub fn build(
        protection: ProtectionStatus,
        devices: Vec<AudioDevice>,
        sessions: Vec<AudioSessionInfo>,
        groups: Vec<AppAudioGroup>,
        state: &PersistedState,
        notes: Vec<String>,
    ) -> Self {
        Self {
            generated_at: chrono::Local::now().to_rfc3339(),
            app_version: crate::VERSION.to_string(),
            os: std::env::consts::OS.to_string(),
            protection,
            devices,
            sessions,
            groups,
            incomplete_session: state.incomplete_session.is_some(),
            notes,
        }
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
