//! Process capture feeds a dedicated remote mix; local endpoints are never rerouted.
use crate::{
    error::{AudioError, Result},
    sessions::SessionService,
    types::AppIdentity,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use windows::{
    core::{implement, Interface, Ref},
    Win32::{
        Media::Audio::*,
        System::{
            Com::{CoCreateInstance, StructuredStorage::PROPVARIANT, BLOB, CLSCTX_ALL},
            Variant::VT_BLOB,
        },
    },
};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MixTelemetry {
    pub running: bool,
    pub system_peak: f32,
    pub private_peak: f32,
    pub voice_peak: f32,
    pub remote_peak: f32,
    pub microphone_name: String,
    pub monitor_output: String,
    pub monitor: String,
    pub microphone_to_remote: bool,
    pub streams: Vec<StreamStatus>,
    pub errors: Vec<String>,
    pub processor_source: Option<ProcessorSource>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessorSource {
    pub input_peak: f32,
    pub muted: bool,
    pub voice_enabled: bool,
    pub system_enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamStatus {
    pub pid: u32,
    pub private: bool,
    pub peak: f32,
    pub name: String,
}
#[derive(Clone)]
pub struct MixSettings {
    pub excluded: Vec<AppIdentity>,
    pub microphone_to_remote: bool,
    pub monitor: String,
    pub processor_output: Option<String>,
    pub processed_microphone: Option<String>,
}
enum Message {
    Update(
        MixSettings,
        mpsc::SyncSender<std::result::Result<(), String>>,
    ),
    Stop,
}
pub struct ProcessMixer {
    sender: mpsc::Sender<Message>,
    telemetry: Arc<Mutex<MixTelemetry>>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl ProcessMixer {
    pub fn start(shared: String, settings: MixSettings) -> Result<Self> {
        if !crate::loopback::process_loopback_supported() {
            return Err(AudioError::message(
                "La captura por proceso requiere Windows build 20348 o posterior (Windows 11).",
            ));
        }
        let (sender, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let telemetry = Arc::new(Mutex::new(MixTelemetry::default()));
        let data = telemetry.clone();
        let join = std::thread::Builder::new()
            .name("noecho-process-mix".into())
            .spawn(move || {
                let result = run(&shared, settings, receiver, data.clone(), ready_tx.clone());
                let mut t = data.lock();
                t.running = false;
                if let Err(e) = result {
                    let _ = ready_tx.send(Err(e.to_string()));
                    t.errors.push(e.to_string());
                }
            })?;
        match ready_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(())) => Ok(Self {
                sender,
                telemetry,
                join: Some(join),
            }),
            other => {
                let _ = sender.send(Message::Stop);
                // On activation timeout the worker may still be in a bounded OS call.
                let _ = join.join();
                Err(AudioError::message(match other {
                    Ok(Err(e)) => e,
                    _ => "La captura no respondió a tiempo.".into(),
                }))
            }
        }
    }
    pub fn update(&self, settings: MixSettings) -> Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(Message::Update(settings, tx))
            .map_err(|_| AudioError::message("El mezclador se detuvo."))?;
        rx.recv_timeout(Duration::from_secs(15))
            .map_err(|_| AudioError::message("El mezclador no confirmó el cambio."))?
            .map_err(AudioError::message)
    }
    pub fn telemetry(&self) -> MixTelemetry {
        self.telemetry.lock().clone()
    }
}
impl Drop for ProcessMixer {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Completion {
    sender: mpsc::SyncSender<std::result::Result<IAudioClient, String>>,
}
impl IActivateAudioInterfaceCompletionHandler_Impl for Completion_Impl {
    fn ActivateCompleted(
        &self,
        operation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let result = (|| unsafe {
            let mut hr = windows::core::HRESULT(0);
            let mut unknown = None;
            operation
                .as_ref()
                .ok_or_else(windows::core::Error::empty)?
                .GetActivateResult(&mut hr, &mut unknown)?;
            hr.ok()?;
            unknown
                .ok_or_else(windows::core::Error::empty)?
                .cast::<IAudioClient>()
        })();
        let _ = self.sender.send(result.map_err(|e| e.to_string()));
        Ok(())
    }
}

fn format() -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 2,
        nSamplesPerSec: 48000,
        nAvgBytesPerSec: 192000,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    }
}
struct Capture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    queue: VecDeque<f32>,
    private: bool,
    name: String,
    peak: f32,
}
impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
impl Capture {
    unsafe fn endpoint(enumerator: &IMMDeviceEnumerator, id: &str) -> Result<Self> {
        let wide: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
        let device = enumerator.GetDevice(windows::core::PCWSTR(wide.as_ptr()))?;
        let client = device.Activate::<IAudioClient>(CLSCTX_ALL, None)?;
        Self::initialize(client, AUDCLNT_STREAMFLAGS_LOOPBACK, false, "UnifiedAudio".into())
    }
    unsafe fn process(pid: u32, private: bool, name: String) -> Result<Self> {
        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };
        // Borrowed blob: do not PropVariantClear this stack-backed value.
        let mut variant = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        (*variant.Anonymous.Anonymous).vt = VT_BLOB;
        (*variant.Anonymous.Anonymous).Anonymous.blob = BLOB {
            cbSize: std::mem::size_of_val(&params) as u32,
            pBlobData: &mut params as *mut _ as *mut u8,
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let completion: IActivateAudioInterfaceCompletionHandler = Completion { sender: tx }.into();
        let _operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*variant),
            &completion,
        )?;
        let client = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| AudioError::message(format!("Timeout capturando PID {pid}")))?
            .map_err(AudioError::message)?;
        Self::initialize(client, AUDCLNT_STREAMFLAGS_LOOPBACK, private, name)
    }
    unsafe fn initialize(
        client: IAudioClient,
        flags: u32,
        private: bool,
        name: String,
    ) -> Result<Self> {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            flags | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format(),
            None,
        )?;
        let capture = client.GetService::<IAudioCaptureClient>()?;
        client.Start()?;
        Ok(Self {
            client,
            capture,
            queue: VecDeque::new(),
            private,
            name,
            peak: 0.0,
        })
    }
    unsafe fn read(&mut self) -> Result<()> {
        self.peak *= 0.8;
        while self.capture.GetNextPacketSize()? > 0 {
            let mut ptr = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            self.capture
                .GetBuffer(&mut ptr, &mut frames, &mut flags, None, None)?;
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || ptr.is_null();
            for i in 0..frames as usize * 2 {
                let v = if silent {
                    0.0
                } else {
                    *ptr.cast::<i16>().add(i) as f32 / 32768.0
                };
                self.peak = self.peak.max(v.abs());
                self.queue.push_back(v);
            }
            self.capture.ReleaseBuffer(frames)?;
            // Bound latency on slow or disconnected output; drop whole stereo frames.
            while self.queue.len() > 9600 {
                self.queue.pop_front();
                self.queue.pop_front();
            }
        }
        Ok(())
    }
}
struct Output {
    client: IAudioClient,
    render: IAudioRenderClient,
    capacity: u32,
}
impl Drop for Output {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
impl Output {
    unsafe fn open(device: &IMMDevice) -> Result<Self> {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format(),
            None,
        )?;
        let render = client.GetService::<IAudioRenderClient>()?;
        let capacity = client.GetBufferSize()?;
        client.Start()?;
        Ok(Self {
            client,
            render,
            capacity,
        })
    }
    unsafe fn available(&self) -> Result<usize> {
        Ok(self
            .capacity
            .saturating_sub(self.client.GetCurrentPadding()?) as usize)
    }
    unsafe fn write(&self, data: &[f32]) -> Result<()> {
        let frames = data.len() / 2;
        if frames == 0 {
            return Ok(());
        }
        let buffer = self.render.GetBuffer(frames as u32)?.cast::<i16>();
        for (i, v) in data.iter().enumerate() {
            *buffer.add(i) = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
        self.render.ReleaseBuffer(frames as u32, 0)?;
        Ok(())
    }
    unsafe fn flush(&self) -> Result<()> {
        self.client.Stop()?;
        self.client.Reset()?;
        self.client.Start()?;
        Ok(())
    }
}

/// Disjoint process trees: never capture an ancestor of a private PID, and never
/// capture the same child twice. Process discovery uses all render endpoints.
fn targets(settings: &MixSettings, shared: &str) -> Result<BTreeMap<u32, (bool, String)>> {
    let sessions = SessionService::new().list_capture_sessions()?;
    let parents = crate::process::process_parent_map()?;
    let mut private = BTreeSet::new();
    for app in &settings.excluded {
        let seeds: Vec<_> = sessions
            .iter()
            .filter(|s| {
                s.exe_name
                    .as_ref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&app.exe_name))
            })
            .map(|s| s.pid)
            .collect();
        for pid in crate::process::expand_related_pids(&seeds, Some(&app.exe_name))? {
            private.extend(crate::process::collect_process_tree(pid)?);
        }
    }
    let mut desired = BTreeMap::new();
    for s in sessions {
        // A processor already contains a mix of other apps. Capturing it again
        // would duplicate sound and could reintroduce private audio.
        if s.exe_name.as_deref().is_some_and(crate::processor::is_processor) {
            continue;
        }
        if s.pid == 0 || !parents.contains_key(&s.pid) || s.pid == std::process::id() || s.device_id.as_deref() == Some(shared) {
            continue;
        }
        let is_private = private.contains(&s.pid);
        if settings.processor_output.is_some() && !is_private { continue; }
        desired.insert(s.pid, (is_private, s.display_name));
    }
    Ok(disjoint_roots(desired, &private, &parents))
}
fn disjoint_roots(mut desired: BTreeMap<u32, (bool, String)>, private: &BTreeSet<u32>, parents: &std::collections::HashMap<u32, u32>) -> BTreeMap<u32, (bool, String)> {
    // A mixed ancestor cannot be included without leaking a selected child.
    desired.retain(|pid, (is_private, _)| *is_private || !private.iter().any(|p| ancestor(*pid, *p, parents)));
    let keys: Vec<_> = desired.keys().copied().collect();
    desired.retain(|pid, _| {
        !keys
            .iter()
            .any(|other| *other != *pid && ancestor(*other, *pid, &parents))
    });
    desired
}
fn ancestor(root: u32, mut pid: u32, parents: &std::collections::HashMap<u32, u32>) -> bool {
    for _ in 0..128 {
        let Some(p) = parents.get(&pid) else {
            return false;
        };
        if *p == root {
            return true;
        }
        if *p == 0 || *p == pid {
            return false;
        }
        pid = *p;
    }
    false
}
unsafe fn reconcile(
    streams: &mut BTreeMap<u32, Capture>,
    desired: BTreeMap<u32, (bool, String)>,
    errors: &mut Vec<String>,
) {
    streams.retain(|pid, c| {
        desired
            .get(pid)
            .is_some_and(|(private, name)| *private == c.private && *name == c.name)
    });
    for (pid, (private, name)) in desired {
        if streams.contains_key(&pid) {
            continue;
        }
        match Capture::process(pid, private, name) {
            Ok(c) => {
                streams.insert(pid, c);
            }
            Err(e) => errors.push(format!("PID {pid}: {e}")),
        }
    }
}
unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
    let store = device.OpenPropertyStore(windows::Win32::System::Com::STGM_READ).ok()?;
    let value = store.GetValue(&windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName).ok()?;
    if value.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_LPWSTR {
        // Property string is borrowed; PROPVARIANT owns and releases it.
        Some(crate::devices::pwstr_to_string(value.Anonymous.Anonymous.Anonymous.pwszVal))
    } else { None }
}

