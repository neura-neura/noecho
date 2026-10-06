//! Per-user Windows logon registration. No administrator rights required.
use crate::error::{AudioError,Result};
pub fn configure(enabled:bool)->Result<()> {
    apply(enabled,&std::env::current_exe()?,"HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run")
}
fn apply(enabled:bool,exe:&std::path::Path,key:&str)->Result<()> {
    use std::os::windows::process::CommandExt;
    let script=r#"$ErrorActionPreference='Stop'; $key=$env:NOECHO_STARTUP_KEY; if ($env:NOECHO_AUTOSTART -eq '1') { New-Item -Path $key -Force | Out-Null; Set-ItemProperty -Path $key -Name NoEcho -Value ('"'+$env:NOECHO_EXE+'" --autostart') } elseif (Get-ItemProperty -Path $key -Name NoEcho -ErrorAction SilentlyContinue) { Remove-ItemProperty -Path $key -Name NoEcho }"#;
    let output=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",script]).env("NOECHO_AUTOSTART",if enabled{"1"}else{"0"}).env("NOECHO_EXE",exe).env("NOECHO_STARTUP_KEY",key).creation_flags(0x08000000).output()?;
    if !output.status.success(){return Err(AudioError::message(format!("No se pudo configurar el inicio con Windows: {}",String::from_utf8_lossy(&output.stderr).trim())));}Ok(())
}
#[cfg(test)] mod tests{
    #[test] fn registers_quoted_executable_and_removes_registration(){
        use std::os::windows::process::CommandExt;
        let key=format!("HKCU:\\Software\\NoEchoStartupTest{}",std::process::id());
        super::apply(true,std::path::Path::new("C:\\Program Files\\NoEcho\\noecho.exe"),&key).unwrap();
        let first=std::process::Command::new("powershell.exe").args(["-NoProfile","-Command","(Get-ItemProperty -Path $env:NOECHO_STARTUP_KEY -Name NoEcho).NoEcho"]).env("NOECHO_STARTUP_KEY",&key).creation_flags(0x08000000).output().unwrap();
        super::apply(false,std::path::Path::new("unused"),&key).unwrap();
        let last=std::process::Command::new("powershell.exe").args(["-NoProfile","-Command","$ErrorActionPreference='Stop'; $v=Get-ItemProperty -Path $env:NOECHO_STARTUP_KEY -Name NoEcho -ErrorAction SilentlyContinue; Remove-Item -LiteralPath $env:NOECHO_STARTUP_KEY; if($v){exit 1}"]).env("NOECHO_STARTUP_KEY",&key).creation_flags(0x08000000).output().unwrap();
        assert!(first.status.success());assert_eq!(String::from_utf8_lossy(&first.stdout).trim(),"\"C:\\Program Files\\NoEcho\\noecho.exe\" --autostart");assert!(last.status.success());
    }
}
