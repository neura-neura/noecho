use anyhow::{bail, Context, Result};
use audio_core::devices::{DefaultRole, DeviceService};
use audio_core::grouping::group_sessions;
use audio_core::loopback::{probe_default_loopback_energy, process_loopback_supported};
use audio_core::policy::set_default_audio_endpoint;
use audio_core::policy::set_process_default_endpoint;
use audio_core::process::{expand_related_pids, process_image_path, process_parent_map};
use audio_core::sessions::SessionService;
use audio_core::types::AppIdentity;
use audio_core::ProtectionEngine;
use clap::{Parser, Subcommand};
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(name = "tech-probe")]
#[command(about = "Prueba tecnica de NoEcho (Fase 1)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Devices,
    Sessions {
        #[arg(long)]
        json: bool,
    },
    Groups {
        #[arg(long)]
        json: bool,
    },
    Tree {
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long)]
        exe: Option<String>,
    },
    Probe {
        #[arg(long, default_value_t = 1.0)]
        seconds: f32,
    },
    SetDefault {
        device_id: String,
    },
    Activate {
        #[arg(long)]
        exe: Vec<String>,
    },
    Deactivate,
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Verifica el enrutamiento por proceso sin tocar una aplicación de audio.
    PolicyProbe {
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long)]
        device_id: Option<String>,
    },
    Smoke,
    Setup,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Devices => cmd_devices()?,
        Commands::Sessions { json } => cmd_sessions(json)?,
        Commands::Groups { json } => cmd_groups(json)?,
        Commands::Tree { pid, exe } => cmd_tree(pid, exe)?,
        Commands::Probe { seconds } => cmd_probe(seconds)?,
        Commands::SetDefault { device_id } => cmd_set_default(&device_id)?,
        Commands::Activate { exe } => cmd_activate(exe)?,
        Commands::Deactivate => cmd_deactivate()?,
        Commands::Status { json } => cmd_status(json)?,
        Commands::PolicyProbe { pid, device_id } => cmd_policy_probe(pid, device_id)?,
        Commands::Smoke => cmd_smoke()?,
        Commands::Setup => cmd_setup()?,
    }
    Ok(())
}

fn cmd_devices() -> Result<()> {
    let devices = DeviceService::new().list_render_devices()?;
    println!("Dispositivos de salida activos: {}\n", devices.len());
    for d in devices {
        let marks = format!(
            "{}{}{}{}",
            if d.is_default_multimedia { " [default-mm]" } else { "" },
            if d.is_default_communications { " [default-comm]" } else { "" },
            if d.is_virtual_shared_candidate { " [virtual]" } else { "" },
            if d.is_physical_candidate { " [physical]" } else { "" },
        );
        println!("- {}{marks}", d.name);
        println!("  id: {}", d.id);
        if let Some(desc) = d.description {
            println!("  desc: {desc}");
        }
        println!();
    }
    Ok(())
}

fn cmd_sessions(json: bool) -> Result<()> {
    let sessions = SessionService::new().list_sessions()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }
    println!("Sesiones de audio: {}\n", sessions.len());
    for s in sessions {
        println!(
            "- {} | pid={} | {:?} | vol={:.0}%{}",
            s.display_name,
            s.pid,
            s.state,
            s.volume * 100.0,
            if s.muted { " muted" } else { "" }
        );
        if let Some(exe) = &s.exe_name {
            println!("  exe: {exe}");
        }
        if let Some(path) = &s.exe_path {
            println!("  path: {path}");
        }
        if let Some(dev) = &s.device_name {
            println!("  device: {dev}");
        }
        println!();
    }
    Ok(())
}

fn cmd_groups(json: bool) -> Result<()> {
    let sessions = SessionService::new().list_sessions()?;
    let groups = group_sessions(&sessions, &[]);
    if json {
        println!("{}", serde_json::to_string_pretty(&groups)?);
        return Ok(());
    }
    println!("Aplicaciones con audio: {}\n", groups.len());
    for g in groups {
        println!(
            "- {} ({}) | {} | {} sesion(es) | pids={:?}",
            g.display_name,
            g.exe_name,
            g.state_label(),
            g.session_count,
            g.pids
        );
        if let Some(path) = g.exe_path {
            println!("  {path}");
        }
        println!();
    }
    Ok(())
}

fn cmd_tree(pid: Option<u32>, exe: Option<String>) -> Result<()> {
    let seed = if let Some(pid) = pid {
        vec![pid]
    } else if let Some(exe) = &exe {
        SessionService::new()
            .list_sessions()?
            .into_iter()
            .filter(|s| {
                s.exe_name
                    .as_ref()
                    .map(|n| n.eq_ignore_ascii_case(exe))
                    .unwrap_or(false)
            })
            .map(|s| s.pid)
            .collect()
    } else {
        bail!("indica --pid o --exe");
    };
    if seed.is_empty() {
        bail!("no se encontraron PIDs semilla");
    }
    let related = expand_related_pids(&seed, exe.as_deref())?;
    let parents = process_parent_map()?;
    println!("Procesos relacionados: {}\n", related.len());
    for pid in related {
        let path = process_image_path(pid).unwrap_or_else(|| "<desconocido>".into());
        let parent = parents.get(&pid).copied().unwrap_or(0);
        println!("- pid={pid} parent={parent}");
        println!("  {path}");
    }
    Ok(())
}

