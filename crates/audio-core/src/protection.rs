use crate::{
    config::AppConfig,
    devices::{set_default_endpoint, AudioDevice, DefaultRole, DeviceService},
    error::{AudioError, Result},
    grouping::{group_sessions, AppAudioGroup},
    mixer::{MixSettings, MixTelemetry, ProcessMixer},
    persist::{IncompleteSession, StateStore},
    policy::clear_app_default_endpoint,
    sessions::{AudioSessionInfo, SessionService},
    types::{AppIdentity, ProtectionMode},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    pub muted_feedback_sessions: Vec<String>,
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
    pub process_capture_supported: bool,
    pub processor: Option<crate::processor::ProcessorRoute>,
    pub remote_capture_ready: bool,
    pub remote_backend: String,
    pub integration_pending: bool,
}

pub struct ProtectionEngine {
    inner: Mutex<EngineInner>,
}
struct EngineInner {
    config: AppConfig,
    store: StateStore,
    mixer: Option<ProcessMixer>,
    unified: Option<crate::unified::UnifiedStage>,
    shared_name: Option<String>,
    warnings: Vec<String>,
    enabled: bool,
    managed_remote: bool,
}
impl ProtectionEngine {
    pub fn initialize() -> Result<Self> {
        let store = StateStore::open_default()?;
        let mut state = store.load()?;
        let mut warnings = Vec::new();
        if let Some(incomplete) = &state.incomplete_session {
            restore_legacy(incomplete)?;
            warnings.push("Se restauró la configuración de la versión anterior.".into());
            state.incomplete_session = None;
            store.save(&state)?;
        }
        state.config.monitor = "none".into();
        if state.config.migrate_startup_default() { store.save(&state)?; }
        if state.config.start_with_windows {if let Err(e)=crate::startup::configure(true){warnings.push(e.to_string());}}
        let engine=Self {
            inner: Mutex::new(EngineInner {
                config: state.config,
                store,
                mixer: None,
                unified: None,
                shared_name: None,
                warnings,
                enabled: false,
                managed_remote: false,
            }),
        };
        if let Ok(d)=crate::parsec::remote_device(){
            if crate::parsec::ready(&d.id){
                let mut inner=engine.inner.lock();
                let sessions=SessionService::new().list_capture_sessions()?;
                let conflicts=remote_channel_conflicts(&d,&sessions);
                if !conflicts.is_empty(){
                    inner.warnings.push(format!("La salida interna de Parsec está ocupada por: {}.",conflicts.join(", ")));
                    drop(inner);return Ok(engine);
                }
                inner.config.preferred_shared_device_id=Some(d.id.clone());
                let mut pass=settings(&inner.config);pass.excluded.clear();
                match ProcessMixer::start(d.id,pass){
                    Ok(m)=>{inner.mixer=Some(m);inner.shared_name=Some(d.name);inner.managed_remote=true;}
                    Err(e)=>inner.warnings.push(e.to_string()),
                }
            }
        }
        Ok(engine)
    }
    pub fn config(&self) -> AppConfig {
        self.inner.lock().config.clone()
    }
    pub fn update_config(&self, mut config: AppConfig) -> Result<()> {
        config.excluded_apps = sanitize(config.excluded_apps);
        validate_config(&config)?;
        let mut inner = self.inner.lock();
        if inner.managed_remote && config.preferred_shared_device_id != inner.config.preferred_shared_device_id {
            return Err(AudioError::message("La salida interna de Parsec se configura automáticamente. No cambies el cable del micrófono."));
        }
        if config.microphone_to_remote && !inner.config.microphone_to_remote && crate::processor::read().is_ok(){
            return Err(AudioError::message("UnifiedAudio ya entrega tu micrófono a la llamada. NoEcho no lo duplica en Parsec."));
        }
        if (inner.mixer.is_some() || inner.unified.is_some())
            && config.preferred_shared_device_id != inner.config.preferred_shared_device_id
        {
            return Err(AudioError::message(
                "Detén la mezcla antes de cambiar el canal remoto.",
            ));
        }
        let exclusions_changed=serde_json::to_string(&config.excluded_apps)?!=serde_json::to_string(&inner.config.excluded_apps)?;
        if exclusions_changed || config.monitor!=inner.config.monitor || config.microphone_to_remote!=inner.config.microphone_to_remote {
            if let Some(m) = &inner.mixer {
                let mut next=settings(&config);if !inner.enabled{next.excluded.clear();}m.update(next)?;
            }
        }
        if exclusions_changed {
            if let Some(u) = &inner.unified {u.update(&config.excluded_apps)?;}
        }
        let startup_changed=config.start_with_windows!=inner.config.start_with_windows;
        if startup_changed {crate::startup::configure(config.start_with_windows)?;}
        if let Err(e)=persist(&inner.store, &config){if startup_changed{let _=crate::startup::configure(inner.config.start_with_windows);}return Err(e);}
        inner.config = config;
        Ok(())
    }
    pub fn set_excluded_apps(&self, apps: Vec<AppIdentity>) -> Result<()> {
        let mut inner = self.inner.lock();
        let mut next = inner.config.clone();
        next.excluded_apps = sanitize(apps);
        if let Some(m) = &inner.mixer {
            let mut mix=settings(&next);if !inner.enabled{mix.excluded.clear();}m.update(mix)?;
        }
        if let Some(u) = &inner.unified { u.update(&next.excluded_apps)?; }
        persist(&inner.store, &next)?;
        inner.config = next;
        Ok(())
    }
    pub fn list_devices(&self) -> Result<Vec<AudioDevice>> {
        DeviceService::new().list_render_devices()
    }
    pub fn list_sessions(&self) -> Result<Vec<AudioSessionInfo>> {
        SessionService::new().list_sessions()
    }
    pub fn list_app_groups(&self) -> Result<Vec<AppAudioGroup>> {
        let config = self.config();
        let mut groups = group_sessions(&self.list_sessions()?, &config.excluded_apps);
        for app in &config.excluded_apps {
            if groups
                .iter()
                .any(|g| g.exe_name.eq_ignore_ascii_case(&app.exe_name))
            {
                continue;
            }
            groups.push(AppAudioGroup {
                id: app.exe_name.clone(),
                identity: app.clone(),
                display_name: app.display_name.clone().unwrap_or(app.exe_name.clone()),
                exe_name: app.exe_name.clone(),
                exe_path: app.exe_path.clone(),
                icon_data_url: None,
                state: crate::types::PlaybackState::Inactive,
                session_count: 0,
                pids: vec![],
                excluded: true,
                is_system: false,
                is_critical: false,
                volume: 0.0,
                device_names: vec![],
            });
        }
        Ok(groups)
    }
    pub fn status(&self) -> ProtectionStatus {
        status(&self.inner.lock())
    }
    pub fn telemetry(&self) -> MixTelemetry {
        let i = self.inner.lock();
        combined_telemetry(&i)
    }
    pub fn activate(&self, selected: Option<Vec<AppIdentity>>) -> Result<ProtectionStatus> {
        let mut inner = self.inner.lock();
        let mut config = inner.config.clone();
        if let Some(apps) = selected {
            config.excluded_apps = sanitize(apps);
        }
        validate_config(&config)?;
        let sessions = SessionService::new().list_capture_sessions()?;
        let processor=crate::processor::detect(&sessions)?;
        if crate::parsec::installed(){
            let remote=crate::parsec::remote_device()?;
            let conflicts=remote_channel_conflicts(&remote,&sessions);
            if !conflicts.is_empty(){return Err(AudioError::message(format!("La salida interna de Parsec está ocupada por: {}. Cierra esa aplicación o cambia su salida.",conflicts.join(", "))));}
            if !crate::parsec::ready(&remote.id){return Err(AudioError::message("La preparacion de Parsec esta pendiente. NoEcho la aplicara automaticamente cuando Parsec termine de usar su configuracion."));}
            if processor.is_some() {
                if let Some(u)=&inner.unified{u.update(&config.excluded_apps)?;}
                else{inner.unified=Some(crate::unified::UnifiedStage::start(&config.excluded_apps)?);}
            }else{inner.unified=None;}
            config.preferred_shared_device_id=Some(remote.id.clone());
            if let Some(m)=&inner.mixer{m.update(settings(&config))?;}
            else{
                match ProcessMixer::start(remote.id,settings(&config)){
                    Ok(m)=>inner.mixer=Some(m),Err(e)=>{inner.unified=None;return Err(e);}
                }
            }
            persist(&inner.store, &config)?;
            inner.config = config;
            inner.shared_name = Some(remote.name);
            inner.enabled=true;inner.managed_remote=true;
            inner.warnings.clear();
            return Ok(status(&inner));
        }
        inner.unified = None;
        if let Some(m) = &inner.mixer {
            if m.telemetry().running {
                m.update(settings(&config))?;
                persist(&inner.store, &config)?;
                inner.config = config;
                inner.enabled=true;
                return Ok(status(&inner));
            }
        }
        inner.mixer = None;
        let service = DeviceService::new();
        let sessions = SessionService::new().list_capture_sessions()?;
        let original = service.find_shared_candidate(config.preferred_shared_device_id.as_deref())?;
        let shared = if config.preferred_shared_device_id.is_none() {
            service.shared_candidates()?.into_iter().find(|d| channel_available(d, &sessions)).or(original).ok_or_else(|| AudioError::message("Falta un canal virtual para NoEcho. Prepara el audio desde Ayuda."))?
        } else { original.ok_or_else(|| AudioError::message("El canal guardado no está disponible. Elige automáticamente en Ajustes."))? };
        if shared.is_default_multimedia || shared.is_default_communications {
            return Err(AudioError::message("El canal remoto es una salida predeterminada de Windows. Selecciona tus altavoces o auriculares en Windows; NoEcho conserva esa salida."));
        }
        let conflicts: Vec<_> = sessions.into_iter().filter(|s| {
            s.device_id.as_deref() == Some(&shared.id)
                && s.pid != std::process::id()
                && s.pid != 0
                && matches!(s.state, crate::types::PlaybackState::Active)
        }).map(|s| s.display_name).collect();
        if !conflicts.is_empty() {
            return Err(AudioError::message(format!("El canal remoto está ocupado por: {}. Elige un cable libre en Ajustes; conserva la salida de las otras aplicaciones.", conflicts.join(", "))));
        }
        // Save the chosen channel so subsequent starts and the receiver's setup agree.
        config.preferred_shared_device_id = Some(shared.id.clone());
        let mixer = ProcessMixer::start(shared.id, settings(&config))?;
        persist(&inner.store, &config)?;
        inner.config = config;
        inner.shared_name = Some(shared.name);
        inner.mixer = Some(mixer);
        inner.enabled=true;
        Ok(status(&inner))
    }
    pub fn deactivate(&self) -> Result<ProtectionStatus> {
        let mut inner = self.inner.lock();
        if inner.managed_remote{
            if let Some(m)=&inner.mixer{let mut pass=settings(&inner.config);pass.excluded.clear();pass.monitor="none".into();m.update(pass)?;}
        }else{inner.mixer=None;inner.shared_name=None;}
        inner.unified = None;
        inner.enabled=false;
        inner.config.monitor = "none".into();
        persist(&inner.store, &inner.config)?;
        Ok(status(&inner))
    }
    pub fn refresh_routes(&self) -> Result<()> {
        let inner = self.inner.lock();
        if let Some(m) = &inner.mixer {
            m.update(settings(&inner.config))?;
        }
        Ok(())
    }
    pub fn capture_plan(&self) -> Result<crate::loopback::LoopbackCapturePlan> {
        let inner = self.inner.lock();
        let sessions = SessionService::new().list_sessions()?;
        let pids: Vec<_> = sessions
            .iter()
            .filter(|s| {
                s.exe_name.as_ref().is_some_and(|n| {
                    inner
                        .config
                        .excluded_apps
                        .iter()
                        .any(|a| a.exe_name.eq_ignore_ascii_case(n))
                })
            })
            .map(|s| s.pid)
            .collect();
        Ok(crate::loopback::plan_shared_capture(&pids))
    }
    pub fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut self.inner.lock().warnings)
    }
}
impl Drop for ProtectionEngine {
    fn drop(&mut self) {
        self.inner.get_mut().mixer = None;
        self.inner.get_mut().unified = None;
    }
}
pub type SharedEngine = Arc<ProtectionEngine>;
pub fn shared_engine() -> Result<SharedEngine> {
    Ok(Arc::new(ProtectionEngine::initialize()?))
}
fn settings(c: &AppConfig) -> MixSettings {
    MixSettings {
        excluded: c.excluded_apps.clone(),
        microphone_to_remote: c.microphone_to_remote && crate::processor::read().is_err(),
        monitor: c.monitor.clone(),
        processor_output: None,
        processed_microphone: crate::processor::read().ok().map(|r|r.output_id),
    }
}
fn channel_available(device: &AudioDevice, sessions: &[AudioSessionInfo]) -> bool {
    !device.is_default_multimedia && !device.is_default_communications && !sessions.iter().any(|s|
        s.device_id.as_deref() == Some(&device.id) && s.pid != 0 && s.pid != std::process::id()
        && s.state == crate::types::PlaybackState::Active)
}

