//! Authenticated loopback API. Remote operators use SSH/PowerShell remoting.
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
pub fn start(engine: SharedEngine) -> Result<(), String> {
    let server = Server::http("127.0.0.1:47832").map_err(|e| format!("API: {e}"))?;
    let dir = dirs::data_local_dir()
        .ok_or("No hay AppData")?
        .join("NoEcho");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    std::fs::write(dir.join("api-token"), &token).map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name("noecho-control".into())
        .spawn(move || {
            for mut req in server.incoming_requests() {
                let authorized = req.headers().iter().any(|h| {
                    h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}")
                });
                if !authorized {
                    reply(req, 401, json!({"error":"Unauthorized"}));
                    continue;
                }
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
