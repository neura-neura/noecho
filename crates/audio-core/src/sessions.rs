use crate::error::{AudioError, Result};
use crate::icons::icon_data_url_for_path;
use crate::process::{file_name_from_path, process_image_path};
use crate::types::PlaybackState;
use serde::{Deserialize, Serialize};
use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, AudioSessionStateActive, AudioSessionStateExpired,
    AudioSessionStateInactive, IAudioSessionControl, IAudioSessionControl2, IAudioSessionEnumerator,
    IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, ISimpleAudioVolume,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CLSCTX_INPROC_SERVER, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSessionInfo {
    pub session_id: String,
    pub pid: u32,
    pub display_name: String,
    pub exe_path: Option<String>,
    pub exe_name: Option<String>,
    pub icon_path: Option<String>,
    pub icon_data_url: Option<String>,
    pub state: PlaybackState,
    pub is_system_sounds: bool,
    pub volume: f32,
    pub muted: bool,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
}

pub struct SessionService;

impl SessionService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_sessions(&self) -> Result<Vec<AudioSessionInfo>> {
        let _com = crate::com::ComApartment::init_mta()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let collection = enumerator.EnumAudioEndpoints(
                eRender,
                windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE,
            )?;
            let count = collection.GetCount()?;
            let mut sessions = Vec::new();

            for i in 0..count {
                let device = collection.Item(i)?;
                let device_id = get_device_id(&device).ok();
                let device_name = get_device_name(&device).ok();
                if let Ok(mut device_sessions) =
                    enumerate_device_sessions(&device, device_id.clone(), device_name.clone())
                {
                    sessions.append(&mut device_sessions);
                }
            }