fn remote_channel_conflicts(device:&AudioDevice,sessions:&[AudioSessionInfo])->Vec<String>{
    if device.is_default_multimedia || device.is_default_communications {return vec!["la salida predeterminada de Windows".into()];}
    sessions.iter().filter(|s|{
        if s.device_id.as_deref()!=Some(&device.id) || s.pid==0 || s.pid==std::process::id() || s.state!=crate::types::PlaybackState::Active {return false;}
        !s.exe_name.as_deref().is_some_and(is_parsec_output_process)
    }).map(|s|s.display_name.clone()).collect()
}

fn is_parsec_output_process(name:&str)->bool{
    ["parsecd.exe","steam.exe"].iter().any(|allowed|name.eq_ignore_ascii_case(allowed))
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn occupied_and_default_channels_are_not_available() {
        let mut device = AudioDevice {id:"cable".into(),name:"CABLE Input".into(),description:None,is_default_multimedia:false,is_default_communications:false,is_virtual_shared_candidate:true,is_physical_candidate:false,state:1};
        let mut session = AudioSessionInfo {session_id:"s".into(),pid:123,display_name:"Busy".into(),exe_path:None,exe_name:None,icon_path:None,icon_data_url:None,state:crate::types::PlaybackState::Active,is_system_sounds:false,volume:1.0,muted:false,device_id:Some("cable".into()),device_name:None};
        assert!(!channel_available(&device, &[session.clone()]));
        session.muted = true;
        assert!(!channel_available(&device, &[session.clone()]));
        session.state = crate::types::PlaybackState::Inactive;
        assert!(channel_available(&device, &[session.clone()]));
        session.state = crate::types::PlaybackState::Expired;
        assert!(channel_available(&device, &[session]));
        device.is_default_multimedia=true;
        assert!(!channel_available(&device, &[]));
    }
    #[test]
    fn parsec_output_sessions_are_allowed_but_other_audio_is_not(){
        assert!(is_parsec_output_process("parsecd.exe"));
        assert!(is_parsec_output_process("Steam.exe"));
        assert!(!is_parsec_output_process("telegram.exe"));
    }
}
fn sanitize(apps: Vec<AppIdentity>) -> Vec<AppIdentity> {
    let mut result = Vec::new();
    for mut a in apps {
        a.exe_name = a.exe_name.trim().to_ascii_lowercase();
        if a.exe_name.is_empty()
            || crate::process::is_critical_system_process(&a.exe_name)
            || result
                .iter()
                .any(|b: &AppIdentity| b.exe_name == a.exe_name)
        {
            continue;
        }
        result.push(a);
    }
    result
}
fn validate_config(c: &AppConfig) -> Result<()> {
    if !["none", "voice", "system", "private", "remote"].contains(&c.monitor.as_str()) {
        return Err(AudioError::message(
            "monitor: usa none, voice, system, private o remote.",
        ));
    }
    Ok(())
}
fn persist(store: &StateStore, config: &AppConfig) -> Result<()> {
    let mut state = store.load()?;
    state.config = config.clone();
    state.incomplete_session = None;
    store.save(&state)
}
fn combined_telemetry(i:&EngineInner)->MixTelemetry{
    merge_signals(i.mixer.as_ref().map(|m|m.telemetry()).unwrap_or_default(),i.unified.as_ref().map(|u|u.telemetry()))
}
fn merge_signals(mut t:MixTelemetry,mic:Option<MixTelemetry>)->MixTelemetry{
    if let Some(mic)=mic{
        t.running &= mic.running;
        t.errors.extend(mic.errors);
        // The virtual microphone may contain voice, PC audio, or both.
        t.voice_peak=mic.remote_peak;
        t.microphone_name="Micrófono virtual de UnifiedAudio".into();
    }
    t.processor_source=None;
    t
}
#[cfg(test)] mod signal_tests{
    use super::*;
    #[test] fn virtual_microphone_mode_does_not_gate_remote_programs(){
        let system=MixTelemetry{running:true,system_peak:0.4,private_peak:0.3,remote_peak:0.1,..Default::default()};
        // PC-only virtual microphone has no voice track but does have an output.
        let mic=MixTelemetry{running:true,voice_peak:0.0,remote_peak:0.7,..Default::default()};
        let result=merge_signals(system.clone(),Some(mic));
        assert_eq!(result.voice_peak,0.7);assert_eq!(result.system_peak,0.4);assert_eq!(result.private_peak,0.3);assert_eq!(result.remote_peak,0.1);
        // Voice-only virtual microphone leaves the independently captured PC meters intact.
        let result=merge_signals(system,Some(MixTelemetry{running:true,remote_peak:0.6,..Default::default()}));
        assert_eq!(result.voice_peak,0.6);assert_eq!(result.system_peak,0.4);assert!(result.running);
    }
}
fn status(i: &EngineInner) -> ProtectionStatus {
    let shared = DeviceService::new()
        .find_shared_candidate(i.config.preferred_shared_device_id.as_deref())
        .ok()
        .flatten();
    let telemetry = combined_telemetry(i);
    let remote_ready=!crate::parsec::installed() || crate::parsec::remote_device().is_ok_and(|d|crate::parsec::ready(&d.id));
    let mut warnings = i.warnings.clone();
    warnings.extend(telemetry.errors);
    ProtectionStatus {
        active: i.enabled && telemetry.running && remote_ready,
        mode: i.config.mode,
        message: if i.enabled && telemetry.running && remote_ready {
            "Mezcla remota por proceso activa. Usa el canal de NoEcho en la aplicación remota."
        } else {
            "Mezcla remota detenida."
        }
        .into(),
        excluded_count: i.config.excluded_apps.len(),
        excluded_apps: i.config.excluded_apps.clone(),
        physical_device_name: Some("Cada aplicación conserva su salida local".into()),
        shared_device_name: i
            .shared_name
            .clone()
            .or_else(|| shared.as_ref().map(|s| s.name.clone())),
        shared_device_available: shared.is_some(),
        warnings,
        snapshot: None,
        process_capture_supported: crate::loopback::process_loopback_supported(),
        processor: SessionService::new().list_capture_sessions().ok().and_then(|s| crate::processor::detect(&s).ok().flatten()),
        remote_capture_ready: remote_ready,
        remote_backend: if crate::parsec::installed(){"parsec"}else{"manual"}.into(),
        integration_pending: dirs::data_local_dir().is_some_and(|p|p.join("NoEcho/parsec-pending").exists() || p.join("NoEcho/pending-integration/engine.exe").exists()),
    }
}
fn restore_legacy(s: &IncompleteSession) -> Result<()> {
    if let Some(id) = &s.previous_default_multimedia_id {
        set_default_endpoint(id, DefaultRole::Multimedia)?;
    }
    if let Some(id) = &s.previous_default_communications_id {
        set_default_endpoint(id, DefaultRole::Communications)?;
    }
    for a in &s.excluded_apps {
        if let Some(path) = &a.exe_path {
            clear_app_default_endpoint(path)?;
        }
    }
    for id in &s.muted_feedback_sessions {
        SessionService::new().set_session_muted(id, false)?;
    }
    Ok(())
}
