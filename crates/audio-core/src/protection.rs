use crate::config::AppConfig;
use crate::devices::{set_default_endpoint, AudioDevice, DefaultRole, DeviceService};
use crate::error::{AudioError, Result};
use crate::grouping::{group_sessions, AppAudioGroup};
use crate::loopback::{plan_shared_capture, SharedMonitor};
use crate::persist::{IncompleteSession, LastProtectionInfo, StateStore};
use crate::policy::{
    clear_app_default_endpoint, set_process_default_endpoint, set_process_default_endpoints,
};
use crate::process::{expand_related_pids, is_critical_system_process, process_image_path};
use crate::sessions::{AudioSessionInfo, SessionService};
use crate::types::{AppIdentity, ProtectionMode};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionSnapshot {
    pub previous_default_multimedia_id: Option<String>,
    pub previous_default_communications_id: Option<String>,
    pub physical_device_id: String,
    pub physical_device_name: String,
    pub communications_device_id: String,
    pub communications_device_name: String,
    pub shared_device_id: String,
    pub shared_device_name: String,
    pub excluded_apps: Vec<AppIdentity>,
    pub routed_app_paths: Vec<String>,
    pub activated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionStatus {
    pub active: bool,
    pub mode: ProtectionMode,
    pub message: String,
    pub excluded_count: usize,
    pub excluded_apps: Vec<AppIdentity>,
    pub physical_device_name: Option<String>,
    pub shared_device_name: Option<String>,
    pub shared_device_available: bool,
    pub warnings: Vec<String>,
    pub snapshot: Option<ProtectionSnapshot>,
}

pub struct ProtectionEngine {
    inner: Mutex<EngineInner>,
}

struct EngineInner {
    config: AppConfig,
    store: StateStore,
    device_service: DeviceService,
    session_service: SessionService,
    active: bool,
    snapshot: Option<ProtectionSnapshot>,
    monitor: Option<SharedMonitor>,
    warnings: Vec<String>,
}

impl ProtectionEngine {
    pub fn initialize() -> Result<Self> {
        let store = StateStore::open_default()?;
        let mut state = store.load().unwrap_or_default();
        let mut warnings = Vec::new();

        // Older builds used Spanish as the implicit language and had no language
        // selector. Migrate that one-time default, while preserving any choice
        // made after the selector has been used.
        if !state.config.language_migrated {
            if state.config.language == "es" {
                state.config.language = "en".into();
            }
            state.config.language_migrated = true;
            let _ = store.save(&state);
        }

        if state.config.auto_recover_on_start {
            if let Some(incomplete) = state.incomplete_session.clone() {
                warn!("sesion incompleta detectada; restaurando audio");
                if let Err(e) = restore_defaults_from_incomplete(&incomplete) {
                    warnings.push(format!(
                        "No se pudo restaurar completamente la sesion anterior: {e}"
                    ));
                } else {
                    warnings.push(
                        "Se detecto un cierre inesperado. La configuracion de audio fue restaurada."
                            .into(),
                    );
                }
                state.incomplete_session = None;
                let _ = store.save(&state);
            }
        }

        Ok(Self {
            inner: Mutex::new(EngineInner {
                config: state.config,
                store,
                device_service: DeviceService::new(),
                session_service: SessionService::new(),
                active: false,
                snapshot: None,
                monitor: None,
                warnings,
            }),
        })
    }

    pub fn config(&self) -> AppConfig {
        self.inner.lock().config.clone()
    }

    pub fn update_config(&self, config: AppConfig) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.config = config;
        persist_locked(&mut inner)?;
        Ok(())
    }

    pub fn list_devices(&self) -> Result<Vec<AudioDevice>> {
        self.inner.lock().device_service.list_render_devices()
    }

    pub fn list_sessions(&self) -> Result<Vec<AudioSessionInfo>> {
        self.inner.lock().session_service.list_sessions()
    }

    pub fn list_app_groups(&self) -> Result<Vec<AppAudioGroup>> {
        let inner = self.inner.lock();
        let sessions = inner.session_service.list_sessions()?;
        let excluded = current_excluded(&inner);
        let mut groups = group_sessions(&sessions, &excluded);
        if !inner.config.show_inactive_recent {
            groups.retain(|g| {
                matches!(
                    g.state,
                    crate::types::PlaybackState::Active | crate::types::PlaybackState::Inactive
                ) || g.excluded
            });
        }
        Ok(groups)
    }

    pub fn status(&self) -> ProtectionStatus {
        let inner = self.inner.lock();
        status_from_inner(&inner)
    }

    pub fn set_excluded_apps(&self, apps: Vec<AppIdentity>) -> Result<()> {
        let mut filtered = Vec::new();
        for app in apps {
            if is_critical_system_process(&app.exe_name) {
                continue;
            }
            filtered.push(app);
        }
        let mut inner = self.inner.lock();
        inner.config.excluded_apps = filtered;
        if inner.active {
            let excluded = inner.config.excluded_apps.clone();
            let snap = inner
                .snapshot
                .as_ref()
                .ok_or(AudioError::ProtectionNotActive)?
                .clone();
            apply_routes(
                &inner.session_service,
                &excluded,
                &snap.shared_device_id,
                &snap.physical_device_id,
                &snap.communications_device_id,
            )?;
            if let Some(s) = inner.snapshot.as_mut() {
                s.excluded_apps = excluded;
            }
        }
        persist_locked(&mut inner)?;
        Ok(())
    }

    pub fn activate(&self, selected: Option<Vec<AppIdentity>>) -> Result<ProtectionStatus> {
        let mut inner = self.inner.lock();
        if inner.active {
            return Err(AudioError::ProtectionAlreadyActive);
        }

        if let Some(selected) = selected {
            inner.config.excluded_apps = selected
                .into_iter()
                .filter(|a| !is_critical_system_process(&a.exe_name))
                .collect();
        }

        if inner.config.excluded_apps.is_empty() {
            return Err(AudioError::message(
                "Elige al menos una aplicacion de la lista y vuelve a intentarlo.",
            ));
        }

        let devices = inner.device_service.list_render_devices()?;
        let previous_mm = devices.iter().find(|d| d.is_default_multimedia).cloned();
        let previous_comm = devices
            .iter()
            .find(|d| d.is_default_communications)
            .cloned();

        // The shared-audio monitor always returns normal system sound to the
        // output Windows was already using. Choosing a private output must not
        // move the rest of the host computer's audio.
        let local_monitor = inner.device_service.choose_physical(
            previous_mm.as_ref().map(|d| d.id.as_str()),
            true,
        )?;
        let physical = inner.device_service.choose_physical(
            inner
                .config
                .preferred_physical_device_id
                .as_deref()
                .or(Some(local_monitor.id.as_str())),
            true,
        )?;

        let shared = inner
            .device_service
            .find_shared_candidate(inner.config.preferred_shared_device_id.as_deref())?
            .ok_or_else(|| {
                AudioError::SharedDeviceUnavailable(
                    "Aun falta un paso de instalacion de audio. Abre NoEcho y sigue el aviso en pantalla: es algo que solo se hace una vez.".into(),
                )
            })?;

        // In automatic mode preserve the pre-existing split between general
        // audio and calls. An explicit private-output choice applies to every
        // role of the selected app, as requested by the user.
        let communications = if inner.config.preferred_physical_device_id.is_some() {
            physical.clone()
        } else {
            previous_comm
                .as_ref()
                .filter(|d| d.is_physical_candidate && d.id != shared.id)
                .unwrap_or(&physical)
                .clone()
        };

        if shared.id == physical.id {
            return Err(AudioError::message(
                "Hay un problema de configuracion de audio. Prueba reiniciar NoEcho o elige otra salida en Avanzado.",
            ));
        }

        let incomplete = IncompleteSession {
            created_at: chrono::Local::now().to_rfc3339(),
            previous_default_multimedia_id: previous_mm.as_ref().map(|d| d.id.clone()),
            previous_default_communications_id: previous_comm.as_ref().map(|d| d.id.clone()),
            physical_device_id: Some(physical.id.clone()),
            shared_device_id: Some(shared.id.clone()),
            excluded_apps: inner.config.excluded_apps.clone(),
            reason: "activation-in-progress".into(),
        };
        {
            let mut state = inner.store.load().unwrap_or_default();
            state.config = inner.config.clone();
            state.incomplete_session = Some(incomplete);
            inner.store.save(&state)?;
        }

        if let Err(e) = set_default_endpoint(&shared.id, DefaultRole::Multimedia) {
            let _ = restore_defaults(
                previous_mm.as_ref().map(|d| d.id.as_str()),
                previous_comm.as_ref().map(|d| d.id.as_str()),
            );
            clear_incomplete(&inner.store);
            return Err(AudioError::message(format!(
                "No se pudo activar. No te preocupes: tu audio se dejo como estaba. {e}"
            )));
        }
        // Keep Windows' global communications output untouched. Selected apps
        // are routed explicitly below, including that role.

        let route_result = apply_routes(
            &inner.session_service,
            &inner.config.excluded_apps,
            &shared.id,
            &physical.id,
            &communications.id,
        );

        match route_result {
            Ok(paths) => {
                let mut warnings = Vec::new();
                let monitor = match SharedMonitor::start(
                    shared.id.clone(),
                    local_monitor.id.clone(),
                ) {
                    Ok(mon) => Some(mon),
                    Err(e) => {
                        warnings.push(format!(
                            "La exclusion se aplico, pero el monitor local fallo: {e}."
                        ));
                        None
                    }
                };

                let snapshot = ProtectionSnapshot {
                    previous_default_multimedia_id: previous_mm.map(|d| d.id),
                    previous_default_communications_id: previous_comm.map(|d| d.id),
                    physical_device_id: physical.id.clone(),
                    physical_device_name: physical.name.clone(),
                    communications_device_id: communications.id.clone(),
                    communications_device_name: communications.name.clone(),
                    shared_device_id: shared.id.clone(),
                    shared_device_name: shared.name.clone(),
                    excluded_apps: inner.config.excluded_apps.clone(),
                    routed_app_paths: paths,
                    activated_at: chrono::Local::now().to_rfc3339(),
                };

                inner.monitor = monitor;
                inner.snapshot = Some(snapshot.clone());
                inner.active = true;
                inner.warnings = warnings;

                let mut state = inner.store.load().unwrap_or_default();
                state.config = inner.config.clone();
                state.incomplete_session = Some(IncompleteSession {
                    created_at: snapshot.activated_at.clone(),
                    previous_default_multimedia_id: snapshot.previous_default_multimedia_id.clone(),
                    previous_default_communications_id: snapshot
                        .previous_default_communications_id
                        .clone(),
                    physical_device_id: Some(snapshot.physical_device_id.clone()),
                    shared_device_id: Some(snapshot.shared_device_id.clone()),
                    excluded_apps: snapshot.excluded_apps.clone(),
                    reason: "protection-active".into(),
                });
                state.last_protection = Some(LastProtectionInfo {
                    active: true,
                    updated_at: chrono::Local::now().to_rfc3339(),
                    excluded_apps: snapshot.excluded_apps.clone(),
                    physical_device_id: Some(snapshot.physical_device_id.clone()),
                    shared_device_id: Some(snapshot.shared_device_id.clone()),
                });
                inner.store.save(&state)?;
                info!(
                    "proteccion activa: {} apps privadas, shared={}, physical={}",
                    snapshot.excluded_apps.len(),
                    snapshot.shared_device_name,
                    snapshot.physical_device_name
                );
            }
            Err(e) => {
                let _ = restore_defaults(
                    previous_mm.as_ref().map(|d| d.id.as_str()),
                    previous_comm.as_ref().map(|d| d.id.as_str()),
                );
                clear_incomplete(&inner.store);
                return Err(AudioError::message(format!(
                    "No se pudo activar. No te preocupes: tu audio se dejo como estaba. {e} Tu audio continua funcionando normalmente."
                )));
            }
        }

        Ok(status_from_inner(&inner))
    }

    pub fn deactivate(&self) -> Result<ProtectionStatus> {
        let mut inner = self.inner.lock();
        if !inner.active {
            clear_incomplete(&inner.store);
            return Ok(status_from_inner(&inner));
        }

        if let Some(mon) = inner.monitor.take() {
            mon.stop();
        }

        if let Some(snapshot) = inner.snapshot.take() {
            for path in &snapshot.routed_app_paths {
                let _ = clear_app_default_endpoint(path);
            }
            let _ = restore_defaults(
                snapshot.previous_default_multimedia_id.as_deref(),
                snapshot.previous_default_communications_id.as_deref(),
            );
        }

        inner.active = false;
        inner.warnings.clear();
        clear_incomplete(&inner.store);

        let mut state = inner.store.load().unwrap_or_default();
        state.config = inner.config.clone();
        state.last_protection = Some(LastProtectionInfo {
            active: false,
            updated_at: chrono::Local::now().to_rfc3339(),
            excluded_apps: inner.config.excluded_apps.clone(),
            physical_device_id: inner.config.preferred_physical_device_id.clone(),
            shared_device_id: inner.config.preferred_shared_device_id.clone(),
        });
        inner.store.save(&state)?;
        info!("proteccion desactivada; audio restaurado");
        Ok(status_from_inner(&inner))
    }

    pub fn refresh_routes(&self) -> Result<()> {
        let inner = self.inner.lock();
        if !inner.active {
            return Ok(());
        }
        let snap = inner
            .snapshot
            .as_ref()
            .ok_or(AudioError::ProtectionNotActive)?;
        apply_routes(
            &inner.session_service,
            &snap.excluded_apps,
            &snap.shared_device_id,
            &snap.physical_device_id,
            &snap.communications_device_id,
        )?;
        Ok(())
    }

    pub fn capture_plan(&self) -> Result<crate::loopback::LoopbackCapturePlan> {
        let inner = self.inner.lock();
        let mut pids = BTreeSet::new();
        let apps = current_excluded(&inner);
        let sessions = inner.session_service.list_sessions()?;
        for app in &apps {
            let seed: Vec<u32> = sessions
                .iter()
                .filter(|s| {
                    s.exe_name
                        .as_ref()
                        .map(|n| n.eq_ignore_ascii_case(&app.exe_name))
                        .unwrap_or(false)
                })
                .map(|s| s.pid)
                .collect();
            for pid in expand_related_pids(&seed, Some(&app.exe_name)).unwrap_or(seed) {
                pids.insert(pid);
            }
        }
        Ok(plan_shared_capture(&pids.into_iter().collect::<Vec<_>>()))
    }

    pub fn take_warnings(&self) -> Vec<String> {
        let mut inner = self.inner.lock();
        std::mem::take(&mut inner.warnings)
    }
}