            if sessions.is_empty() {
                if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    let device_id = get_device_id(&device).ok();
                    let device_name = get_device_name(&device).ok();
                    if let Ok(mut device_sessions) =
                        enumerate_device_sessions(&device, device_id, device_name)
                    {
                        sessions.append(&mut device_sessions);
                    }
                }
            }

            sessions.sort_by(|a, b| {
                b.state_rank()
                    .cmp(&a.state_rank())
                    .then(
                        a.display_name
                            .to_ascii_lowercase()
                            .cmp(&b.display_name.to_ascii_lowercase()),
                    )
            });
            Ok(sessions)
        }
    }

    /// Change the mute state of one concrete render-session instance.
    /// Returns true when the session still existed and was updated.
    pub fn set_session_muted(&self, session_id: &str, muted: bool) -> Result<bool> {
        let _com = crate::com::ComApartment::init_mta()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let collection = enumerator.EnumAudioEndpoints(
                eRender,
                windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE,
            )?;

            for i in 0..collection.GetCount()? {
                let device = collection.Item(i)?;
                let manager: IAudioSessionManager2 =
                    match device.Activate(CLSCTX_INPROC_SERVER, None) {
                        Ok(manager) => manager,
                        Err(_) => continue,
                    };
                let sessions = match manager.GetSessionEnumerator() {
                    Ok(sessions) => sessions,
                    Err(_) => continue,
                };
                for index in 0..sessions.GetCount()? {
                    let control: IAudioSessionControl = match sessions.GetSession(index) {
                        Ok(control) => control,
                        Err(_) => continue,
                    };
                    let control2: IAudioSessionControl2 = match control.cast() {
                        Ok(control) => control,
                        Err(_) => continue,
                    };
                    let instance_id = control2
                        .GetSessionInstanceIdentifier()
                        .ok()
                        .and_then(|value| value.to_string().ok());
                    if instance_id.as_deref() != Some(session_id) {
                        continue;
                    }
                    let volume: ISimpleAudioVolume = control.cast()?;
                    volume.SetMute(muted, std::ptr::null())?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl Default for SessionService {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSessionInfo {
    fn state_rank(&self) -> u8 {
        match self.state {
            PlaybackState::Active => 3,
            PlaybackState::Inactive => 2,
            PlaybackState::Expired => 1,
            PlaybackState::Unknown => 0,
        }
    }
}

unsafe fn enumerate_device_sessions(
    device: &IMMDevice,
    device_id: Option<String>,
    device_name: Option<String>,
) -> Result<Vec<AudioSessionInfo>> {
    let manager: IAudioSessionManager2 = device.Activate(CLSCTX_INPROC_SERVER, None)?;
    let enumerator: IAudioSessionEnumerator = manager.GetSessionEnumerator()?;
    let count = enumerator.GetCount()?;
    let mut out = Vec::with_capacity(count as usize);

    for i in 0..count {
        let control: IAudioSessionControl = match enumerator.GetSession(i) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let control2: IAudioSessionControl2 = match control.cast() {
            Ok(c) => c,
            Err(_) => continue,
        };

        let pid = control2.GetProcessId().unwrap_or(0);
        let state = match control.GetState() {
            Ok(AudioSessionStateActive) => PlaybackState::Active,
            Ok(AudioSessionStateInactive) => PlaybackState::Inactive,
            Ok(AudioSessionStateExpired) => PlaybackState::Expired,
            _ => PlaybackState::Unknown,
        };

        let display_raw = control
            .GetDisplayName()
            .ok()
            .and_then(|p| unsafe { p.to_string().ok() })
            .unwrap_or_default();

        let icon_path = control
            .GetIconPath()
            .ok()
            .and_then(|p| unsafe { p.to_string().ok() })
            .filter(|s| !s.is_empty());

        let session_identifier = control2
            .GetSessionIdentifier()
            .ok()
            .and_then(|p| unsafe { p.to_string().ok() })
            .unwrap_or_else(|| format!("pid-{pid}-{i}"));

        let session_instance = control2
            .GetSessionInstanceIdentifier()
            .ok()
            .and_then(|p| unsafe { p.to_string().ok() })
            .unwrap_or_else(|| session_identifier.clone());

        // IsSystemSoundsSession returns HRESULT directly: S_OK=0 system, S_FALSE=1 not.
        let is_system = control2.IsSystemSoundsSession().is_ok() && pid == 0;

        let (volume, muted) = session_volume(&control);

        let exe_path = if pid > 0 {
            process_image_path(pid)
        } else {
            None
        };
        let exe_name = exe_path.as_deref().and_then(file_name_from_path);

        let display_name = if !display_raw.is_empty() && !display_raw.starts_with('@') {
            display_raw
        } else if let Some(name) = &exe_name {
            pretty_app_name(name)
        } else if is_system {
            "Sonidos de Windows".into()
        } else {
            format!("Proceso {pid}")
        };

        let icon_data_url = exe_path
            .as_deref()
            .and_then(icon_data_url_for_path)
            .or_else(|| icon_path.as_deref().and_then(icon_data_url_for_path));

        out.push(AudioSessionInfo {
            session_id: session_instance,
            pid,
            display_name,
            exe_path,
            exe_name,
            icon_path,
            icon_data_url,
            state,
            is_system_sounds: is_system || pid == 0,
            volume,
            muted,
            device_id: device_id.clone(),
            device_name: device_name.clone(),
        });
    }

    Ok(out)
}

unsafe fn session_volume(control: &IAudioSessionControl) -> (f32, bool) {
    if let Ok(vol) = control.cast::<ISimpleAudioVolume>() {
        let level = vol.GetMasterVolume().unwrap_or(1.0);
        let muted = vol
            .GetMute()
            .ok()
            .map(|b| b.as_bool())
            .unwrap_or(false);
        (level, muted)
    } else {
        (1.0, false)
    }
}

unsafe fn get_device_id(device: &IMMDevice) -> Result<String> {
    let id = device.GetId()?;
    Ok(crate::devices::pwstr_to_string(id))
}

unsafe fn get_device_name(device: &IMMDevice) -> Result<String> {
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::{VT_BSTR, VT_LPWSTR};

    let key = windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    let store: IPropertyStore = device.OpenPropertyStore(STGM_READ)?;
    let value: PROPVARIANT = store.GetValue(&key)?;
    let vt = value.Anonymous.Anonymous.vt;
    if vt == VT_LPWSTR {
        let pw = value.Anonymous.Anonymous.Anonymous.pwszVal;
        Ok(crate::devices::pwstr_to_string(pw))
    } else if vt == VT_BSTR {
        let b = &value.Anonymous.Anonymous.Anonymous.bstrVal;
        Ok(b.to_string())
    } else {
        Err(AudioError::message("device name unavailable"))
    }
}

fn pretty_app_name(exe_name: &str) -> String {
    let stem = exe_name
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE");
    let mut chars = stem.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => stem.to_string(),
    }
}
