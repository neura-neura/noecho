use crate::error::{AudioError, Result};
use crate::{KNOWN_VIRTUAL_DEVICE_HINTS, SHARED_DEVICE_FRIENDLY_NAME};
use serde::{Deserialize, Serialize};
use windows::core::GUID;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    eCommunications, eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default_multimedia: bool,
    pub is_default_communications: bool,
    pub is_virtual_shared_candidate: bool,
    pub is_physical_candidate: bool,
    pub state: u32,
}

pub struct DeviceService;

impl DeviceService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_render_devices(&self) -> Result<Vec<AudioDevice>> {
        let _com = crate::com::ComApartment::init_mta()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let default_mm = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .ok()
                .and_then(|d| device_id(&d).ok());
            let default_comm = enumerator
                .GetDefaultAudioEndpoint(eRender, eCommunications)
                .ok()
                .and_then(|d| device_id(&d).ok());

            let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let count = collection.GetCount()?;
            let mut devices = Vec::with_capacity(count as usize);

            for i in 0..count {
                let device = collection.Item(i)?;
                let id = device_id(&device)?;
                let name = device_friendly_name(&device).unwrap_or_else(|_| id.clone());
                let description = device_description(&device).ok();
                let is_virtual = is_virtual_candidate(&name, description.as_deref());
                devices.push(AudioDevice {
                    is_default_multimedia: default_mm.as_ref() == Some(&id),
                    is_default_communications: default_comm.as_ref() == Some(&id),
                    is_virtual_shared_candidate: is_virtual,
                    is_physical_candidate: !is_virtual,
                    id,
                    name,
                    description,
                    state: DEVICE_STATE_ACTIVE.0,
                });
            }

            devices.sort_by(|a, b| {
                b.is_physical_candidate
                    .cmp(&a.is_physical_candidate)
                    .then(a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            });
            Ok(devices)
        }
    }

    pub fn get_default_render(&self, role: DefaultRole) -> Result<AudioDevice> {
        let devices = self.list_render_devices()?;
        let wanted = match role {
            DefaultRole::Multimedia => devices.into_iter().find(|d| d.is_default_multimedia),
            DefaultRole::Communications => {
                devices.into_iter().find(|d| d.is_default_communications)
            }
        };
        wanted.ok_or_else(|| AudioError::DeviceNotFound("default render device".into()))
    }

    pub fn find_by_id(&self, id: &str) -> Result<AudioDevice> {
        self.list_render_devices()?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| AudioError::DeviceNotFound(id.into()))
    }

    pub fn find_shared_candidate(&self, preferred_id: Option<&str>) -> Result<Option<AudioDevice>> {
        let devices = self.list_render_devices()?;
        if let Some(id) = preferred_id {
            if let Some(d) = devices.iter().find(|d| d.id == id) {
                return Ok(Some(d.clone()));
            }
        }
        if let Some(d) = devices
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(SHARED_DEVICE_FRIENDLY_NAME))
        {
            return Ok(Some(d.clone()));
        }

        // Pick the best automatic shared device for non-technical users.
        // Prefer standard shared cables; avoid mic-chain cables A/B.
        let mut preferred: Vec<AudioDevice> = devices
            .iter()
            .filter(|d| d.is_virtual_shared_candidate && is_preferred_shared_virtual(&d.name))
            .cloned()
            .collect();
        preferred.sort_by_key(|d| shared_device_score(&d.name));
        if let Some(d) = preferred.into_iter().next() {
            return Ok(Some(d));
        }
        // Do not auto-select Cable A/B; they are usually part of mic FX chains.
        Ok(None)
    }

    pub fn choose_physical(
        &self,
        preferred_id: Option<&str>,
        fallback_to_default: bool,
    ) -> Result<AudioDevice> {
        let devices = self.list_render_devices()?;
        if let Some(id) = preferred_id {
            if let Some(d) = devices.iter().find(|d| d.id == id && d.is_physical_candidate) {
                return Ok(d.clone());
            }
        }
        if fallback_to_default {
            if let Some(d) = devices
                .iter()
                .find(|d| d.is_default_multimedia && d.is_physical_candidate)
            {
                return Ok(d.clone());
            }
        }
        devices
            .into_iter()
            .find(|d| d.is_physical_candidate)
            .ok_or_else(|| {
                AudioError::PhysicalDeviceUnavailable("no physical render device".into())
            })
    }
}

