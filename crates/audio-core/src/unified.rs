//! Controls the in-engine NoEcho stage. This path never opens a render endpoint.
use crate::{error::{AudioError, Result}, mixer::{MixTelemetry, ProcessorSource}, types::AppIdentity};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, sync::{Arc,atomic::{AtomicBool,Ordering}}, time::{Duration, Instant}};
const CLIENT: &str = r#"
$ErrorActionPreference='Stop'
do {
$pipe=[IO.Pipes.NamedPipeClientStream]::new('.', $env:NOECHO_PIPE, [IO.Pipes.PipeDirection]::InOut, [IO.Pipes.PipeOptions]::Asynchronous)
try {
 $pipe.Connect(4000)
 $body=[Text.Encoding]::UTF8.GetBytes($env:NOECHO_REQUEST)
 $size=[BitConverter]::GetBytes([uint32]$body.Length)
 $pipe.Write($size,0,4); $pipe.Write($body,0,$body.Length); $pipe.Flush()
 function ReadExact([int]$count) {
  $bytes=[byte[]]::new($count); $offset=0
  while($offset -lt $count) {
   $task=$pipe.ReadAsync($bytes,$offset,$count-$offset)
   if(-not $task.Wait(11000)){throw 'UnifiedAudio timeout'}
   $got=$task.Result; if($got -eq 0){throw 'UnifiedAudio disconnected'}; $offset+=$got
  }
  return ,$bytes
 }
 $header=ReadExact 4; $length=[BitConverter]::ToUInt32($header,0)
 if($length -gt 65536){throw 'Invalid response'}
 $response=ReadExact $length
 $json=[Text.Encoding]::UTF8.GetString($response) | ConvertFrom-Json
 [Console]::Out.WriteLine(($json | ConvertTo-Json -Compress -Depth 16)); [Console]::Out.Flush()
} catch {[Console]::Error.Write($_.Exception.Message); exit 1} finally {$pipe.Dispose()}
 if($env:NOECHO_STREAM -eq '1'){Start-Sleep -Milliseconds 100}
} while($env:NOECHO_STREAM -eq '1')
"#;
fn request_at(pipe: &str, name: &str, payload: Value) -> Result<Value> {
    use std::os::windows::process::CommandExt;
    let output = Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",CLIENT])
        .env("NOECHO_PIPE",pipe)
        .env("NOECHO_STREAM","0")
        .env("NOECHO_REQUEST", json!({"contractVersion":1,"messageId":"noecho","name":name,"payload":payload}).to_string())
        .stdin(Stdio::null()).creation_flags(0x08000000).output()?;
    if !output.status.success() {
        return Err(AudioError::message("No se pudo conectar con el motor compatible de UnifiedAudio. Instala la actualización incluida y reinicia UnifiedAudio."));
    }
    let response: Value = serde_json::from_slice(&output.stdout)?;
    if !response["error"].is_null() { return Err(AudioError::message(response["error"].to_string())); }
    Ok(response["payload"].clone())
}
fn configure_at(pipe:&str,enabled:bool,apps:&[AppIdentity])->Result<Value>{
    request_at(pipe,"noecho.configure",json!({"enabled":enabled,"apps":apps.iter().map(|a| &a.exe_name).collect::<Vec<_>>()}))
}
fn telemetry(v: &Value) -> MixTelemetry {
    let peak = |key: &str| v[key].as_f64().unwrap_or_default() as f32;
    MixTelemetry {running:v["active"].as_bool().unwrap_or(false) && v["running"].as_bool().unwrap_or(false), voice_peak:peak("voicePeak"), system_peak:peak("systemPeak"), private_peak:peak("privatePeak"),remote_peak:peak("remotePeak"), microphone_name:"Voz procesada de UnifiedAudio".into(), errors:v["error"].as_str().filter(|e| !e.is_empty()).map(|e|vec![e.to_string()]).unwrap_or_default(), processor_source: v["systemEnabled"].as_bool().map(|system_enabled| ProcessorSource { input_peak:peak("inputPeak"), muted:v["muted"].as_bool().unwrap_or(false), voice_enabled:v["voiceEnabled"].as_bool().unwrap_or(false),system_enabled }),..Default::default()}
}
pub struct UnifiedStage { pipe:String, data: Arc<Mutex<(MixTelemetry, Instant)>>, child:Arc<Mutex<Option<Child>>>, apps:Arc<Mutex<Vec<AppIdentity>>>, stop:Arc<AtomicBool>, join:Option<std::thread::JoinHandle<()>> }
impl UnifiedStage {
    pub fn start(apps: &[AppIdentity]) -> Result<Self> {
        Self::start_at("UnifiedAudio.NoEcho.v1",apps)
    }
    fn start_at(pipe:&str,apps:&[AppIdentity])->Result<Self>{
        let initial = configure_at(pipe,true, apps)?;
        use std::os::windows::process::CommandExt;
        let data=Arc::new(Mutex::new((telemetry(&initial),Instant::now())));
        // One helper for the lifetime of the stage; no process launch per meter sample.
        let child=Arc::new(Mutex::new(None::<Child>));
        let saved_apps=Arc::new(Mutex::new(apps.to_vec()));
        let stop=Arc::new(AtomicBool::new(false));
        let target=data.clone();let worker_child=child.clone();let worker_apps=saved_apps.clone();let worker_stop=stop.clone();let worker_pipe=pipe.to_string();
        let join=std::thread::spawn(move || {
          while !worker_stop.load(Ordering::SeqCst) {
            let mut process=match Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",CLIENT])
            .env("NOECHO_PIPE",&worker_pipe).env("NOECHO_STREAM","1")
            .env("NOECHO_REQUEST",json!({"contractVersion":1,"messageId":"meters","name":"noecho.status","payload":{}}).to_string())
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(0x08000000).spawn(){Ok(c)=>c,Err(e)=>{target.lock().0.errors=vec![format!("No se pudo leer UnifiedAudio: {e}")];break;}};
        let stdout=process.stdout.take().expect("piped telemetry output");
        {let mut slot=worker_child.lock();if worker_stop.load(Ordering::SeqCst){let _=process.kill();let _=process.wait();break;}*slot=Some(process);}
            let mut frames=None;
            let mut last_audio=Instant::now();
            for line in BufReader::new(stdout).lines() {
                let value=line.ok().and_then(|s|serde_json::from_str::<Value>(&s).ok());
                let Some(response)=value else {break};
                if !response["error"].is_null(){break;}
                let v=&response["payload"];
                if v["active"].as_bool()==Some(false){break;}
                let mut t=telemetry(v);
                if let Some(current)=v["renderFrames"].as_i64() {
                    if frames!=Some(current){last_audio=Instant::now();frames=Some(current);}
                    if last_audio.elapsed()>Duration::from_secs(10){
                        t.running=false;
                        t.errors.push("UnifiedAudio no está entregando audio. Revisa su entrada y su salida.".into());
                    }
                }
                let mut data=target.lock();
                let smooth=|now:f32,prior:f32|{let p=now.max(prior*0.65);if p<0.00001{0.0}else{p}};
                t.voice_peak=smooth(t.voice_peak,data.0.voice_peak);
                t.system_peak=smooth(t.system_peak,data.0.system_peak);
                t.private_peak=smooth(t.private_peak,data.0.private_peak);
                t.remote_peak=smooth(t.remote_peak,data.0.remote_peak);
                *data=(t,Instant::now());
            }
            if let Some(mut c)=worker_child.lock().take(){let _=c.kill();let _=c.wait();}
            // A restarted Engine Host loses its temporary filter. Reapply the
            // latest selection before reconnecting the meter helper.
            while !worker_stop.load(Ordering::SeqCst){
                let result={let current=worker_apps.lock();configure_at(&worker_pipe,true,&current)};
                match result {Ok(v)=>{*target.lock()=(telemetry(&v),Instant::now());break;},Err(_)=>{
                    let mut t=target.lock();t.0.running=false;t.0.errors=vec!["Reconectando con UnifiedAudio. NoEcho volverá a aplicar los programas seleccionados.".into()];
                }}
                for _ in 0..10{if worker_stop.load(Ordering::SeqCst){break;}std::thread::sleep(Duration::from_millis(100));}
            }
          }
        });
        Ok(Self {pipe:pipe.to_string(),data,child,apps:saved_apps,stop,join:Some(join)})
    }
    pub fn update(&self, apps:&[AppIdentity]) -> Result<()> {
        let mut current=self.apps.lock();configure_at(&self.pipe,true,apps)?;*current=apps.to_vec();
        Ok(())
    }
    pub fn telemetry(&self)->MixTelemetry {
        let data=self.data.lock();let mut t=data.0.clone();
        if data.1.elapsed()>Duration::from_secs(15){t.running=false;if t.errors.is_empty(){t.errors=vec!["Reconectando con UnifiedAudio. La lectura de niveles no responde.".into()];}}
        t
    }
}
impl Drop for UnifiedStage {
    fn drop(&mut self) {
        self.stop.store(true,Ordering::SeqCst);
        if let Some(c)=self.child.lock().as_mut(){let _=c.kill();}
        if let Some(j)=self.join.take(){let _=j.join();}
        if let Some(mut c)=self.child.lock().take(){let _=c.wait();}
        let _=configure_at(&self.pipe,false,&[]);
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn reconnects_and_restores_selection_after_meter_disconnect(){
        use std::os::windows::process::CommandExt;
        let pipe=format!("NoEcho.Reconnect.Test.{}",std::process::id());
        let server=r#"
$ErrorActionPreference='Stop'; $configured=0; $interrupted=$false
function ReadExact($p,[int]$n){$b=[byte[]]::new($n);$i=0;while($i -lt $n){$r=$p.Read($b,$i,$n-$i);if(!$r){throw 'closed'};$i+=$r};return ,$b}
for($i=0;$i -lt 100;$i++){
 $p=[IO.Pipes.NamedPipeServerStream]::new($env:NOECHO_PIPE,[IO.Pipes.PipeDirection]::InOut,1,[IO.Pipes.PipeTransmissionMode]::Byte,[IO.Pipes.PipeOptions]::Asynchronous)
 if($i -eq 0){[Console]::Out.WriteLine('ready');[Console]::Out.Flush()}
 $wait=$p.WaitForConnectionAsync();if(-not $wait.Wait(15000)){exit 2}
 $h=ReadExact $p 4;$b=ReadExact $p ([BitConverter]::ToUInt32($h,0));$q=[Text.Encoding]::UTF8.GetString($b)|ConvertFrom-Json
 $responseError=$null;$stop=$false
 if($q.name -eq 'noecho.configure'){
  if($q.payload.enabled){if($q.payload.apps[0] -ne 'ayugram.exe'){exit 3};$configured++}
  else{$stop=$true}
 } elseif(-not $interrupted){$responseError='simulated disconnect';$interrupted=$true}
 $peak=0.0;if($configured -gt 1){$peak=0.75}
 $response=@{error=$responseError;payload=@{active=$true;running=$true;remotePeak=$peak}}|ConvertTo-Json -Depth 8
 $bytes=[Text.Encoding]::UTF8.GetBytes($response);$h=[BitConverter]::GetBytes([uint32]$bytes.Length)
 $p.Write($h,0,4);$p.Write($bytes,0,$bytes.Length);$p.Flush();$p.Dispose()
 if($stop){if($configured -lt 2){exit 4};exit 0}
}
exit 5
"#;
        let mut child=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",server]).env("NOECHO_PIPE",&pipe).stdout(Stdio::piped()).creation_flags(0x08000000).spawn().unwrap();
        let mut ready=String::new();BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready).unwrap();assert_eq!(ready.trim(),"ready");
        let stage=UnifiedStage::start_at(&pipe,&[AppIdentity{exe_name:"ayugram.exe".into(),exe_path:None,display_name:None}]).unwrap();
        let deadline=Instant::now()+Duration::from_secs(20);
        while stage.telemetry().remote_peak<0.7 && Instant::now()<deadline{std::thread::sleep(Duration::from_millis(100));}
        let recovered=stage.telemetry();drop(stage);
        assert!(child.wait().unwrap().success());assert!(recovered.running);assert!(recovered.remote_peak>=0.7);assert!(recovered.errors.is_empty());
    }
    #[test] fn maps_processed_voice_and_final_output_meters() {
        let t=telemetry(&json!({"active":true,"running":true,"voicePeak":0.5,"systemPeak":0.3,"privatePeak":0.2,"remotePeak":0.1,"error":"","inputPeak":0.6,"muted":true,"voiceEnabled":true,"systemEnabled":false}));
        assert!(t.running);
        assert_eq!(t.voice_peak,0.5);
        assert_eq!(t.remote_peak,0.1);
        assert!(t.errors.is_empty());
        let source=t.processor_source.unwrap();assert!(source.muted);assert!(!source.system_enabled);assert_eq!(source.input_peak,0.6);
        assert!(!telemetry(&json!({"active":true,"running":false,"error":"capture failed"})).running);
    }
    #[test] fn real_pipe_transport_applies_first_selection_and_stop() {
        use std::{io::{BufRead,BufReader},os::windows::process::CommandExt};
        let pipe=format!("NoEcho.Test.{}",std::process::id());
        let server=r#"
