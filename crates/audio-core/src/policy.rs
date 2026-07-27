use crate::devices::DefaultRole;
use crate::error::{AudioError, Result};
use std::ffi::c_void;
use windows::core::{HSTRING, IInspectable, Interface, GUID, HRESULT, IUnknown};
use windows::Win32::Media::Audio::{eCommunications, eConsole, eMultimedia, eRender, ERole};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::WinRT::RoGetActivationFactory;

pub fn set_default_audio_endpoint(device_id: &str, role: DefaultRole) -> Result<()> {
    let _com = crate::com::ComApartment::init_mta()?;
    let e_role = match role {
        DefaultRole::Multimedia => eMultimedia,
        DefaultRole::Communications => eCommunications,
    };

    if let Err(e) = set_default_via_policy_config(device_id, e_role) {
        tracing::warn!("policy config failed: {e}");
        return Err(e);
    }
    if matches!(role, DefaultRole::Multimedia) {
        let _ = set_default_via_policy_config(device_id, eConsole);
    }
    Ok(())
}

fn set_default_via_policy_config(device_id: &str, role: ERole) -> Result<()> {
    const CLSID_POLICY_CONFIG_CLIENT: GUID =
        GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IIDS: [GUID; 3] = [
        GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8),
        GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620),
        GUID::from_u128(0xca286fc3_91fd_42c3_8e9b_caafa66242e3),
    ];

    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let unknown: IUnknown = CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)
            .map_err(|e| AudioError::message(format!("No se pudo crear PolicyConfigClient: {e}")))?;

        // IPolicyConfig/ IPolicyConfigVista have the same method order here:
        // SetDefaultEndpoint is vtable slot 13 (3 IUnknown + 10 interface methods).
        // Do not probe arbitrary slots: calling the wrong COM method can corrupt
        // another process's audio session or terminate it.
        for iid in IIDS {
            if let Ok(ptr) = query_interface_raw(&unknown, &iid) {
                let result = call_set_default_endpoint(ptr, wide.as_ptr(), role);
                release_raw(ptr);
                if result.is_ok() {
                    return Ok(());
                }
            }
        }
    }

    Err(AudioError::message(
        "No se pudo cambiar el dispositivo predeterminado mediante PolicyConfig. En desarrollo puedes seleccionar manualmente Audio compartido como salida predeterminada.",
    ))
}

unsafe fn query_interface_raw<T: Interface>(unknown: &T, iid: &GUID) -> Result<*mut c_void> {
    let mut out: *mut c_void = std::ptr::null_mut();
    let hr = Interface::query(unknown, iid, &mut out);
    if hr.is_ok() && !out.is_null() {
        Ok(out)
    } else {
        Err(AudioError::from_hresult(hr, "QueryInterface PolicyConfig"))
    }
}

unsafe fn release_raw(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vtable = *(ptr as *const *const usize);
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        std::mem::transmute(*vtable.add(2));
    let _ = release(ptr);
}

unsafe fn call_set_default_endpoint(
    this: *mut c_void,
    device_id: *const u16,
    role: ERole,
) -> Result<()> {
    if this.is_null() {
        return Err(AudioError::message("null policy pointer"));
    }
    let vtable = *(this as *const *const usize);
    let func: unsafe extern "system" fn(*mut c_void, *const u16, ERole) -> HRESULT =
        // IPolicyConfig::SetDefaultEndpoint is slot 13.
        std::mem::transmute(*vtable.add(13));
    let hr = func(this, device_id, role);
    if hr.is_ok() {
        Ok(())
    } else {
        Err(AudioError::from_hresult(hr, "SetDefaultEndpoint"))
    }
}

pub fn set_app_default_endpoint(process_path: &str, device_id: &str) -> Result<()> {
    let pids = pids_for_process_path(process_path);
    if pids.is_empty() {
        return Err(AudioError::message(format!(
            "no se encontraron procesos activos para {process_path}"
        )));
    }

    let mut last_error = None;
    for pid in pids {
        if let Err(error) = set_process_default_endpoint(pid, device_id) {
            last_error = Some(error);
        }
    }
    last_error.map_or(Ok(()), Err)
}

pub fn clear_app_default_endpoint(process_path: &str) -> Result<()> {
    let pids = pids_for_process_path(process_path);
    if pids.is_empty() {
        return Ok(());
    }

    let mut last_error = None;
    for pid in pids {
        if let Err(error) = set_process_default_endpoint(pid, "") {
            last_error = Some(error);
        }
    }
    last_error.map_or(Ok(()), Err)
}

/// Safely assign an active process to a render endpoint using the Windows
/// AudioPolicyConfig activation factory used by modern Windows Sound settings.
///
/// This is an undocumented API, but unlike the previous implementation it uses
/// the complete interface layout and one fixed, verified method slot. It never
/// guesses vtable offsets.
pub fn set_process_default_endpoint(pid: u32, device_id: &str) -> Result<()> {
    set_process_default_endpoints(pid, device_id, device_id)
}

