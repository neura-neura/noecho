//! Opt-in hardware test. Produces quiet tones and never changes audio defaults.
use audio_core::{
    devices::DeviceService,
    mixer::{MixSettings, ProcessMixer},
    AppIdentity,
};
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};
use windows::Win32::{
    Media::Audio::*,
    System::Com::{CoCreateInstance, CLSCTX_ALL},
};
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for c in &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
fn settings(names: &[&str]) -> MixSettings {
    MixSettings {
        processor_output: None,
        processed_microphone: None,
        excluded: names
            .iter()
            .map(|n| AppIdentity {
                exe_name: (*n).into(),
                exe_path: None,
                display_name: None,
            })
            .collect(),
        microphone_to_remote: false,
        monitor: "none".into(),
    }
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("tone") {
        return tone(&args[2], args[3].parse()?);
    }
    let service = DeviceService::new();
    let shared = if args.iter().any(|a|a=="--internal"){
        audio_core::parsec::remote_device()?
    }else{service
        .find_shared_candidate(None)?
        .ok_or_else(|| anyhow::anyhow!("No dedicated virtual channel available"))?};
    anyhow::ensure!(
        !shared.is_default_multimedia && !shared.is_default_communications,
        "Shared channel must not be Windows default"
    );
    let physical = service
        .list_render_devices()?
        .into_iter()
        .find(|d| d.is_default_multimedia && d.id != shared.id)
        .ok_or_else(|| anyhow::anyhow!("No local default output"))?;
    let alternate = service
        .list_render_devices()?
        .into_iter()
        .find(|d| d.is_physical_candidate && !d.is_default_multimedia && d.id != shared.id)
        .unwrap_or(physical.clone());
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap();
    let public = dir.join("noecho-test-public.exe");
    let private = dir.join("noecho-test-private.exe");
    std::fs::copy(&exe, &public)?;
    std::fs::copy(&exe, &private)?;
    let mut children = Children(vec![
        Command::new(&public)
            .args(["tone", &physical.id, "0.03"])
            .spawn()?,
        Command::new(&private)
            .args(["tone", &alternate.id, "0.06"])
            .spawn()?,
    ]);
    std::thread::sleep(Duration::from_secs(2));
    let ignored:Vec<String>=audio_core::SessionService::new().list_sessions()?.into_iter()
        .filter_map(|s|s.exe_name).filter(|n|!n.to_ascii_lowercase().starts_with("noecho-test-")).collect();
    let configured=|names:&[&str]|{
        let mut s=settings(names);
        if args.iter().any(|a|a=="--internal"){s.processed_microphone=audio_core::processor::read().ok().map(|r|r.output_id);}
        s.excluded.extend(ignored.iter().map(|n|AppIdentity{exe_name:n.clone(),exe_path:None,display_name:None}));s
    };
    let mixer = ProcessMixer::start(shared.id.clone(), configured(&["noecho-test-private.exe"]))?;
    let wait = |private_count: usize,
                public_count: usize|
     -> anyhow::Result<audio_core::mixer::MixTelemetry> {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            let t = mixer.telemetry();
            if t.streams
                .iter()
                .filter(|s| s.name.to_lowercase().contains("noecho-test") && s.private)
                .count()
                == private_count
                && t.streams
                    .iter()
                    .filter(|s| s.name.to_lowercase().contains("noecho-test") && !s.private)
                    .count()
                    == public_count
                && t.private_peak > 0.02
            {
                return Ok(t);
            }
            anyhow::ensure!(Instant::now() < deadline, "Timed out: {:?}", t);
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let initial = wait(1, 1)?;
    anyhow::ensure!(
        initial.remote_peak > 0.015,
        "Public tone not present: {:?}",
        initial
    );
    println!(
        "included_public=true excluded_private=true remote_peak={:.4} private_peak={:.4}",
        initial.remote_peak, initial.private_peak
    );
    let delivered=audio_core::loopback::probe_device_loopback_energy(Some(&shared.id),0.5)?;
    anyhow::ensure!(delivered.peak_energy>0.001,"No audio delivered to receiver endpoint");
    mixer.update(configured(&[
        "noecho-test-private.exe",
        "noecho-test-public.exe",
    ]))?;
    let both = wait(2, 0)?;
    anyhow::ensure!(
        both.streams
            .iter()
            .filter(|s| s.name.to_lowercase().contains("noecho-test"))
            .all(|s| s.private),
        "Selection did not apply"
    );
    println!("first_toggle_applied=true");
    std::thread::sleep(Duration::from_millis(350));
    let excluded=audio_core::loopback::probe_device_loopback_energy(Some(&shared.id),0.5)?;
    anyhow::ensure!(excluded.peak_energy < delivered.peak_energy*0.05,"Selected audio leaked: before={} after={}",delivered.peak_energy,excluded.peak_energy);
    println!("delivered_peak={} after_excluding_both={} receiver_endpoint_verified=true",delivered.peak_energy,excluded.peak_energy);
    mixer.update(configured(&["noecho-test-private.exe"]))?;
    wait(1, 1)?;
    println!("uncheck_applied=true");
    children.0[1].kill()?;
    children.0[1].wait()?;
    children.0[1] = Command::new(&private)
        .args(["tone", &alternate.id, "0.06"])
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(12);
    let restarted = loop {
        let current = mixer.telemetry();
        if current.streams.iter().any(|s| s.pid == children.0[1].id() && s.private) {
            break current;
        }
        anyhow::ensure!(Instant::now() < deadline, "Restart not discovered: {:?}", current);
        std::thread::sleep(Duration::from_millis(100));
    };
    anyhow::ensure!(
        restarted
            .streams
            .iter()
            .any(|s| s.pid == children.0[1].id() && s.private),
        "Restart was not rediscovered"
    );
    println!(
        "restarted_app_excluded=true local_default_preserved={}",
        service
            .list_render_devices()?
            .iter()
            .any(|d| d.id == physical.id && d.is_default_multimedia)
    );
    println!(
        "remote_channel={} private_local_output={} warnings={:?}",
        shared.name, alternate.name, restarted.errors
    );
    drop(mixer);
    drop(children);
    std::fs::remove_file(public)?;
    std::fs::remove_file(private)?;
    Ok(())
}
fn tone(id: &str, amplitude: f32) -> anyhow::Result<()> {
    let _com = audio_core::com::ComApartment::init_mta()?;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let wide: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
        let device = enumerator.GetDevice(windows::core::PCWSTR(wide.as_ptr()))?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 2,
            nSamplesPerSec: 48000,
            nAvgBytesPerSec: 192000,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format,
            None,
        )?;
        let render = client.GetService::<IAudioRenderClient>()?;
        let capacity = client.GetBufferSize()?;
        client.Start()?;
        let mut offset = 0u64;
        loop {
            let frames = capacity.saturating_sub(client.GetCurrentPadding()?);
            if frames > 0 {
                let ptr = render.GetBuffer(frames)?.cast::<i16>();
                for i in 0..frames as usize {
                    let v = (((offset + i as u64) as f64 * 440.0 * std::f64::consts::TAU / 48000.0)
                        .sin()
                        * amplitude as f64
                        * 32767.0) as i16;
                    *ptr.add(i * 2) = v;
                    *ptr.add(i * 2 + 1) = v;
                }
                render.ReleaseBuffer(frames, 0)?;
                offset += frames as u64;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
