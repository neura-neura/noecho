//! Process loopback capture helpers and shared monitor bridge.

use crate::error::{AudioError, Result};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopbackCapturePlan {
    pub mode: LoopbackMode,
    pub target_pids: Vec<u32>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopbackMode {
    ExcludeProcessTrees,
    IncludeProcessTrees,
    DeviceLoopback,
}

pub fn plan_shared_capture(exclude_pids: &[u32]) -> LoopbackCapturePlan {
    let mut notes = vec![
        "La captura de loopback por proceso requiere Windows 10 2004+ / Windows 11.".into(),
        "PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE permite validar exclusion.".into(),
    ];
    if exclude_pids.is_empty() {
        notes.push("Sin PIDs excluidos: loopback de dispositivo completo.".into());
        return LoopbackCapturePlan {
            mode: LoopbackMode::DeviceLoopback,
            target_pids: vec![],
            notes,
        };
    }
    notes.push(format!(
        "Se excluirán {} procesos raíz de la captura de verificación.",
        exclude_pids.len()
    ));
    LoopbackCapturePlan {
        mode: LoopbackMode::ExcludeProcessTrees,
        target_pids: exclude_pids.to_vec(),
        notes,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopbackProbeResult {
    pub seconds: f32,
    pub average_energy: f32,
    pub peak_energy: f32,
    pub frames: u64,
    pub method: String,
    pub notes: Vec<String>,
}

pub fn probe_default_loopback_energy(seconds: f32) -> Result<LoopbackProbeResult> {
    probe_device_loopback_energy(None, seconds)
}

pub fn probe_device_loopback_energy(
    device_id: Option<&str>,
    seconds: f32,
) -> Result<LoopbackProbeResult> {
    let _com = crate::com::ComApartment::init_mta()?;
    let seconds = seconds.clamp(0.2, 5.0);

    use windows::core::Interface;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = if let Some(id) = device_id {
            let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            enumerator.GetDevice(windows::core::PCWSTR(wide.as_ptr()))?
        } else {
            enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?
        };

        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let mix = client.GetMixFormat()?;
        let format = *mix;

        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            1_000_000,
            0,
            mix,
            None,
        )?;

        let capture: IAudioCaptureClient = client.GetService::<IAudioCaptureClient>()?;
        client.Start()?;

        let deadline = std::time::Instant::now() + Duration::from_secs_f32(seconds);
        let mut sum = 0f64;
        let mut peak = 0f32;
        let mut samples = 0u64;
        let mut notes = Vec::new();

        while std::time::Instant::now() < deadline {
            let mut packet_length = capture.GetNextPacketSize()?;
            while packet_length > 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames = 0u32;
                let mut flags = 0u32;
                capture.GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)?;
                if !data_ptr.is_null() && num_frames > 0 {
                    let channels = format.nChannels.max(1) as usize;
                    let bits = format.wBitsPerSample;
                    let frames = num_frames as usize;
                    if bits == 32 {
                        let count = frames * channels;
                        let slice = std::slice::from_raw_parts(data_ptr as *const f32, count);
                        for &s in slice {
                            let a = s.abs();
                            sum += a as f64;
                            if a > peak {
                                peak = a;
                            }
                            samples += 1;
                        }
                    } else if bits == 16 {
                        let count = frames * channels;
                        let slice = std::slice::from_raw_parts(data_ptr as *const i16, count);
                        for &s in slice {
                            let a = (s as f32).abs() / 32768.0;
                            sum += a as f64;
                            if a > peak {
                                peak = a;
                            }
                            samples += 1;
                        }
                    }
                }
                capture.ReleaseBuffer(num_frames)?;
                packet_length = capture.GetNextPacketSize()?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        client.Stop()?;
        windows::Win32::System::Com::CoTaskMemFree(Some(mix as *const _ as *const c_void));

        if samples == 0 {
            notes.push("No se recibieron frames de loopback.".into());
        }

        Ok(LoopbackProbeResult {
            seconds,
            average_energy: if samples == 0 {
                0.0
            } else {
                (sum / samples as f64) as f32
            },
            peak_energy: peak,
            frames: samples,
            method: "WASAPI device loopback".into(),
            notes,
        })
    }
}

pub fn process_loopback_supported() -> bool {
    os_build_number().map(|b| b >= 19041).unwrap_or(true)
}

fn os_build_number() -> Option<u32> {
    rtl_build_number()
}

fn rtl_build_number() -> Option<u32> {
    use windows::core::s;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    type RtlGetVersionFn = unsafe extern "system" fn(*mut OsVersionInfo) -> i32;
    #[repr(C)]
    struct OsVersionInfo {
        dw_os_version_info_size: u32,
        dw_major_version: u32,
        dw_minor_version: u32,
        dw_build_number: u32,
        dw_platform_id: u32,
        sz_csd_version: [u16; 128],
    }
    unsafe {
        let module = LoadLibraryW(windows::core::w!("ntdll.dll")).ok()?;
        let proc = GetProcAddress(module, s!("RtlGetVersion"))?;
        let func: RtlGetVersionFn = std::mem::transmute(proc);
        let mut info = OsVersionInfo {
            dw_os_version_info_size: std::mem::size_of::<OsVersionInfo>() as u32,
            dw_major_version: 0,
            dw_minor_version: 0,
            dw_build_number: 0,
            dw_platform_id: 0,
            sz_csd_version: [0; 128],
        };
        if func(&mut info) == 0 {
            Some(info.dw_build_number)
        } else {
            None
        }
    }
}

pub struct SharedMonitor {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl SharedMonitor {
    pub fn start(shared_device_id: String, physical_device_id: String) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let join = std::thread::Builder::new()
            .name("noecho-shared-monitor".into())
            .spawn(move || {
                if let Err(e) = run_monitor_loop(&shared_device_id, &physical_device_id, stop_thread)
                {
                    tracing::error!("shared monitor stopped with error: {e}");
                }
            })
            .map_err(|e| AudioError::message(format!("no se pudo iniciar monitor: {e}")))?;
        Ok(Self {
            stop,
            join: Some(join),
        })
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SharedMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

fn run_monitor_loop(
    shared_device_id: &str,
    physical_device_id: &str,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let _com = crate::com::ComApartment::init_mta()?;
    use windows::core::Interface;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::Media::Audio::{
        IAudioCaptureClient, IAudioClient, IAudioRenderClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let shared_w: Vec<u16> = shared_device_id
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let phys_w: Vec<u16> = physical_device_id
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let capture_dev = enumerator.GetDevice(windows::core::PCWSTR(shared_w.as_ptr()))?;
        let render_dev = enumerator.GetDevice(windows::core::PCWSTR(phys_w.as_ptr()))?;

        let capture_client: IAudioClient = capture_dev.Activate(CLSCTX_ALL, None)?;
        let render_client: IAudioClient = render_dev.Activate(CLSCTX_ALL, None)?;
        let mix = capture_client.GetMixFormat()?;

        capture_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            1_000_000,
            0,
            mix,
            None,
        )?;

        render_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            mix,
            None,
        )?;

        let event = CreateEventW(None, false, false, None)?;
        capture_client.SetEventHandle(event)?;

        let capturer: IAudioCaptureClient = capture_client.GetService::<IAudioCaptureClient>()?;
        let renderer: IAudioRenderClient = render_client.GetService::<IAudioRenderClient>()?;

        capture_client.Start()?;
        render_client.Start()?;

        while !stop.load(Ordering::SeqCst) {
            let wait = WaitForSingleObject(event, 50);
            if wait != WAIT_OBJECT_0 {
                continue;
            }
            let mut packet = capturer.GetNextPacketSize()?;
            while packet > 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                capturer.GetBuffer(&mut data_ptr, &mut frames, &mut flags, None, None)?;
                if frames > 0 {
                    if let Ok(padding) = render_client.GetCurrentPadding() {
                        let buffer_frames = render_client.GetBufferSize().unwrap_or(0);
                        let available = buffer_frames.saturating_sub(padding);
                        let to_write = frames.min(available);
                        if to_write > 0 {
                            if let Ok(render_ptr) = renderer.GetBuffer(to_write) {
                                if !data_ptr.is_null() && !render_ptr.is_null() {
                                    let bytes =
                                        (to_write as usize) * (*mix).nBlockAlign as usize;
                                    std::ptr::copy_nonoverlapping(data_ptr, render_ptr, bytes);
                                }
                                let _ = renderer.ReleaseBuffer(to_write, 0);
                            }
                        }
                    }
                }
                capturer.ReleaseBuffer(frames)?;
                packet = capturer.GetNextPacketSize()?;
            }
        }

        let _ = capture_client.Stop();
        let _ = render_client.Stop();
        let _ = CloseHandle(event);
        windows::Win32::System::Com::CoTaskMemFree(Some(mix as *const _ as *const c_void));
    }
    Ok(())
}