impl Drop for ProtectionEngine {
    fn drop(&mut self) {
        let _ = self.deactivate();
    }
}

pub type SharedEngine = Arc<ProtectionEngine>;

pub fn shared_engine() -> Result<SharedEngine> {
    Ok(Arc::new(ProtectionEngine::initialize()?))
}

fn current_excluded(inner: &EngineInner) -> Vec<AppIdentity> {
    if inner.active {
        inner
            .snapshot
            .as_ref()
            .map(|s| s.excluded_apps.clone())
            .unwrap_or_else(|| inner.config.excluded_apps.clone())
    } else {
        inner.config.excluded_apps.clone()
    }
}

fn status_from_inner(inner: &EngineInner) -> ProtectionStatus {
    let shared = inner
        .device_service
        .find_shared_candidate(inner.config.preferred_shared_device_id.as_deref())
        .ok()
        .flatten();
    let excluded = current_excluded(inner);
    let message = if inner.active {
        if excluded.is_empty() {
            "Protecci?n activa".into()
        } else if excluded.len() == 1 {
            let name = excluded[0]
                .display_name
                .clone()
                .unwrap_or_else(|| excluded[0].exe_name.clone());
            format!("{name} solo se oye en esta computadora.")
        } else {
            format!(
                "{} aplicaciones solo se oyen en esta computadora.",
                excluded.len()
            )
        }
    } else {
        "Listo para proteger".into()
    };

    ProtectionStatus {
        active: inner.active,
        mode: inner.config.mode,
        message,
        excluded_count: excluded.len(),
        excluded_apps: excluded,
        physical_device_name: inner.snapshot.as_ref().map(|s| s.physical_device_name.clone()),
        shared_device_name: inner
            .snapshot
            .as_ref()
            .map(|s| s.shared_device_name.clone())
            .or_else(|| shared.as_ref().map(|d| d.name.clone())),
        shared_device_available: shared.is_some(),
        warnings: inner.warnings.clone(),
        snapshot: inner.snapshot.clone(),
    }
}

