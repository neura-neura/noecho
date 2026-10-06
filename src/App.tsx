import { useCallback, useEffect, useRef, useState } from "react";
import { Search, Shield } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { activateProtection, AppAudioGroup, AppConfig, AudioDevice, copyDiagnosticReport, deactivateProtection, getConfig, getSetupStatus, getStatus, getTelemetry, listAppGroups, listDevices, MixTelemetry, prepareSharedAudio, ProtectionStatus, setExcludedApps, SetupStatus, updateConfig } from "@/lib/api";

export default function App() {
  const [groups, setGroups] = useState<AppAudioGroup[]>([]);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [status, setStatus] = useState<ProtectionStatus | null>(null);
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [meters, setMeters] = useState<MixTelemetry | null>(null);
  const [meterError, setMeterError] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [options, setOptions] = useState(false);
  const [soundOpen, setSoundOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const pending = useRef(false);
  const refreshing = useRef(false);
  const generation = useRef(0);
  const es = config?.language !== "en";
  const text = (esText: string, en: string) => es ? esText : en;
  const active = !!status?.active;
  const selected = config?.excluded_apps || [];
  const refresh = useCallback(async () => {
    if (pending.current || refreshing.current) return;
    refreshing.current = true;
    const version = generation.current;
    try {
      const [g, d, c, s, ready] = await Promise.all([listAppGroups(), listDevices(), getConfig(), getStatus(), getSetupStatus()]);
      if (version !== generation.current || pending.current) return;
      setGroups(g); setDevices(d); setConfig(c); setStatus(s); setSetup(ready);
      (window as any).__noechoApplyTheme?.(c.theme);
    } catch (e) { if (version === generation.current) setError(String(e)); }
    finally { refreshing.current = false; }
  }, []);
  useEffect(() => { void refresh(); const id = window.setInterval(refresh, 2000); return () => clearInterval(id); }, [refresh]);
  useEffect(() => {
    if (!soundOpen) return;
    let stopped = false, running = false;
    const poll = async () => { if (running || pending.current) return; running = true; try { const m = await getTelemetry(); if (!stopped) { setMeters(m); setMeterError(""); } } catch (e) { if (!stopped) setMeterError(String(e)); } finally {running = false;} };
    void poll(); const id = setInterval(poll, 150); return () => {stopped = true; clearInterval(id);};
  }, [soundOpen]);
  async function action(work: () => Promise<unknown>) {
    if (pending.current) return;
    pending.current = true; generation.current++; setBusy(true); setError("");
    try { await work(); } catch (e) { setError(String(e)); }
    finally { pending.current = false; setBusy(false); await refresh(); }
  }
  async function patch(values: Partial<AppConfig>) {
    if (!config) return;
    const latest = await getConfig();
    const saved = await updateConfig({ ...latest, ...values }); setConfig(saved);
    (window as any).__noechoApplyTheme?.(saved.theme);
  }
  function toggle(group: AppAudioGroup, checked: boolean) {
    void action(async () => {
      const latest = await getConfig();
      const apps = latest.excluded_apps.filter(a => a.exe_name.toLowerCase() !== group.exe_name.toLowerCase());
      if (checked) apps.push(group.identity);
      await setExcludedApps(apps);
      setConfig({ ...latest, excluded_apps: apps });
    });
  }
  const visible = groups.filter(g => !g.is_critical && g.exe_name.toLowerCase() !== "noecho.exe" && !(status?.processor && /^unifiedaudio( engine host)?\.exe$/i.test(g.exe_name))).filter(g => (g.display_name + g.exe_name).toLowerCase().includes(search.toLowerCase()));
  const warnings = [...new Set([...(status?.warnings || []), ...(meters?.errors || [])])];
  const problem = !!error || warnings.some(w => !w.includes("Se restauró la configuración de la versión anterior"));
  const rawProblem = error || warnings.join(" · ");
  const problemDetail = es ? rawProblem : rawProblem
    .replace(/El canal remoto está ocupado por: (.*)\. Cambia su salida.*$/, "The audio channel is being used by: $1. In Settings, choose Choose automatically, then try Turn on again.")
    .replace(/La salida interna de Parsec está ocupada por: (.*)\. Cierra esa aplicación o cambia su salida\./, "The internal Parsec output is being used by: $1. Close that app or change its output.")
    .replace(/La salida interna de Parsec está ocupada por: (.*)\./, "The internal Parsec output is being used by: $1.")
    .replace(/El canal remoto es una salida predeterminada.*$/, "The NoEcho audio channel is your Windows default output. Select your speakers or headphones in Windows, then try again.")
    .replace(/Falta un canal virtual dedicado.*$/, "NoEcho needs its audio connection set up. Open Help to set up audio.")
    .replace(/El canal virtual configurado no está disponible\./, "The saved audio channel is unavailable. Choose Choose automatically in Settings.");
  const ready = !!status?.process_capture_supported && !!status?.shared_device_available && status?.remote_capture_ready !== false;
  return <div className="simple-app">
    <header className="simple-header"><h1><Shield aria-hidden="true" size={28}/>NoEcho</h1><button className="text-button" onClick={() => setHelpOpen(true)}>{text("Ayuda", "Help")}</button></header>
    <div className={`simple-status ${active ? "enabled" : ""}`} role="status" aria-live="polite">
      <strong>{busy ? text("Espera un momento…", "Please wait…") : text(active ? "Activado" : "Apagado", active ? "On" : "Off")}</strong>
      <p>{text(active ? "Puedes cambiar los programas de la lista." : "Elige los programas y pulsa Activar.", active ? "You can change the programs below." : "Choose your programs, then press Turn on.")}</p>
    </div>
    {!ready && status && <div className="simple-notice" role="status"><strong>{text("Falta preparar el audio", "Audio needs to be set up")}</strong><p>{text(status.remote_capture_ready === false ? "Cierra Parsec desde su bandeja e instala la versión nueva de NoEcho. Esto se hace una sola vez." : "Esto se hace una sola vez. Pide ayuda a quien instaló NoEcho.", status.remote_capture_ready === false ? "Quit Parsec from its tray and install the new NoEcho version. This is done once." : "This is done once. Ask the person who installed NoEcho for help.")}</p><button className="text-button" onClick={() => setHelpOpen(true)}>{text("Ver ayuda", "Get help")}</button></div>}
    {status?.processor && <p className="processor-route">{text("Tu micrófono sigue en UnifiedAudio. NoEcho filtra el audio para la conexión remota.", "Your microphone stays in UnifiedAudio. NoEcho filters audio for the remote connection.")}</p>}
    {problem && <div className="simple-notice" role="alert"><strong>{text("Hay un problema con el audio", "There is an audio problem")}</strong><p>{problemDetail}</p><button className="text-button" onClick={() => setOptions(true)}>{text("Revisar ajustes", "Check settings")}</button></div>}
    <main className="simple-list-panel">
      <div className="simple-heading"><span className="step-number">1</span><div><h2>{text("¿Qué programas no debe oír la otra persona?", "Which programs should the other person not hear?")}</h2><p>{text("Márcalos aquí. Tú podrás seguir escuchándolos.", "Check them here. You will still be able to hear them.")}</p></div></div>
      {groups.length > 8 && <label className="simple-search"><Search size={20} aria-hidden="true"/><input aria-label={text("Buscar un programa", "Find a program")} placeholder={text("Buscar un programa", "Find a program")} value={search} onChange={e => setSearch(e.target.value)}/></label>}
      <div className="simple-list">
        {!config && <p className="simple-empty">{text("Buscando tus programas…", "Finding your programs…")}</p>}
        {config && visible.length === 0 && <p className="simple-empty">{search ? text("No encontramos ese programa.", "We did not find that program.") : text("Abre un programa y reproduce algún sonido. Aparecerá aquí.", "Open a program and play a sound. It will appear here.")}</p>}
        {visible.map(g => {
          const checked = selected.some(a => a.exe_name.toLowerCase() === g.exe_name.toLowerCase());
          const name = g.display_name.replace(/\.exe$/i, "");
          return <label key={g.id} className={`simple-row ${checked ? "chosen" : ""}`}>
            <Checkbox aria-label={text("No compartir el sonido de ", "Do not share sound from ") + name} checked={checked} onCheckedChange={v => toggle(g, !!v)} disabled={busy || !config}/>
            <span className="simple-app-icon" aria-hidden="true">{g.icon_data_url ? <img src={g.icon_data_url} alt=""/> : name.slice(0,2).toUpperCase()}</span>
            <span className="simple-app-name">{name}</span>
            {checked && <span className="chosen-label">{text("Elegido", "Selected")}</span>}
          </label>;
        })}
      </div>
    </main>
    <footer className="simple-footer"><div className="simple-heading"><span className="step-number">2</span><div><h2>{text(active ? "Cuando termines, pulsa Desactivar" : "Ahora pulsa Activar", active ? "When finished, press Turn off" : "Now press Turn on")}</h2><p>{selected.length === 0 ? text("Primero marca un programa en la lista.", "First check a program in the list.") : text(`${selected.length} ${selected.length === 1 ? "programa elegido" : "programas elegidos"}`, `${selected.length} ${selected.length === 1 ? "program selected" : "programs selected"}`)}</p></div></div>
      <button className={`main-action ${active ? "stop-action" : ""}`} disabled={busy || !config || (!active && (!ready || selected.length === 0))} onClick={() => void action(() => active ? deactivateProtection() : activateProtection(selected))}>{busy ? text("Espera…", "Please wait…") : text(active ? "Desactivar" : "Activar", active ? "Turn off" : "Turn on")}</button>
    </footer>
    <nav className="simple-links" aria-label={text("Más opciones", "More options")}><button className="text-button" onClick={() => setSoundOpen(true)}>{text("Comprobar el sonido", "Check sound")}</button><button className="text-button" onClick={() => setOptions(true)}>{text("Ajustes", "Settings")}</button></nav>
    {soundOpen && <Dialog title={text("Comprobar el sonido", "Check sound")} onClose={() => { setSoundOpen(false); if (config?.monitor && config.monitor !== "none") void action(() => patch({monitor:"none"})); }} text={text}>
      <p>{text(active ? "Habla o reproduce sonido para comprobar las barras.": "Primero activa NoEcho para comprobar el sonido.", active ? "Speak or play sound to check the bars." : "Turn on NoEcho first to check sound.")}</p>
      {(meterError || (active && !!meters?.errors.length)) && <p role="alert">{meterError || meters?.errors.join(" · ")}</p>}
      <div className="simple-signals">{[
        {title:status?.processor ? text("Mi micrófono virtual", "My virtual microphone") : text("Mi micrófono", "My microphone"), source:"voice", peak:meters?.voice_peak || 0},
        {title:text("Todos los programas", "All programs"), source:"system", peak:meters?.system_peak || 0},
        {title:text("Los programas que elegí", "Programs I selected"), source:"private", peak:meters?.private_peak || 0},
        {title:text("El sonido que enviamos", "Sound we send"), source:"remote", peak:meters?.remote_peak || 0},
      ].map(v => {
        const p=meters?.processor_source;
        const detail=!active ? text("Apagado", "Off") : !meters ? text("Recibiendo niveles…", "Waiting for levels…") : meterError || !meters.running ? text("No se puede comprobar el audio", "Unable to check audio")
          : p && v.source === "voice" && p.input_peak > 0.001 && v.peak <= 0.001 ? text("El micrófono recibe sonido; revisa los efectos de UnifiedAudio", "Microphone receives sound; check UnifiedAudio effects")
          : v.peak > 0.001 ? text("Hay sonido", "Sound detected") : text("Sin sonido ahora", "No sound right now");
        return <Signal key={v.source} title={v.title} value={meters?.running ? v.peak : 0} source={v.source} config={config} active={active} busy={busy} canListen={true} detail={detail} onListen={m => void action(() => patch({monitor:m}))} text={text}/>;
      })}</div>
      {!status?.processor && <label className="simple-option"><Checkbox disabled={busy || !config} checked={!!config?.microphone_to_remote} onCheckedChange={v => void action(() => patch({microphone_to_remote:!!v}))}/><span>{text("Enviar también mi voz", "Also send my voice")}</span></label>}
      {status?.processor && <p>{text("El micrófono y la conexión remota son señales distintas. Usa auriculares para escuchar estas pruebas. Comprueba también lo que oye la otra persona.", "Your microphone and the remote connection are separate signals. Use headphones for these listening checks. Also check what the other person hears.")}</p>}
      {!status?.processor && <p>{text("Si la otra persona ya oye tu voz, deja esta casilla sin marcar.", "If the other person already hears your voice, leave this unchecked.")}</p>}
    </Dialog>}
    {helpOpen && <Dialog title={text("Cómo usar NoEcho", "How to use NoEcho")} onClose={() => setHelpOpen(false)} text={text}>
      <ol className="simple-help"><li>{text("Marca los programas que no quieres que oiga la otra persona.", "Check the programs you do not want the other person to hear.")}</li><li>{text("Pulsa Activar. Tú sigues escuchando tus programas.", "Press Turn on. You still hear your programs.")}</li><li>{text("Al terminar, pulsa Desactivar.", "When finished, press Turn off.")}</li></ol>
      <p>{text("¿No aparece un programa? Ábrelo y reproduce algún sonido.", "Missing a program? Open it and play a sound.")}</p>
      <details className="helper-details"><summary>{text("Para quien prepara el equipo", "For the person setting up this computer")}</summary><p>{text("La primera vez, configura el programa de conexión para recibir el audio de NoEcho. Si recibe el audio de Windows directamente, también oirá los programas marcados. Los pasos están en el README.", "The first time, set the connection program to receive NoEcho audio. If it receives Windows audio directly, it will also hear the checked programs. Follow the README.")}</p>
      {status?.process_capture_supported === false && <p>{text("La captura de audio requiere Windows 10 versión 2004 (build 19041) o posterior.", "Audio capture requires Windows 10 version 2004 (build 19041) or later.")}</p>}
      {setup?.can_prepare_automatically && !setup.ready && <button className="main-action" disabled={busy} onClick={() => void action(() => prepareSharedAudio())}>{text("Preparar audio", "Set up audio")}</button>}
      <p>{text("Canal de audio: ", "Audio channel: ")}{status?.shared_device_name || text("No disponible", "Not available")}</p>
      {(error || warnings.length > 0) && <pre className="technical-message">{error || warnings.join("\n")}</pre>}
      <button className="text-button" disabled={busy} onClick={() => void action(async () => navigator.clipboard.writeText(await copyDiagnosticReport()))}>{text("Copiar información para pedir ayuda", "Copy information for support")}</button></details>
    </Dialog>}
    {options && <Options config={config} devices={devices} managedRemote={status?.remote_backend === "parsec"} busy={busy} text={text} onClose={() => setOptions(false)} onPatch={v => void action(() => patch(v))} onCopy={() => void action(async () => navigator.clipboard.writeText(await copyDiagnosticReport()))}/>}
  </div>;
}
function Dialog({title,children,onClose,text}:{title:string;children:React.ReactNode;onClose:()=>void;text:(a:string,b:string)=>string}) {
 const dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{dialog.current?.showModal();},[]);
 return <dialog ref={dialog} className="simple-dialog" onCancel={onClose} onClick={e=>{if(e.target===e.currentTarget)onClose();}} aria-label={title}><header><h2>{title}</h2><button className="text-button" onClick={onClose}>{text("Cerrar", "Close")}</button></header><div className="simple-dialog-content">{children}</div></dialog>;
}

