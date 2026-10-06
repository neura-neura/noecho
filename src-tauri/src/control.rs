//! Local and LAN control API. The installer scopes the firewall rule to LocalSubnet.
use audio_core::protection::SharedEngine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use tiny_http::{Header, Response, Server, StatusCode};
#[derive(Deserialize)]
pub struct Command {
    pub command: String,
    #[serde(default)]
    pub apps: Vec<String>,
    pub value: Option<Value>,
}
pub fn execute(engine: &SharedEngine, c: Command) -> Result<Value, String> {
    let result = match c.command.as_str() {
        "start" => {
            if !c.apps.is_empty() {
                engine
                    .set_excluded_apps(identities(c.apps))
                    .map_err(|e| e.to_string())?;
            }
            engine.activate(None).map(|s| json!(s))
        }
        "stop" => engine.deactivate().map(|s| json!(s)),
        "exclude" | "include" | "set-exclusions" => {
            let mut apps = if c.command == "set-exclusions" {
                vec![]
            } else {
                engine.config().excluded_apps
            };
            for a in identities(c.apps) {
                apps.retain(|v| !v.exe_name.eq_ignore_ascii_case(&a.exe_name));
                if c.command != "include" {
                    apps.push(a);
                }
            }
            engine
                .set_excluded_apps(apps)
                .map(|_| json!(engine.status()))
        }
        "monitor" | "voice" | "channel" => {
            let mut config = engine.config();
            let v = c.value.ok_or("Falta value")?;
            match c.command.as_str() {
                "monitor" => config.monitor = v.as_str().ok_or("value debe ser texto")?.into(),
                "voice" => {
                    config.microphone_to_remote = v.as_bool().ok_or("value debe ser booleano")?
                }
                _ => {
                    config.preferred_shared_device_id = if v.is_null() {
                        None
                    } else {
                        Some(
                            v.as_str()
                                .ok_or("value debe ser un ID de canal o null")?
                                .into(),
                        )
                    }
                }
            }
            engine.update_config(config).map(|_| json!(engine.config()))
        }
        "status" => return Ok(json!(engine.status())),
        "apps" => {
            return engine
                .list_app_groups()
                .map(|g| json!(g))
                .map_err(|e| e.to_string())
        }
        "devices" => {
            return engine
                .list_devices()
                .map(|d| json!(d))
                .map_err(|e| e.to_string())
        }
        "config" => return Ok(json!(engine.config())),
        "meters" => return Ok(json!(engine.telemetry())),
        _ => return Err("Comando desconocido".into()),
    };
    result.map_err(|e| e.to_string())
}
fn identities(names: Vec<String>) -> Vec<audio_core::AppIdentity> {
    names
        .into_iter()
        .map(|name| audio_core::AppIdentity {
            exe_name: name.to_ascii_lowercase(),
            exe_path: None,
            display_name: None,
        })
        .collect()
}

fn control_dir()->Result<std::path::PathBuf,String>{
    dirs::data_local_dir().map(|p|p.join("NoEcho")).ok_or("No hay AppData".into())
}
fn powershell(script:&str)->Result<String,String>{
    use std::os::windows::process::CommandExt;
    let output=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).creation_flags(0x08000000).output().map_err(|e|e.to_string())?;
    if !output.status.success(){return Err(String::from_utf8_lossy(&output.stderr).trim().into());}
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
fn network_ips()->Result<Vec<String>,String>{
    let result=powershell("$ErrorActionPreference='Stop'; Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { Get-NetIPAddress -InterfaceIndex $_.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue } | Where-Object { $_.AddressState -eq 'Preferred' } | Select-Object -ExpandProperty IPAddress")?;
    let ips:Vec<_>=result.lines().filter_map(|s|s.trim().parse::<std::net::Ipv4Addr>().ok()).filter(|ip|!ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local()).map(|ip|ip.to_string()).collect();
    if ips.is_empty(){return Err("Conecta el equipo a la red y vuelve a intentarlo.".into());}Ok(ips)
}
#[tauri::command]
pub fn get_control_info()->Result<Value,String>{
    Ok(json!({"urls":network_ips().unwrap_or_default().iter().map(|ip|format!("http://{ip}:47832")).collect::<Vec<_>>()}))
}
pub fn setup_network()->Result<(),String>{
    let dir=control_dir()?;std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let script_path=dir.join("enable-remote.ps1");
    std::fs::write(&script_path,include_str!("../../scripts/Enable-NoEchoRemote.ps1")).map_err(|e|e.to_string())?;
    let path=script_path.to_string_lossy().replace('\'',"''");
    let exe=std::env::current_exe().map_err(|e|e.to_string())?.to_string_lossy().replace('\'',"''");
    powershell(&format!("$ErrorActionPreference='Stop'; $p=Start-Process powershell.exe -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{path}\" -ProgramPath \"{exe}\"' -Wait -PassThru; if ($p.ExitCode -ne 0) {{ throw 'No se pudo permitir NoEcho en el firewall.' }}"))?;Ok(())
}
pub fn start(engine: SharedEngine) -> Result<(), String> {
    let server = Server::http("0.0.0.0:47832").map_err(|e| format!("API: {e}"))?;
    let dir = dirs::data_local_dir()
        .ok_or("No hay AppData")?
        .join("NoEcho");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    serve(server,engine)?;
    Ok(())
}
fn serve(server:Server,engine:SharedEngine)->Result<(),String>{
    std::thread::Builder::new()
        .name("noecho-control".into())
        .spawn(move || {
            for mut req in server.incoming_requests() {
                if req.headers().iter().any(|h| h.field.equiv("Origin")) {
                    reply(req, 403, json!({"error":"Browser origins are not allowed"}));
                    continue;
                }
                let result = match (req.method().as_str(), req.url()) {
                    ("GET", "/v1/status") => Ok(json!(engine.status())),
                    ("GET", "/v1/apps") => engine
                        .list_app_groups()
                        .map(|g| json!(g))
                        .map_err(|e| e.to_string()),
                    ("GET", "/v1/devices") => engine
                        .list_devices()
                        .map(|d| json!(d))
                        .map_err(|e| e.to_string()),
                    ("GET", "/v1/config") => Ok(json!(engine.config())),
                    ("GET", "/v1/meters") => Ok(json!(engine.telemetry())),
                    ("POST", "/v1/command") => {
                        let mut body = String::new();
                        match req.as_reader().take(65537).read_to_string(&mut body) {
                            Ok(_) if body.len() <= 65536 => serde_json::from_str::<Command>(&body)
                                .map_err(|e| e.to_string())
                                .and_then(|c| execute(&engine, c)),
                            _ => Err("Invalid or oversized request".into()),
                        }
                    }
                    _ => {
                        reply(req, 404, json!({"error":"Not found"}));
                        continue;
                    }
                };
                match result {
                    Ok(v) => reply(req, 200, json!({"ok":true,"data":v})),
                    Err(e) => reply(req, 400, json!({"ok":false,"error":e})),
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn reply(req: tiny_http::Request, code: u16, body: Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(code))
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
    let _ = req.respond(response);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_requires_name_and_preserves_boolean() {
        assert!(serde_json::from_str::<Command>("{}").is_err());
        let c: Command = serde_json::from_str(r#"{"command":"voice","value":false}"#).unwrap();
        assert_eq!(c.value, Some(json!(false)));
    }
}