fn persist_locked(inner: &mut EngineInner) -> Result<()> {
    let mut state = inner.store.load().unwrap_or_default();
    state.config = inner.config.clone();
    if let Some(snapshot) = &inner.snapshot {
        state.incomplete_session = Some(IncompleteSession {
            created_at: snapshot.activated_at.clone(),
            previous_default_multimedia_id: snapshot.previous_default_multimedia_id.clone(),
            previous_default_communications_id: snapshot.previous_default_communications_id.clone(),
            physical_device_id: Some(snapshot.physical_device_id.clone()),
            shared_device_id: Some(snapshot.shared_device_id.clone()),
            excluded_apps: snapshot.excluded_apps.clone(),
            reason: if inner.active {
                "protection-active".into()
            } else {
                "idle".into()
            },
        });
    }
    inner.store.save(&state)
}

fn apply_routes(
    sessions: &SessionService,
    excluded_apps: &[AppIdentity],
    shared_device_id: &str,
    physical_device_id: &str,
    communications_device_id: &str,
) -> Result<Vec<String>> {
    let list = sessions.list_sessions()?;
    let mut routed_paths = BTreeSet::new();
    let mut private_route_failures = Vec::new();
    let excluded_names: BTreeSet<String> = excluded_apps
        .iter()
        .map(|a| a.exe_name.to_ascii_lowercase())
        .collect();

    let mut private_pids = BTreeSet::new();
    for app in excluded_apps {
        let seed: Vec<u32> = list
            .iter()
            .filter(|s| {
                s.exe_name
                    .as_ref()
                    .map(|n| n.eq_ignore_ascii_case(&app.exe_name))
                    .unwrap_or(false)
            })
            .map(|s| s.pid)
            .collect();
        for pid in expand_related_pids(&seed, Some(&app.exe_name)).unwrap_or(seed) {
            private_pids.insert(pid);
        }
    }

    for session in &list {
        if session.is_system_sounds || session.pid == 0 {
            continue;
        }
        if is_microphone_chain_session(session) {
            // Do not redirect MicVST/Mic Mix or anything already using the
            // user's Cable A/B microphone chain.
            tracing::debug!(
                "se omite cadena de microfono: exe={:?}, device={:?}",
                session.exe_name,
                session.device_name
            );
            continue;
        }
        let Some(path) = session
            .exe_path
            .clone()
            .or_else(|| process_image_path(session.pid))
        else {
            continue;
        };
        let exe = session
            .exe_name
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if is_critical_system_process(&exe) {
            continue;
        }

        let is_private = private_pids.contains(&session.pid)
            || excluded_names.contains(&exe)
            || excluded_apps.iter().any(|a| a.matches_path(&path));

        // Route the concrete session PID through the exact Windows
        // AudioPolicyConfig method. The previous path-based implementation
        // guessed COM vtable slots and could terminate the selected app.
        let route_result = if is_private {
            set_process_default_endpoints(
                session.pid,
                physical_device_id,
                communications_device_id,
            )
        } else {
            set_process_default_endpoint(session.pid, shared_device_id)
        };
        match route_result {
            Ok(()) => {
                routed_paths.insert(path);
            }
            Err(e) => {
                tracing::debug!("route path failed for {path}: {e}");
                if is_private {
                    private_route_failures.push(format!("{}: {e}", session.display_name));
                }
            }
        }
    }

    if !private_route_failures.is_empty() {
        return Err(AudioError::message(format!(
            "No se pudo separar de forma segura: {}",
            private_route_failures.join(", ")
        )));
    }

    for app in excluded_apps {
        if let Some(path) = &app.exe_path {
            // The active-session loop above normally handles this. For an app
            // with no current session, leave it alone; it will be picked up by
            // refresh_routes when Windows creates its audio session.
            tracing::debug!("private app has no active session yet: {path}");
        }
    }

    Ok(routed_paths.into_iter().collect())
}

