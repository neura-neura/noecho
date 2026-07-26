use crate::process::is_critical_system_process;
use crate::sessions::AudioSessionInfo;
use crate::types::{AppIdentity, PlaybackState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppAudioGroup {
    pub id: String,
    pub identity: AppIdentity,
    pub display_name: String,
    pub exe_name: String,
    pub exe_path: Option<String>,
    pub icon_data_url: Option<String>,
    pub state: PlaybackState,
    pub session_count: usize,
    pub pids: Vec<u32>,
    pub excluded: bool,
    pub is_system: bool,
    pub is_critical: bool,
    pub volume: f32,
    pub device_names: Vec<String>,
}

/// Group audio sessions by application family (executable name / path).
pub fn group_sessions(sessions: &[AudioSessionInfo], excluded: &[AppIdentity]) -> Vec<AppAudioGroup> {
    let mut map: BTreeMap<String, Vec<&AudioSessionInfo>> = BTreeMap::new();

    for session in sessions {
        if session.is_system_sounds {
            map.entry("system-sounds".into()).or_default().push(session);
            continue;
        }
        let key = session
            .exe_name
            .as_ref()
            .map(|n| n.to_ascii_lowercase())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("pid:{}", session.pid));
        map.entry(key).or_default().push(session);
    }

    let mut groups = Vec::new();
    for (key, items) in map {
        let first = items[0];
        let exe_name = first
            .exe_name
            .clone()
            .unwrap_or_else(|| key.clone());
        let exe_path = items.iter().find_map(|s| s.exe_path.clone());
        let display_name = items
            .iter()
            .map(|s| s.display_name.clone())
            .find(|n| !n.is_empty() && !n.starts_with("Proceso "))
            .unwrap_or_else(|| first.display_name.clone());

        let state = if items.iter().any(|s| s.state == PlaybackState::Active) {
            PlaybackState::Active
        } else if items.iter().any(|s| s.state == PlaybackState::Inactive) {
            PlaybackState::Inactive
        } else if items.iter().any(|s| s.state == PlaybackState::Expired) {
            PlaybackState::Expired
        } else {
            PlaybackState::Unknown
        };

        let mut pids: Vec<u32> = items.iter().map(|s| s.pid).filter(|p| *p > 0).collect();
        pids.sort_unstable();
        pids.dedup();

        let identity = AppIdentity {
            exe_name: exe_name.to_ascii_lowercase(),
            exe_path: exe_path.clone(),
            display_name: Some(display_name.clone()),
        };

        let excluded_flag = excluded.iter().any(|e| {
            e.exe_name == identity.exe_name
                || e.exe_path
                    .as_ref()
                    .zip(identity.exe_path.as_ref())
                    .map(|(a, b)| a.eq_ignore_ascii_case(b))
                    .unwrap_or(false)
        });

        let icon_data_url = items.iter().find_map(|s| s.icon_data_url.clone());
        let volume = items
            .iter()
            .map(|s| s.volume)
            .fold(0.0_f32, f32::max);
        let mut device_names: Vec<String> = items
            .iter()
            .filter_map(|s| s.device_name.clone())
            .collect();
        device_names.sort();
        device_names.dedup();

        let is_system = first.is_system_sounds || key == "system-sounds";
        let is_critical = is_system || is_critical_system_process(&exe_name);

        groups.push(AppAudioGroup {
            id: identity.exe_name.clone(),
            identity,
            display_name,
            exe_name,
            exe_path,
            icon_data_url,
            state,
            session_count: items.len(),
            pids,
            excluded: excluded_flag,
            is_system,
            is_critical,
            volume,
            device_names,
        });
    }

    groups.sort_by(|a, b| {
        b.state_rank()
            .cmp(&a.state_rank())
            .then(b.excluded.cmp(&a.excluded))
            .then(
                a.display_name
                    .to_ascii_lowercase()
                    .cmp(&b.display_name.to_ascii_lowercase()),
            )
    });
    groups
}

impl AppAudioGroup {
    fn state_rank(&self) -> u8 {
        match self.state {
            PlaybackState::Active => 3,
            PlaybackState::Inactive => 2,
            PlaybackState::Expired => 1,
            PlaybackState::Unknown => 0,
        }
    }

    pub fn state_label(&self) -> String {
        if self.session_count > 1 && self.state == PlaybackState::Active {
            format!("{} sesiones de audio", self.session_count)
        } else {
            self.state.as_label().to_string()
        }
    }
}
