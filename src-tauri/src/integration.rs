//! Deferred integration updates without terminating UnifiedAudio or Parsec.
pub fn start_pending()->Result<(),String>{
    use std::os::windows::process::CommandExt;
    let root=dirs::data_local_dir().ok_or("No hay AppData")?.join("NoEcho");
    if !root.join("pending-integration/engine.exe").exists() && !root.join("parsec-pending").exists(){return Ok(());}
    std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let helper=root.join("Apply-PendingIntegration.ps1");
    std::fs::write(&helper,include_str!("../../installer/Apply-PendingIntegration.ps1")).map_err(|e|e.to_string())?;
    std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-WindowStyle","Hidden","-ExecutionPolicy","Bypass","-File"]).arg(helper).arg("-NoEchoExecutable").arg(std::env::current_exe().map_err(|e|e.to_string())?).creation_flags(0x08000000).spawn().map_err(|e|e.to_string())?;
    Ok(())
}
