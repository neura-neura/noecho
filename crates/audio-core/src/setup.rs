use crate::devices::DeviceService;
use crate::error::{AudioError, Result};
use crate::SHARED_DEVICE_FRIENDLY_NAME;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetupState {
    Ready,
    NeedsPrepare,
    Preparing,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub state: SetupState,
    pub ready: bool,
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub shared_device_name: Option<String>,
    pub can_prepare_automatically: bool,
    pub prepare_button_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareResult {
    pub success: bool,
    pub status: SetupStatus,
    pub log: Vec<String>,
}

pub struct SetupService;

impl SetupService {
    pub fn status() -> SetupStatus {
        let devices = DeviceService::new().list_render_devices().unwrap_or_default();
        let shared = DeviceService::new()
            .find_shared_candidate(None)
            .ok()
            .flatten();

        if let Some(shared) = shared {
            return SetupStatus {
                state: SetupState::Ready,
                ready: true,
                title: "Todo listo".into(),
                message: "Ya se puede ocultar audio del escritorio remoto.".into(),
                detail: Some(format!("Canal detectado: {}", shared.name)),
                shared_device_name: Some(shared.name),
                can_prepare_automatically: false,
                prepare_button_label: "Preparar canal compartido (opcional)".into(),
            };
        }

        let has_ab = devices.iter().any(|d| {
            let n = d.name.to_ascii_lowercase();
            n.contains("cable-a") || n.contains("cable a") || n.contains("cable-b") || n.contains("cable b")
        });
        let payload = find_prepare_payload();
        let can = payload.is_some();
        let extra = if has_ab {
            " Detecte Cable A/B, pero NoEcho no los usara para no tocar tu microfono."
        } else {
            ""
        };
        SetupStatus {
            state: SetupState::NeedsPrepare,
            ready: false,
            title: "Opcional: preparar canal compartido".into(),
            message: if can {
                format!("Si esta PC no tiene un canal de audio compartido, puedes prepararlo una sola vez con el boton de abajo.{extra}")
            } else {
                format!("NoEcho funciona con el canal compartido que ya tengas en esta PC. Si no hay ninguno, puedes instalar uno o elegirlo en Opciones.{extra}")
            },
            detail: if devices.is_empty() {
                Some("No se detectaron salidas de audio.".into())
            } else {
                payload.map(|p| format!("Paquete encontrado: {}", p.display()))
            },
            shared_device_name: None,
            can_prepare_automatically: can,
            prepare_button_label: "Preparar canal compartido (opcional)".into(),
        }
    }

    /// Runs the one-time shared-audio preparation if a payload is available.
    pub fn prepare_shared_audio() -> Result<PrepareResult> {
        let mut log = Vec::new();
        let before = Self::status();
        if before.ready {
            log.push("El canal compartido ya estaba listo.".into());
            return Ok(PrepareResult {
                success: true,
                status: before,
                log,
            });
        }

        let payload = find_prepare_payload().ok_or_else(|| {
            AudioError::message(
                "No se encontró el paquete de preparación de audio. Usa el instalador completo de NoEcho.",
            )
        })?;
        log.push(format!("Usando paquete: {}", payload.display()));

        let work = prepare_work_dir()?;
        log.push(format!("Carpeta temporal: {}", work.display()));

        // Copy payload into work dir
        let staged = stage_payload(&payload, &work)?;
        log.push(format!("Paquete preparado: {}", staged.display()));

        // Prefer silent installers.
        let code = run_payload_installer(&staged, &mut log)?;
        log.push(format!("Instalador terminó con código {code}"));

        // Give Windows a moment to register endpoints.
        for attempt in 1..=10 {
            std::thread::sleep(Duration::from_millis(800));
            if DeviceService::new()
                .find_shared_candidate(None)
                .ok()
                .flatten()
                .is_some()
            {
                log.push(format!("Canal detectado en el intento {attempt}."));
                let mut status = Self::status();
                status.message =
                    "Listo. Ya puedes ocultar apps del escritorio remoto.".into();
                // Persist marker so uninstall can know we prepared audio.
                let _ = write_prepare_marker(true);
                return Ok(PrepareResult {
                    success: true,
                    status,
                    log,
                });
            }
        }

        let _ = write_prepare_marker(false);
        let mut status = Self::status();
        status.state = SetupState::Failed;
        status.title = "No se completó el preparativo".into();
        status.message = "Se ejecutó el preparativo, pero Windows todavía no muestra el canal compartido. Reinicia la PC e intenta de nuevo.".into();
        Ok(PrepareResult {
            success: false,
            status,
            log,
        })
    }
}

fn prepare_work_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| AudioError::message("No se pudo usar AppData"))?
        .join("NoEcho")
        .join("setup");
    fs::create_dir_all(&base)?;
    Ok(base)
}

