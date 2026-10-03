//! Read-only diagnosis of render endpoints and the sessions reserving them.
fn main() -> audio_core::Result<()> {
    let sessions = audio_core::SessionService::new().list_sessions()?;
    for d in audio_core::DeviceService::new().list_render_devices()? {
        if !d.is_virtual_shared_candidate { continue; }
        println!("{} ({})",d.name,d.id);
        for s in sessions.iter().filter(|s| s.device_id.as_deref()==Some(&d.id)) {
            println!("  {} PID {} {:?} muted {}",s.display_name,s.pid,s.state,s.muted);
        }
    }
    Ok(())
}