$ErrorActionPreference='Stop'
$active=$false; $apps=@()
function ReadExact($p,[int]$n){$b=[byte[]]::new($n);$i=0;while($i -lt $n){$r=$p.Read($b,$i,$n-$i);if(!$r){throw 'closed'};$i+=$r};return ,$b}
for($i=0;$i -lt 6;$i++){
 $p=[IO.Pipes.NamedPipeServerStream]::new($env:NOECHO_PIPE,[IO.Pipes.PipeDirection]::InOut,1,[IO.Pipes.PipeTransmissionMode]::Byte,[IO.Pipes.PipeOptions]::Asynchronous)
 if($i -eq 0){[Console]::Out.WriteLine('ready');[Console]::Out.Flush()}
 $waiting=$p.WaitForConnectionAsync();if(-not $waiting.Wait(15000)){exit 1}
 $h=ReadExact $p 4;$n=[BitConverter]::ToUInt32($h,0);$b=ReadExact $p $n;$q=[Text.Encoding]::UTF8.GetString($b)|ConvertFrom-Json
 if($q.name -eq 'noecho.configure'){$active=$q.payload.enabled;$apps=@($q.payload.apps)}
 $response=@{error=$null;payload=@{active=$active;running=$true;apps=$apps;voicePeak=0.5}}|ConvertTo-Json -Depth 8
 $bytes=[Text.Encoding]::UTF8.GetBytes($response);$h=[BitConverter]::GetBytes([uint32]$bytes.Length)
 $p.Write($h,0,4);$p.Write($bytes,0,$bytes.Length);$p.Flush();$p.Dispose()
}
"#;
        let mut child=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",server]).env("NOECHO_PIPE",&pipe).stdout(Stdio::piped()).creation_flags(0x08000000).spawn().unwrap();
        let mut line=String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        assert_eq!(line.trim(),"ready");
        let first=request_at(&pipe,"noecho.configure",json!({"enabled":true,"apps":["ayugram.exe"]})).unwrap();
        assert_eq!(first["apps"],json!(["ayugram.exe"]));
        let current=request_at(&pipe,"noecho.status",json!({})).unwrap();
        assert_eq!(current["active"],true);
        let updated=request_at(&pipe,"noecho.configure",json!({"enabled":true,"apps":["other.exe"]})).unwrap();
        assert_eq!(updated["apps"],json!(["other.exe"]));
        assert_eq!(request_at(&pipe,"noecho.configure",json!({"enabled":false,"apps":[]})).unwrap()["active"],false);
        let mut stream=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",CLIENT])
            .env("NOECHO_PIPE",&pipe).env("NOECHO_STREAM","1")
            .env("NOECHO_REQUEST",json!({"name":"noecho.status","payload":{}}).to_string())
            .stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().unwrap();
        let mut reader=BufReader::new(stream.stdout.take().unwrap());
        for _ in 0..2 {
            line.clear();reader.read_line(&mut line).unwrap();
            let v:Value=serde_json::from_str(&line).unwrap();assert_eq!(v["payload"]["voicePeak"],0.5);
        }
        let _=stream.kill();let _=stream.wait();
        assert!(child.wait().unwrap().success());
    }
}