fn run(
    shared: &str,
    mut settings: MixSettings,
    receiver: mpsc::Receiver<Message>,
    telemetry: Arc<Mutex<MixTelemetry>>,
    ready: mpsc::SyncSender<std::result::Result<(), String>>,
) -> Result<()> {
    let _com = crate::com::ComApartment::init_mta()?;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let wide: Vec<_> = shared.encode_utf16().chain(Some(0)).collect();
        let output = match enumerator
            .GetDevice(windows::core::PCWSTR(wide.as_ptr()))
            .map_err(AudioError::from)
            .and_then(|d| Output::open(&d))
        {
            Ok(o) => o,
            Err(e) => {
                let _ = ready.send(Err(e.to_string()));
                return Err(e);
            }
        };
        let mut microphone = None;
        let mut microphone_name = String::new();
        let mut mic_device_id = String::new();
        let mut monitor_output: Option<Output> = None;
        let mut monitor_id = String::new();
        let mut monitor_name = String::new();
        let mut streams = BTreeMap::new();
        let mut errors = Vec::new();
        let mut processor = settings.processor_output.as_deref().map(|id| Capture::endpoint(&enumerator, id)).transpose()?;
        let mut cancellation = crate::cancellation::EchoCancellation::new()?;
        reconcile(&mut streams, targets(&settings, shared)?, &mut errors);
        telemetry.lock().errors = errors.clone();
        telemetry.lock().running = true;
        let _ = ready.send(Ok(()));
        let mut next_discovery = Instant::now();
        loop {
            match receiver.try_recv() {
                Ok(Message::Stop) | Err(mpsc::TryRecvError::Disconnected) => break,
                Ok(Message::Update(next, ack)) => {
                    // Stop output and discard buffered audio before acknowledging exclusion.
                    let result = (|| -> Result<()> {
                        let selection_changed = serde_json::to_string(&settings.excluded)? != serde_json::to_string(&next.excluded)?;
                        output.flush()?;
                        streams.clear();
                        if let Some(m) = microphone.as_mut() {
                            let m: &mut Capture = m;
                            m.queue.clear();
                        }
                        if next.processor_output != settings.processor_output {
                            return Err(AudioError::message("Cambió la salida de UnifiedAudio. Desactiva y vuelve a activar NoEcho."));
                        }
                        if selection_changed { cancellation = crate::cancellation::EchoCancellation::new()?; }
                        if let Some(p) = &mut processor { p.queue.clear(); }
                        settings = next;
                        let desired = targets(&settings, shared)?;
                        errors.clear();
                        reconcile(&mut streams, desired, &mut errors);
                        Ok(())
                    })();
                    let _ = ack.send(result.map_err(|e| e.to_string()));
                    telemetry.lock().errors = errors.clone();
                    next_discovery = Instant::now();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if Instant::now() >= next_discovery {
                if let Some(expected) = &settings.processor_output {
                    let route = crate::processor::read()?;
                    if &route.output_id != expected {
                        output.flush()?;
                        return Err(AudioError::message("Cambió la conexión de UnifiedAudio. Revisa su entrada de sonido y vuelve a activar NoEcho."));
                    }
                }
                errors.clear();
                match targets(&settings, shared) {
                    Ok(d) => reconcile(&mut streams, d, &mut errors),
                    Err(e) => {
                        streams.clear();
                        output.flush()?;
                        errors.push(e.to_string());
                    }
                }
                let mic_device=if let Some(id)=&settings.processed_microphone{
                    let id:Vec<_>=id.encode_utf16().chain(Some(0)).collect();
                    enumerator.GetDevice(windows::core::PCWSTR(id.as_ptr()))
                }else{enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications)};
                if let Ok(d) = mic_device {
                    let id = crate::devices::owned_pwstr_to_string(d.GetId()?);
                    if id != mic_device_id || microphone.is_none() {
                        microphone = None;
                        mic_device_id = id;
                        microphone_name = friendly_name(&d).unwrap_or_else(|| "Micrófono de comunicaciones de Windows".into());
                        match d
                            .Activate::<IAudioClient>(CLSCTX_ALL, None)
                            .map_err(AudioError::from)
                            .and_then(|c| Capture::initialize(c, if settings.processed_microphone.is_some(){AUDCLNT_STREAMFLAGS_LOOPBACK}else{0}, false, microphone_name.clone()))
                        {
                            Ok(c) => microphone = Some(c),
                            Err(e) => errors.push(format!("Micrófono: {e}")),
                        }
                    }
                } else {
                    microphone = None;
                    mic_device_id.clear();
                    errors.push("No hay micrófono de comunicaciones disponible.".into());
                }
                if settings.monitor == "none" {
                    monitor_output = None;
                    monitor_id.clear();
                } else if let Ok(d) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    let id = crate::devices::owned_pwstr_to_string(d.GetId()?);
                    if id == shared {
                        monitor_output = None;
                        errors.push("El canal remoto no puede ser la salida de escucha.".into());
                    } else if id != monitor_id || monitor_output.is_none() {
                        monitor_output = None;
                        monitor_id = id;
                        monitor_name = friendly_name(&d).unwrap_or_else(|| "Salida multimedia actual de Windows".into());
                        match Output::open(&d) {
                            Ok(o) => monitor_output = Some(o),
                            Err(e) => errors.push(format!("Escucha: {e}")),
                        }
                    }
                }
                next_discovery = Instant::now() + Duration::from_millis(750);
            }
            let mut broken = Vec::new();
            for (pid, c) in &mut streams {
                if let Err(e) = c.read() {
                    errors.push(format!("PID {pid}: {e}"));
                    broken.push(*pid);
                }
            }
            for pid in broken {
                streams.remove(&pid);
            }
            if let Some(m) = microphone.as_mut() {
                if let Err(e) = m.read() {
                    errors.push(format!("Micrófono: {e}"));
                    microphone = None;
                }
            }
            if let Some(p) = &mut processor { p.read()?; }
            let available = output.available()?;
            let frames = if processor.is_some() {
                if available >= 480 && processor.as_ref().is_some_and(|p| p.queue.len() >= 960) { 480 } else { 0 }
            } else { available.min(480) };
            if frames > 0 {
                let mut remote = vec![0.0f32; frames * 2];
                let mut private_mix = vec![0.0f32; frames * 2];
                let mut system = vec![0.0f32; frames * 2];
                let mut voice = vec![0.0f32; frames * 2];
                for c in streams.values_mut() {
                    for i in 0..frames * 2 {
                        let v = c.queue.pop_front().unwrap_or(0.0);
                        system[i] += v;
                        if c.private {
                            private_mix[i] += v;
                        } else {
                            remote[i] += v;
                        }
                    }
                }
                if let Some(m) = microphone.as_mut() {
                    for i in 0..frames * 2 {
                        voice[i] = m.queue.pop_front().unwrap_or(0.0);
                        if settings.microphone_to_remote {
                            remote[i] += voice[i];
                        }
                    }
                }
                if let Some(p) = &mut processor {
                    let processed: Vec<_> = (0..frames*2).map(|_| p.queue.pop_front().unwrap_or(0.0)).collect();
                    system.copy_from_slice(&processed);
                    if settings.excluded.is_empty() { remote.copy_from_slice(&processed); }
                    else { cancellation.process(&private_mix, &processed, &mut remote)?; }
                }
                output.write(&remote)?;
                if let Some(m) = monitor_output.as_ref() {
                    let data = match settings.monitor.as_str() {
                        "voice" => &voice,
                        "private" => &private_mix,
                        "system" => &system,
                        _ => &remote,
                    };
                    let available = m.available()?.min(frames);
                    m.write(&data[..available * 2])?;
                }
                let peak = |s: &[f32]| s.iter().map(|v| v.abs()).fold(0.0f32, f32::max).min(1.0);
                let mut previous=telemetry.lock();
                let held=|v:f32,p:f32|v.max(p*0.94);
                *previous = MixTelemetry {
                    running: true,
                    system_peak: held(peak(&system),previous.system_peak),
                    private_peak: held(peak(&private_mix),previous.private_peak),
                    voice_peak: held(peak(&voice),previous.voice_peak),
                    remote_peak: held(peak(&remote),previous.remote_peak),
                    microphone_name: microphone_name.clone(),
                    monitor_output: monitor_name.clone(),
                    monitor: settings.monitor.clone(),
                    microphone_to_remote: settings.microphone_to_remote,
                    streams: streams
                        .iter()
                        .map(|(pid, c)| StreamStatus {
                            pid: *pid,
                            private: c.private,
                            peak: c.peak,
                            name: c.name.clone(),
                        })
                        .collect(),
                    errors: errors.clone(),
                    processor_source: None,
                };
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ancestry_is_bounded_and_detects_children() {
        let parents = [(2, 1), (3, 2), (4, 4)].into_iter().collect();
        assert!(ancestor(1, 3, &parents));
        assert!(!ancestor(3, 1, &parents));
        assert!(!ancestor(1, 4, &parents));
    }
    #[test]
    fn private_child_omits_public_ancestors_and_deduplicates_public_tree() {
        let parents = [(2,1),(3,2),(6,5)].into_iter().collect();
        let candidates = [(1,(false,"parent".into())),(2,(false,"middle".into())),(3,(true,"private".into())),(5,(false,"public".into())),(6,(false,"child".into()))].into_iter().collect();
        let roots = disjoint_roots(candidates, &[3].into_iter().collect(), &parents);
        assert_eq!(roots.keys().copied().collect::<Vec<_>>(), vec![3,5]);
        assert!(roots[&3].0);
        assert!(!roots[&5].0);
    }
}