fn is_microphone_chain_session(session: &AudioSessionInfo) -> bool {
    let exe = session
        .exe_name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if exe.contains("micvst") || exe.contains("mic-mix") || exe.contains("mic_mixer") {
        return true;
    }

    let device = session
        .device_name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    device.contains("cable-a")
        || device.contains("cable a")
        || device.contains("cable-b")
        || device.contains("cable b")
}

fn restore_defaults(mm: Option<&str>, comm: Option<&str>) -> Result<()> {
    if let Some(id) = mm {
        set_default_endpoint(id, DefaultRole::Multimedia)?;
    }
    if let Some(id) = comm {
        let _ = set_default_endpoint(id, DefaultRole::Communications);
    }
    Ok(())
}

fn restore_defaults_from_incomplete(incomplete: &IncompleteSession) -> Result<()> {
    restore_defaults(
        incomplete.previous_default_multimedia_id.as_deref(),
        incomplete.previous_default_communications_id.as_deref(),
    )?;
    for app in &incomplete.excluded_apps {
        if let Some(path) = &app.exe_path {
            let _ = clear_app_default_endpoint(path);
        }
    }
    Ok(())
}

fn clear_incomplete(store: &StateStore) {
    if let Ok(mut state) = store.load() {
        state.incomplete_session = None;
        if let Some(last) = state.last_protection.as_mut() {
            last.active = false;
            last.updated_at = chrono::Local::now().to_rfc3339();
        }
        let _ = store.save(&state);
    }
}
