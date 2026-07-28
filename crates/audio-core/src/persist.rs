use crate::config::AppConfig;
use crate::error::{AudioError, Result};
use crate::types::AppIdentity;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedState {
    pub config: AppConfig,
    pub incomplete_session: Option<IncompleteSession>,
    pub last_protection: Option<LastProtectionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncompleteSession {
    pub created_at: String,
    pub previous_default_multimedia_id: Option<String>,
    pub previous_default_communications_id: Option<String>,
    pub physical_device_id: Option<String>,
    pub shared_device_id: Option<String>,
    pub excluded_apps: Vec<AppIdentity>,
    #[serde(default)]
    pub muted_feedback_sessions: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastProtectionInfo {
    pub active: bool,
    pub updated_at: String,
    pub excluded_apps: Vec<AppIdentity>,
    pub physical_device_id: Option<String>,
    pub shared_device_id: Option<String>,
}

pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn default_path() -> Result<PathBuf> {
        let base = dirs::data_local_dir()
            .ok_or_else(|| AudioError::message("no se pudo resolver AppData\\Local"))?;
        Ok(base.join("NoEcho").join("state.json"))
    }

    pub fn logs_dir() -> Result<PathBuf> {
        let base = dirs::data_local_dir()
            .ok_or_else(|| AudioError::message("no se pudo resolver AppData\\Local"))?;
        Ok(base.join("NoEcho").join("logs"))
    }

    pub fn open_default() -> Result<Self> {
        Self::open(Self::default_path()?)
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<PersistedState> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let text = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, state: &PersistedState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(state)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
