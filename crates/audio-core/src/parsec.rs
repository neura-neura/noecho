//! Configure only Parsec's capture route; never change Windows or app outputs.
use crate::{devices::{AudioDevice,DeviceService},error::{AudioError,Result}};
use serde_json::{json,Value};
use std::{path::PathBuf,fs};

fn config_path()->Result<PathBuf>{
    Ok(dirs::config_dir().ok_or_else(||AudioError::message("No hay AppData"))?.join("Parsec/config.json"))
}
pub fn installed()->bool{config_path().is_ok_and(|p|p.exists())}
pub fn remote_device()->Result<AudioDevice>{
    DeviceService::new().list_render_devices()?.into_iter()
        .find(|d|d.name.to_ascii_lowercase().contains("steam streaming speakers") && !d.is_default_multimedia && !d.is_default_communications)
        .ok_or_else(||AudioError::message("Falta una salida interna para Parsec. Esta integración requiere Steam Streaming Speakers; no usa Cable A/B ni cambia tu micrófono."))
}
fn settings(v:&Value)->Option<&serde_json::Map<String,Value>>{
    if let Some(a)=v.as_array(){return a.iter().find_map(settings);}
    v.as_object().filter(|o|o.contains_key("host_audio_cancel") || o.contains_key("host_audio_id"))
}
pub fn ready(id:&str)->bool{
    config_path().is_ok_and(|p|ready_at(&p,id))
}
fn ready_at(path:&std::path::Path,id:&str)->bool{
    fs::read(path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v|settings(&v).is_some_and(|s|s.get("host_audio_id").and_then(|v|v.get("value")).and_then(Value::as_str)==Some(id)
            && s.get("host_audio_cancel").and_then(|v|v.get("value")).and_then(Value::as_i64)==Some(0)))
}
fn apply(v:&mut Value,id:&str)->Result<()> {
    let owner=if let Some(a)=v.as_array_mut(){a.iter_mut().find(|v|v.is_object())}
        else{Some(v)}.ok_or_else(||AudioError::message("Formato de Parsec desconocido; se conserva su configuración."))?;
    let object=owner.as_object_mut().ok_or_else(||AudioError::message("Configuración de Parsec inválida"))?;
    for (key,value) in [("host_audio_id",json!(id)),("host_audio_cancel",json!(0))]{
        let entry=object.entry(key).or_insert_with(||json!({}));
        let entry=entry.as_object_mut().ok_or_else(||AudioError::message("Opción de Parsec incompatible"))?;
        entry.insert("value".into(),value);
    }
    Ok(())
}
pub fn prepare()->Result<()> {
    if !installed(){return Ok(());}
    let device=remote_device()?;
    let pending=dirs::data_local_dir().ok_or_else(||AudioError::message("No hay AppData"))?.join("NoEcho/parsec-pending");
    let path=config_path()?;
    if ready_at(&path,&device.id){return prepare_at(&path,&device.id,&pending,false);}
    // Parsec persists cached preferences on exit: update only while it is closed.
    let pids=crate::process::process_parent_map()?;
    let running=pids.keys().any(|pid|crate::process::process_image_path(*pid).as_deref().and_then(crate::process::file_name_from_path).is_some_and(|n|n.eq_ignore_ascii_case("parsecd.exe")));
    prepare_at(&path,&device.id,&pending,running)
}
fn prepare_at(path:&std::path::Path,id:&str,pending:&std::path::Path,running:bool)->Result<()>{
    if ready_at(path,id){if pending.exists(){fs::remove_file(pending)?;}return Ok(());}
    if running{
        if let Some(parent)=pending.parent(){fs::create_dir_all(parent)?;}
        fs::write(pending,"pending")?;
        return Ok(());
    }
    let bytes=fs::read(path)?;
    let mut value:Value=serde_json::from_slice(&bytes)?;
    apply(&mut value,id)?;
    let backup=path.with_extension("json.before-noecho");
    if !backup.exists(){fs::write(backup,&bytes)?;}
    let tmp=path.with_extension("json.noecho-tmp");
    fs::write(&tmp,serde_json::to_vec_pretty(&value)?)?;
    use windows::{core::PCWSTR,Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH}};
    let src:Vec<u16>=tmp.to_string_lossy().encode_utf16().chain(Some(0)).collect();
    let dst:Vec<u16>=path.to_string_lossy().encode_utf16().chain(Some(0)).collect();
    unsafe{MoveFileExW(PCWSTR(src.as_ptr()),PCWSTR(dst.as_ptr()),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)?;}
    if pending.exists(){fs::remove_file(pending)?;}
    Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn defers_running_unprepared_parsec_but_skips_prepared_running_parsec(){
        let dir=std::env::temp_dir().join(format!("NoEchoParsecPrepareTest{}",std::process::id()));fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");let pending=dir.join("pending");
        let original=serde_json::to_vec(&json!(["schema",{"host_output":{"value":"screen"},"host_audio_cancel":{"value":2}}])).unwrap();
        fs::write(&path,&original).unwrap();
        prepare_at(&path,"remote",&pending,true).unwrap();assert!(pending.exists());assert_eq!(fs::read(&path).unwrap(),original);assert!(!path.with_extension("json.before-noecho").exists());
        prepare_at(&path,"remote",&pending,false).unwrap();assert!(ready_at(&path,"remote"));assert!(!pending.exists());assert_eq!(fs::read(path.with_extension("json.before-noecho")).unwrap(),original);
        let prepared=fs::read(&path).unwrap();prepare_at(&path,"remote",&pending,true).unwrap();assert_eq!(fs::read(&path).unwrap(),prepared);assert!(!pending.exists());
        for file in [&path,&path.with_extension("json.before-noecho")]{fs::remove_file(file).unwrap();}fs::remove_dir(dir).unwrap();
    }
    #[test] fn changes_capture_only_and_preserves_metadata_and_preferences(){
        let mut v=json!(["schema",{"host_output":{"value":"monitor-id"},"host_audio_cancel":{"value":2,"saved":true},"echo_app_selection":{"value":"Discord.exe"},"other":{"value":17}}]);
        let original=v.clone();apply(&mut v,"internal").unwrap();
        assert_eq!(v[1]["host_audio_id"]["value"],"internal");assert_eq!(v[1]["host_audio_cancel"]["value"],0);
        assert_eq!(v[1]["host_audio_cancel"]["saved"],true);
        for key in ["host_output","echo_app_selection","other"]{assert_eq!(v[1][key],original[1][key]);}
        assert!(apply(&mut json!(["unknown"]),"x").is_err());
    }
}