/// Assign Console/Multimedia and Communications independently. This preserves
/// the common Windows setup where general audio uses speakers while calls use
/// a headset.
pub fn set_process_default_endpoints(
    pid: u32,
    multimedia_device_id: &str,
    communications_device_id: &str,
) -> Result<()> {
    let _com = crate::com::ComApartment::init_mta()?;
    if pid == 0 {
        return Err(AudioError::message("no se puede enrutar el proceso 0"));
    }

    const CLASS_NAME: &str = "Windows.Media.Internal.AudioPolicyConfig";
    const IID_21H2: GUID =
        GUID::from_u128(0xab3d4648_e242_459f_b02f_541c70306324);
    const IID_DOWNLEVEL: GUID =
        GUID::from_u128(0x2a59116d_6c4f_45e0_a74f_707e3fef9258);

    // IInspectable contributes 6 entries (IUnknown + IInspectable). The
    // interface declares 19 methods before SetPersistedDefaultAudioEndpoint.
    const SET_PERSISTED_DEFAULT_ENDPOINT_SLOT: usize = 25;

    let class_name = HSTRING::from(CLASS_NAME);
    let full_multimedia_device_id = if multimedia_device_id.is_empty() {
        String::new()
    } else {
        full_render_device_id(multimedia_device_id)
    };
    let full_communications_device_id = if communications_device_id.is_empty() {
        String::new()
    } else {
        full_render_device_id(communications_device_id)
    };
    let multimedia_device_name = HSTRING::from(full_multimedia_device_id.as_str());
    let communications_device_name = HSTRING::from(full_communications_device_id.as_str());

    unsafe {
        let factory: IInspectable = RoGetActivationFactory(&class_name)
            .map_err(|e| AudioError::message(format!(
                "AudioPolicyConfig no está disponible en Windows: {e}"
            )))?;

        for iid in [IID_21H2, IID_DOWNLEVEL] {
            let ptr = match query_interface_raw(&factory, &iid) {
                Ok(ptr) => ptr,
                Err(_) => continue,
            };

            let vtable = *(ptr as *const *const usize);
            let function: unsafe extern "system" fn(
                *mut c_void,
                i32,
                i32,
                i32,
                *mut c_void,
            ) -> HRESULT = std::mem::transmute(*vtable.add(SET_PERSISTED_DEFAULT_ENDPOINT_SLOT));

            // HSTRING is ABI-compatible with a pointer-sized value. The
            // HSTRING remains alive for the entire COM call.
            let multimedia_device_abi: *mut c_void =
                std::mem::transmute_copy(&multimedia_device_name);
            let communications_device_abi: *mut c_void =
                std::mem::transmute_copy(&communications_device_name);
            // Calls commonly open their render stream with the Communications
            // role. Persist all three roles so a private calling app cannot
            // fall back to the shared system endpoint and create a return path.
            let console_hr = function(
                ptr,
                pid as i32,
                eRender.0,
                eConsole.0,
                multimedia_device_abi,
            );
            let multimedia_hr = function(
                ptr,
                pid as i32,
                eRender.0,
                eMultimedia.0,
                multimedia_device_abi,
            );
            let communications_hr = function(
                ptr,
                pid as i32,
                eRender.0,
                eCommunications.0,
                communications_device_abi,
            );
            release_raw(ptr);

            // A partial assignment is unsafe here: the app may select whichever
            // role was not updated when it recreates its audio stream.
            if console_hr.is_ok() && multimedia_hr.is_ok() && communications_hr.is_ok() {
                return Ok(());
            }
            tracing::debug!(
                "SetPersistedDefaultAudioEndpoint falló para pid={pid}, console=0x{:08X}, multimedia=0x{:08X}, communications=0x{:08X}",
                console_hr.0 as u32,
                multimedia_hr.0 as u32,
                communications_hr.0 as u32
            );
        }
    }

    Err(AudioError::message(format!(
        "Windows no permitió cambiar la salida de la aplicación (pid {pid})"
    )))
}

fn pids_for_process_path(process_path: &str) -> Vec<u32> {
    let wanted = process_path.to_ascii_lowercase();
    crate::process::process_parent_map()
        .unwrap_or_default()
        .keys()
        .filter_map(|pid| {
            crate::process::process_image_path(*pid).and_then(|path| {
                if path.to_ascii_lowercase() == wanted {
                    Some(*pid)
                } else {
                    None
                }
            })
        })
        .collect()
}

fn full_render_device_id(device_id: &str) -> String {
    const PREFIX: &str = r#"\\?\SWD#MMDEVAPI#"#;
    const RENDER_SUFFIX: &str = "#{e6327cad-dcec-4949-ae8a-991e976a79d2}";

    if device_id.starts_with(PREFIX) && device_id.ends_with(RENDER_SUFFIX) {
        device_id.to_string()
    } else {
        format!("{PREFIX}{device_id}{RENDER_SUFFIX}")
    }
}

pub fn set_session_endpoint_by_pid(pid: u32, device_id: &str) -> Result<()> {
    set_process_default_endpoint(pid, device_id)
}
