use crate::state::AppState;
use audio_core::config::AppConfig;
use audio_core::devices::AudioDevice;
use audio_core::grouping::AppAudioGroup;
use audio_core::protection::ProtectionStatus;
use audio_core::report::DiagnosticReport;
use audio_core::setup::{PrepareResult, SetupService, SetupStatus};
use audio_core::types::AppIdentity;
use tauri::State;

#[tauri::command]
pub fn list_app_groups(state: State<'_, AppState>) -> Result<Vec<AppAudioGroup>, String> {
    state.engine.list_app_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Result<Vec<AudioDevice>, String> {
    state.engine.list_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> ProtectionStatus {
    state.engine.status()
}

#[tauri::command]
pub fn get_setup_status() -> SetupStatus {
    SetupService::status()
}

#[tauri::command]
pub fn prepare_shared_audio() -> Result<PrepareResult, String> {
    SetupService::prepare_shared_audio().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.engine.config()
}

#[tauri::command]
pub fn update_config(state: State<'_, AppState>, config: AppConfig) -> Result<AppConfig, String> {
    state
        .engine
        .update_config(config)
        .map_err(|e| e.to_string())?;
    Ok(state.engine.config())
}

#[tauri::command]
pub fn activate_protection(
    state: State<'_, AppState>,
    apps: Vec<AppIdentity>,
) -> Result<ProtectionStatus, String> {
    state
        .engine
        .activate(Some(apps))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn deactivate_protection(state: State<'_, AppState>) -> Result<ProtectionStatus, String> {
    state.engine.deactivate().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn refresh_routes(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.refresh_routes().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_diagnostic_report(state: State<'_, AppState>) -> Result<String, String> {
    let devices = state.engine.list_devices().map_err(|e| e.to_string())?;
    let sessions = state.engine.list_sessions().map_err(|e| e.to_string())?;
    let groups = state.engine.list_app_groups().map_err(|e| e.to_string())?;
    let status = state.engine.status();
    let setup = SetupService::status();
    let store = audio_core::persist::StateStore::open_default().map_err(|e| e.to_string())?;
    let persisted = store.load().map_err(|e| e.to_string())?;
    let report = DiagnosticReport::build(
        status,
        devices,
        sessions,
        groups,
        &persisted,
        vec![
            "Informe de NoEcho".into(),
            "No incluye contenido de audio ni conversaciones.".into(),
            format!("setup_ready={}", setup.ready),
            format!("setup_state={:?}", setup.state),
        ],
    );
    report.to_pretty_json().map_err(|e| e.to_string())
}