function Meter({value,label}:{value:number;label:string}) {
  const peak=Math.min(1,Math.max(0,value)); const db=peak>0 ? Math.max(-60,20*Math.log10(peak)) : -60;
  return <div className="meter" role="meter" aria-label={label} aria-valuemin={-60} aria-valuemax={0} aria-valuenow={db} aria-valuetext={`${db.toFixed(1)} dBFS`}><span style={{width:`${(db+60)/60*100}%`}}/></div>;
}
function Signal({title,value,source,config,active,busy,canListen,detail,onListen,text}:{title:string;value:number;source:string;config:AppConfig|null;active:boolean;busy:boolean;canListen:boolean;detail:string;onListen:(v:string)=>void;text:(a:string,b:string)=>string}) {
 const listening=config?.monitor===source;
 return <section className="simple-signal"><div><h3>{title}</h3>{canListen && <button className="text-button" aria-pressed={listening} disabled={!active||busy} onClick={()=>onListen(listening?"none":source)}>{text(listening?"Dejar de escuchar":"Escuchar",listening?"Stop listening":"Listen")}</button>}</div><Meter value={value} label={title}/><p>{detail}</p></section>;
}
function Options({config,devices,managedRemote,busy,text,onClose,onPatch,onCopy}:{config:AppConfig|null;devices:AudioDevice[];managedRemote:boolean;busy:boolean;text:(a:string,b:string)=>string;onClose:()=>void;onPatch:(v:Partial<AppConfig>)=>void;onCopy:()=>void}) {
 return <Dialog title={text("Ajustes", "Settings")} onClose={onClose} text={text}>
 <label className="simple-field">{text("Idioma", "Language")}<select disabled={busy} value={config?.language||"es"} onChange={e=>onPatch({language:e.target.value as AppConfig["language"]})}><option value="es">Español</option><option value="en">English</option></select></label>
 <label className="simple-field">{text("Colores de la ventana", "Window colors")}<select disabled={busy} value={config?.theme||"system"} onChange={e=>onPatch({theme:e.target.value as AppConfig["theme"]})}><option value="system">{text("Como Windows", "Same as Windows")}</option><option value="light">{text("Claros", "Light")}</option><option value="dark">{text("Oscuros", "Dark")}</option></select></label>
 <details className="helper-details"><summary>{text("Para quien prepara el equipo", "For the person setting up this computer")}</summary>
 {managedRemote ? <p>{text("La captura de Parsec se prepara automáticamente al instalar. No necesitas elegir dispositivos ni cambiar el cable del micrófono.", "Parsec capture is prepared automatically during installation. You do not need to choose devices or change your microphone cable.")}</p> : <><label className="simple-field">{text("Canal de audio para la otra persona", "Audio channel for the other person")}<select disabled={busy} value={config?.preferred_shared_device_id||""} onChange={e=>onPatch({preferred_shared_device_id:e.target.value||null})}><option value="">{text("Elegir automáticamente", "Choose automatically")}</option>{devices.filter(d=>d.is_virtual_shared_candidate).map(d=><option key={d.id} value={d.id}>{d.name}</option>)}</select></label><p>{text("Desactiva NoEcho antes de cambiar el canal.", "Turn off NoEcho before changing the channel.")}</p></>}
 <label className="simple-option"><Checkbox disabled={busy} checked={!!config?.close_to_tray} onCheckedChange={v=>onPatch({close_to_tray:!!v})}/>{text("Seguir funcionando al cerrar la ventana", "Keep running when the window closes")}</label>
 <p>{text("El control desde otra PC se explica en el README.", "Control from another PC is explained in the README.")}</p>
 <button className="text-button" disabled={busy} onClick={onCopy}>{text("Copiar información para pedir ayuda", "Copy information for support")}</button></details>
 </Dialog>;
}