impl Default for DeviceService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DefaultRole {
    Multimedia,
    Communications,
}

fn shared_device_score(name: &str) -> i32 {
    let n = name.to_ascii_lowercase();
    // Lower is better.
    // Cable A/B are commonly used for mic FX chains (MicVST / Mic Mix).
    // Prefer the standard VB-Cable for shared system audio.
    if n.contains("audio compartido") || n.contains("noecho") {
        return 0;
    }
    let is_a = n.contains("cable-a") || n.contains("cable a");
    let is_b = n.contains("cable-b") || n.contains("cable b");
    if n.contains("steam streaming") {
        return 100;
    }
    if is_a || is_b {
        return if is_a { 80 } else { 90 };
    }
    if n.contains("cable") && n.contains("input") {
        return 1;
    }
    if n.contains("vb-audio") || n.contains("vb-cable") || n.contains("virtual cable") {
        return 2;
    }
    10
}

fn is_mic_chain_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("cable-a")
        || n.contains("cable a")
        || n.contains("cable-b")
        || n.contains("cable b")
}

fn is_preferred_shared_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if is_mic_chain_virtual(&n) {
        return false;
    }
    if n.contains("steam streaming") {
        return false;
    }
    n.contains("audio compartido")
        || n.contains("noecho")
        || (n.contains("cable") && n.contains("input"))
        || n.contains("vb-audio")
        || n.contains("vb-cable")
        || n.contains("virtual cable")
}

fn is_virtual_candidate(name: &str, description: Option<&str>) -> bool {

    let hay = format!("{} {}", name, description.unwrap_or("")).to_ascii_lowercase();
    if hay.contains("steam streaming") {
        return true;
    }
    KNOWN_VIRTUAL_DEVICE_HINTS
        .iter()
        .any(|hint| hay.contains(&hint.to_ascii_lowercase()))
}

unsafe fn device_id(device: &IMMDevice) -> Result<String> {
    let id = device.GetId()?;
    Ok(pwstr_to_string(id))
}

unsafe fn device_friendly_name(device: &IMMDevice) -> Result<String> {
    let key = windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    property_string(device, &key)
}

unsafe fn device_description(device: &IMMDevice) -> Result<String> {
    let key = windows::Win32::Devices::FunctionDiscovery::PKEY_Device_DeviceDesc;
    property_string(device, &key)
}

unsafe fn property_string(device: &IMMDevice, key: &PROPERTYKEY) -> Result<String> {
    let store: IPropertyStore = device.OpenPropertyStore(STGM_READ)?;
    let value = store.GetValue(key)?;
    propvariant_to_string(&value)
}

fn propvariant_to_string(value: &PROPVARIANT) -> Result<String> {
    use windows::Win32::System::Variant::{VT_BSTR, VT_EMPTY, VT_LPWSTR, VT_NULL};

    unsafe {
        let vt = value.Anonymous.Anonymous.vt;
        if vt == VT_EMPTY || vt == VT_NULL {
            return Err(AudioError::message("empty property"));
        }
        if vt == VT_LPWSTR {
            let pwstr = value.Anonymous.Anonymous.Anonymous.pwszVal;
            return Ok(pwstr_to_string(pwstr));
        }
        if vt == VT_BSTR {
            let bstr = &value.Anonymous.Anonymous.Anonymous.bstrVal;
            return Ok(bstr.to_string());
        }
    }
    Ok(format!("{value:?}"))
}

pub(crate) unsafe fn pwstr_to_string(p: windows::core::PWSTR) -> String {
    if p.is_null() {
        String::new()
    } else {
        p.to_string().unwrap_or_default()
    }
}

pub fn set_default_endpoint(device_id: &str, role: DefaultRole) -> Result<()> {
    crate::policy::set_default_audio_endpoint(device_id, role)
}

#[allow(dead_code)]
fn _guid_keep() -> GUID {
    GUID::zeroed()
}
