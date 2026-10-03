//! Verify the existing internal endpoint without changing any audio routes.
fn main() -> anyhow::Result<()> {
    let device=audio_core::DeviceService::new().list_render_devices()?.into_iter()
        .find(|d|d.name.to_lowercase().contains("steam streaming speakers"))
        .ok_or_else(||anyhow::anyhow!("No internal endpoint"))?;
    let exe=std::env::current_exe()?.parent().unwrap().join("mix-smoke.exe");
    let mut child=std::process::Command::new(exe).args(["tone",&device.id,"0.03"]).spawn()?;
    let result=audio_core::loopback::probe_device_loopback_energy(Some(&device.id),1.0);
    let _=child.kill();let _=child.wait();
    let result=result?;
    println!("{}",serde_json::to_string(&result)?);
    anyhow::ensure!(result.frames>0 && result.peak_energy>0.005,"Internal loopback cannot carry audio");
    Ok(())
}