fn write_prepare_marker(ok: bool) -> Result<()> {
    let path = dirs::data_local_dir()
        .ok_or_else(|| AudioError::message("No se pudo usar AppData"))?
        .join("NoEcho")
        .join("setup-marker.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::json!({
        "prepared": ok,
        "at": chrono::Local::now().to_rfc3339(),
        "shared_name": SHARED_DEVICE_FRIENDLY_NAME,
    });
    fs::write(path, serde_json::to_vec_pretty(&body)?)?;
    Ok(())
}

/// Search order for one-click audio payload:
/// 1. %LOCALAPPDATA%\NoEcho\payload
/// 2. next to the running executable
/// 3. installer/payload in current working directory / repo
fn find_prepare_payload() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = dirs::data_local_dir() {
        roots.push(local.join("NoEcho").join("payload"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("payload"));
            roots.push(dir.join("resources").join("payload"));
            roots.push(dir.join("resources"));
            roots.push(dir.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("installer").join("payload"));
        roots.push(cwd.join("payload"));
    }

    const NAMES: &[&str] = &[
        "NoEchoAudioSetup.exe",
        "VBCABLE_Setup_x64.exe",
        "VBCABLE_Setup.exe",
        "AudioCompartidoSetup.exe",
        "VBCABLE_Driver_Pack45.zip",
        "vb-cable-setup.exe",
        "vbcable.zip",
        "audio-setup.zip",
        "VBCABLE_A_Setup_x64.exe",
        "VBCABLE_B_Setup_x64.exe",
        "VBCABLE_A_Driver_Pack43.zip",
        "VBCABLE_B_Driver_Pack43.zip",
    ];

    for root in roots {
        if !root.exists() {
            continue;
        }
        if root.is_file() {
            return Some(root);
        }
        for name in NAMES {
            let candidate = root.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // Any exe/zip that looks like cable setup
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if name.contains("vbcable") || name.contains("vb-cable") || name.contains("audio")
                {
                    if name.ends_with(".exe") || name.ends_with(".zip") {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn stage_payload(payload: &Path, work: &Path) -> Result<PathBuf> {
    let ext = payload
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "zip" {
        let dest = work.join("extracted");
        if dest.exists() {
            let _ = fs::remove_dir_all(&dest);
        }
        fs::create_dir_all(&dest)?;
        extract_zip(payload, &dest)?;
        // Find setup exe inside
        if let Some(exe) = find_exe_in_dir(&dest) {
            return Ok(exe);
        }
        return Err(AudioError::message(
            "El paquete ZIP no contiene un instalador .exe",
        ));
    }
    let staged = work.join(
        payload
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    );
    fs::copy(payload, &staged)?;
    Ok(staged)
}

fn find_exe_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut preferred = Vec::new();
    let mut others = Vec::new();
    fn walk(dir: &Path, preferred: &mut Vec<PathBuf>, others: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, preferred, others);
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|e| e.eq_ignore_ascii_case("exe"))
                    .unwrap_or(false)
                {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if name.contains("setup") || name.contains("install") || name.contains("vbcable")
                    {
                        preferred.push(path);
                    } else {
                        others.push(path);
                    }
                }
            }
        }
    }
    walk(dir, &mut preferred, &mut others);
    preferred.into_iter().next().or_else(|| others.into_iter().next())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    // Use PowerShell Expand-Archive to avoid extra crate dependency.
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                zip_path.display(),
                dest.display()
            ),
        ])
        .status()
        .map_err(|e| AudioError::message(format!("No se pudo extraer el ZIP: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AudioError::message("Falló la extracción del paquete de audio"))
    }
}

fn run_payload_installer(exe: &Path, log: &mut Vec<String>) -> Result<i32> {
    // Try common silent switches. VB-Audio often uses -i -h for install/hide.
    let attempts: Vec<Vec<&str>> = vec![
        vec!["-i", "-h"],
        vec!["/S"],
        vec!["/silent"],
        vec!["/quiet"],
        vec![],
    ];

    for args in attempts {
        log.push(format!(
            "Ejecutando: {} {}",
            exe.display(),
            args.join(" ")
        ));
        // Elevate with ShellExecute runas via PowerShell Start-Process -Verb RunAs
        let arg_list = if args.is_empty() {
            String::new()
        } else {
            // Quote each arg
            args.iter()
                .map(|a| format!("'{}'", a.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(",")
        };
        let ps = if arg_list.is_empty() {
            format!(
                "$p = Start-Process -FilePath '{}' -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
                exe.display()
            )
        } else {
            format!(
                "$p = Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
                exe.display(),
                arg_list
            )
        };
        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps])
            .output()
            .map_err(|e| AudioError::message(format!("No se pudo iniciar el preparativo: {e}")))?;
        let code = output.status.code().unwrap_or(-1);
        log.push(format!("Resultado intento: {code}"));
        // 0 success; for VB-Cable, 0 is fine. If user cancels UAC, code may be non-zero.
        if code == 0 {
            return Ok(code);
        }
        // If empty-args GUI attempt, still return that code.
        if args.is_empty() {
            return Ok(code);
        }
    }
    Ok(1)
}
