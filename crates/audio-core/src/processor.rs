//! Read UnifiedAudio's saved routing without modifying its effects or configuration.
use crate::{error::{AudioError, Result}, sessions::AudioSessionInfo};
use serde::{Serialize, Deserialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessorRoute {
    pub input_id: String,
    pub input_name: String,
    pub output_id: String,
    pub output_name: String,
    pub system_enabled: bool,
    pub process_filter_enabled: bool,
}
pub fn is_processor(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "unifiedaudio.exe" | "unifiedaudio engine host.exe")
}
pub fn independent_output(name: &str, processor_name: &str) -> bool {
    let family = |n: &str| {
        let n = n.to_ascii_lowercase();
        if n.contains("steam streaming") { return 0; }
        if n.contains("cable-a") || n.contains("cable a") { return 1; }
        if n.contains("cable-b") || n.contains("cable b") { return 2; }
        if n.contains("vb-audio virtual cable") { return 3; }
        4
    };
    let candidate = family(name);
    candidate != 0 && (candidate == 4 || candidate != family(processor_name))
}
pub fn detect(sessions: &[AudioSessionInfo]) -> Result<Option<ProcessorRoute>> {
    if !sessions.iter().any(|s| s.exe_name.as_deref().is_some_and(is_processor)) { return Ok(None); }
    read().map(Some)
}
pub fn read() -> Result<ProcessorRoute> {
    let path = dirs::config_dir().ok_or_else(|| AudioError::message("No hay AppData para UnifiedAudio."))?.join("UnifiedAudio/config.xml");
    let xml = std::fs::read_to_string(path).map_err(|_| AudioError::message("No se pudo leer la conexión de UnifiedAudio. Guarda su configuración y vuelve a intentar."))?;
    parse(&xml)
}
fn parse(xml: &str) -> Result<ProcessorRoute> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event().map_err(|e| AudioError::message(e.to_string()))? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"UnifiedAudio" => {
                let mut attrs = std::collections::HashMap::new();
                for a in e.attributes() {
                    let a = a.map_err(|e| AudioError::message(e.to_string()))?;
                    attrs.insert(String::from_utf8_lossy(a.key.as_ref()).into_owned(), a.unescape_value().map_err(|e| AudioError::message(e.to_string()))?.into_owned());
                }
                let get = |key: &str| attrs.get(key).cloned().unwrap_or_default();
                return Ok(ProcessorRoute { input_id: get("systemCaptureDeviceId"), input_name: get("systemCaptureDevice"), output_id: get("outputDeviceId"), output_name: get("outputDevice"), system_enabled: matches!(get("mixerMode").as_str(), "1" | "2"), process_filter_enabled: get("processFilterEnabled") == "1" });
            }
            Event::Eof => return Err(AudioError::message("La configuración de UnifiedAudio no es válida.")),
            _ => {}
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn routing_is_read_without_plugin_state() {
        let route = parse(r#"<UnifiedAudio systemCaptureDeviceId="in" systemCaptureDevice="Cable &amp; input" outputDeviceId="out" mixerMode="2" processFilterEnabled="0"><plugins/></UnifiedAudio>"#).unwrap();
        assert_eq!(route.input_name, "Cable & input");
        assert_eq!(route.output_id, "out");
        assert!(route.system_enabled);
        assert!(!route.process_filter_enabled);
        assert!(parse("<Wrong/>").is_err());
        assert!(!independent_output("CABLE In 16ch (VB-Audio Virtual Cable)", "CABLE Input (VB-Audio Virtual Cable)"));
        assert!(independent_output("CABLE-A Input (VB-Audio Cable A)", "CABLE Input (VB-Audio Virtual Cable)"));
        assert!(!independent_output("Steam Streaming Speakers", "CABLE Input"));
    }
}