fn cmd_probe(seconds: f32) -> Result<()> {
    println!("Soporte process-loopback: {}", process_loopback_supported());
    let result = probe_default_loopback_energy(seconds)?;
    println!("Metodo: {}", result.method);
    println!("Segundos: {:.2}", result.seconds);
    println!("Energia media: {:.6}", result.average_energy);
    println!("Pico: {:.6}", result.peak_energy);
    println!("Muestras: {}", result.frames);
    for n in result.notes {
        println!("nota: {n}");
    }
    Ok(())
}

fn cmd_set_default(device_id: &str) -> Result<()> {
    set_default_audio_endpoint(device_id, DefaultRole::Multimedia).context("SetDefault")?;
    println!("Dispositivo predeterminado actualizado.");
    Ok(())
}

fn cmd_activate(exes: Vec<String>) -> Result<()> {
    if exes.is_empty() {
        bail!("indica al menos un --exe");
    }
    let engine = ProtectionEngine::initialize()?;
    let apps = exes
        .into_iter()
        .map(|e| {
            let mut id = AppIdentity::from_exe(e);
            if let Ok(sessions) = engine.list_sessions() {
                if let Some(s) = sessions.into_iter().find(|s| {
                    s.exe_name
                        .as_ref()
                        .map(|n| n.eq_ignore_ascii_case(&id.exe_name))
                        .unwrap_or(false)
                }) {
                    id.exe_path = s.exe_path;
                    id.display_name = Some(s.display_name);
                }
            }
            id
        })
        .collect();
    let status = engine.activate(Some(apps))?;
    println!("active={}", status.active);
    println!("{}", status.message);
    for w in status.warnings {
        println!("aviso: {w}");
    }
    println!("Presiona Enter para restaurar y salir...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    let _ = engine.deactivate();
    Ok(())
}

fn cmd_deactivate() -> Result<()> {
    let engine = ProtectionEngine::initialize()?;
    let status = engine.deactivate()?;
    println!("active={}", status.active);
    println!("{}", status.message);
    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let engine = ProtectionEngine::initialize()?;
    let status = engine.status();
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("active: {}", status.active);
        println!("message: {}", status.message);
        println!("shared available: {}", status.shared_device_available);
        println!("excluded: {}", status.excluded_count);
        for app in status.excluded_apps {
            println!(" - {}", app.exe_name);
        }
        for w in status.warnings {
            println!("warning: {w}");
        }
    }
    Ok(())
}

fn cmd_policy_probe(pid: Option<u32>, device_id: Option<String>) -> Result<()> {
    let pid = pid.unwrap_or(std::process::id());
    let device_id = match device_id {
        Some(id) => id,
        None => DeviceService::new()
            .get_default_render(DefaultRole::Multimedia)?
            .id,
    };
    set_process_default_endpoint(pid, &device_id)
        .context("enrutamiento seguro por proceso")?;
    println!("ok: pid {pid} acepta la ruta de audio solicitada");
    Ok(())
}

fn cmd_smoke() -> Result<()> {
    println!("== NoEcho tech-probe smoke ==");
    let devices = DeviceService::new().list_render_devices()?;
    println!("[ok] devices: {}", devices.len());
    if devices.is_empty() {
        bail!("no hay dispositivos de audio");
    }
    for d in &devices {
        println!(
            "  - {}{}",
            d.name,
            if d.is_default_multimedia { " *" } else { "" }
        );
    }

    let sessions = SessionService::new().list_sessions()?;
    println!("[ok] sessions: {}", sessions.len());
    let groups = group_sessions(&sessions, &[]);
    println!("[ok] groups: {}", groups.len());
    for g in groups.iter().take(12) {
        println!(
            "  - {} ({}) [{}] sessions={} pids={:?}",
            g.display_name,
            g.exe_name,
            g.state_label(),
            g.session_count,
            g.pids
        );
    }

    let parents = process_parent_map()?;
    println!("[ok] process snapshot: {} processes", parents.len());
    println!(
        "[ok] process loopback supported: {}",
        process_loopback_supported()
    );

    match probe_default_loopback_energy(0.5) {
        Ok(p) => println!(
            "[ok] loopback probe avg={:.6} peak={:.6} samples={}",
            p.average_energy, p.peak_energy, p.frames
        ),
        Err(e) => println!("[warn] loopback probe failed: {e}"),
    }

    let engine = ProtectionEngine::initialize()?;
    let status = engine.status();
    println!(
        "[ok] engine status active={} shared_available={}",
        status.active, status.shared_device_available
    );
    if !status.shared_device_available {
        println!("[info] No hay dispositivo virtual Audio compartido. Ver docs/driver.md");
    }
    println!("\nFase 1 basica completada.");
    let _ = Write::flush(&mut io::stdout());
    Ok(())
}


fn cmd_setup() -> Result<()> {
    let status = audio_core::SetupService::status();
    println!("ready={}", status.ready);
    println!("state={:?}", status.state);
    println!("{}", status.title);
    println!("{}", status.message);
    if let Some(d) = status.detail {
        println!("detail: {d}");
    }
    println!("can_prepare={}", status.can_prepare_automatically);
    Ok(())
}
