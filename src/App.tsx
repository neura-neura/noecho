import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CheckCheck,
  ChevronDown,
  Copy,
  HelpCircle,
  RefreshCw,
  Shield,
  ShieldOff,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  activateProtection,
  AppAudioGroup,
  AppConfig,
  AppIdentity,
  AudioDevice,
  copyDiagnosticReport,
  deactivateProtection,
  getConfig,
  getSetupStatus,
  getStatus,
  listAppGroups,
  listDevices,
  prepareSharedAudio,
  ProtectionStatus,
  SetupStatus,
  stateLabel,
  updateConfig,
} from "@/lib/api";
import { isLanguage, Language, t } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export default function App() {
  const [groups, setGroups] = useState<AppAudioGroup[]>([]);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [status, setStatus] = useState<ProtectionStatus | null>(null);
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(false);
  const [preparing, setPreparing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);

  const language: Language = isLanguage(config?.language) ? config.language : "en";
  const tx = (key: Parameters<typeof t>[1], variables?: Record<string, string | number>) =>
    t(language, key, variables);

  const refresh = useCallback(async () => {
    try {
      const [g, d, s, c, setupStatus] = await Promise.all([
        listAppGroups(),
        listDevices(),
        getStatus(),
        getConfig(),
        getSetupStatus(),
      ]);
      setGroups(g.filter((x) => !x.is_critical || x.excluded));
      setDevices(d);
      setStatus(s);
      setSetup(setupStatus);
      setConfig(c);
      setSelected((prev) => {
        const next = { ...prev };
        for (const app of c.excluded_apps) {
          next[app.exe_name.toLowerCase()] = true;
        }
        for (const group of g) {
          if (group.excluded) next[group.exe_name.toLowerCase()] = true;
        }
        return next;
      });
      if ((window as any).__noechoApplyTheme) {
        (window as any).__noechoApplyTheme(c.theme);
      }
      setError(null);
    } catch (e: any) {
      setError(friendlyError(e?.message || String(e), language));
    }
  }, [language]);

  useEffect(() => {
    refresh();
    const id = window.setInterval(refresh, 2500);
    return () => window.clearInterval(id);
  }, [refresh]);

  const selectedApps: AppIdentity[] = useMemo(() => {
    return groups
      .filter((g) => selected[g.exe_name.toLowerCase()] && !g.is_critical)
      .map((g) => ({
        exe_name: g.exe_name.toLowerCase(),
        exe_path: g.exe_path,
        display_name: g.display_name,
      }));
  }, [groups, selected]);

  const selectedCount = selectedApps.length;
  const active = !!status?.active;
  const needsSetup = setup ? !setup.ready : status ? !status.shared_device_available : false;

  async function onProtect() {
    if (selectedCount === 0) {
      setError(tx("selectOneError"));
      return;
    }
    setLoading(true);
    setError(null);
    setInfo(null);
    try {
      const s = await activateProtection(selectedApps);
      setStatus(s);
      setInfo(tx(selectedCount === 1 ? "protectionReadyOne" : "protectionReadyMany"));
      if (s.warnings?.length) {
        setInfo(s.warnings.map((warning) => friendlyError(warning, language)).join(" "));
      }
      await refresh();
    } catch (e: any) {
      setError(friendlyError(e?.message || String(e), language));
      await refresh();
    } finally {
      setLoading(false);
    }
  }

  async function onRestore() {
    setLoading(true);
    setError(null);
    setInfo(null);
    try {
      const s = await deactivateProtection();
      setStatus(s);
      setInfo(tx("restoreDone"));
      await refresh();
    } catch (e: any) {
      setError(friendlyError(e?.message || String(e), language));
    } finally {
      setLoading(false);
    }
  }

  async function onPrepare() {
    setPreparing(true);
    setError(null);
    setInfo(null);
    try {
      const result = await prepareSharedAudio();
      setSetup(result.status);
      if (result.success) {
        setInfo(tx("prepareDone"));
        await refresh();
      } else {
        setError(tx("errorShared"));
      }
    } catch (e: any) {
      setError(friendlyError(e?.message || String(e), language));
    } finally {
      setPreparing(false);
    }
  }

  function toggle(exe: string, value: boolean) {
    setSelected((prev) => ({ ...prev, [exe.toLowerCase()]: value }));
  }

  async function saveConfigPatch(patch: Partial<AppConfig>) {
    if (!config) return;
    const next = { ...config, ...patch };
    const saved = await updateConfig(next);
    setConfig(saved);
    if (patch.theme && (window as any).__noechoApplyTheme) {
      (window as any).__noechoApplyTheme(patch.theme);
    }
  }

  async function onCopyReport() {
    try {
      const report = await copyDiagnosticReport();
      await navigator.clipboard.writeText(report);
      setInfo(tx("reportCopied"));
    } catch (e: any) {
      setError(friendlyError(e?.message || String(e), language));
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto p-4">
      <header className="mb-3 flex items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-md border">
              <Shield className="h-4 w-4" />
            </div>
            <div>
              <h1 className="text-base font-semibold tracking-tight">NoEcho</h1>
              <p className="text-xs text-muted-foreground">{tx("privateAudio")}</p>
            </div>
          </div>
          <p className="mt-2 max-w-[22rem] text-sm text-muted-foreground">{tx("intro")}</p>
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setHelpOpen((v) => !v)}
            title={tx("quickHelp")}
          >
            <HelpCircle className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" onClick={() => refresh()} title={tx("refresh")}>
            <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
          </Button>
        </div>
      </header>

      {helpOpen && (
        <div className="mb-3 rounded-lg border bg-card p-3 text-xs leading-relaxed text-muted-foreground">
          <p className="font-medium text-foreground">{tx("helpTitle")}</p>
          <ol className="mt-2 list-decimal space-y-1 pl-4">
            <li>{tx("help1")}</li>
            <li>{tx("help2")}</li>
            <li>{tx("help3")}</li>
          </ol>
          <p className="mt-2">{tx("helpOutro")}</p>
        </div>
      )}

      {needsSetup && (
        <div className="mb-3 rounded-lg border p-3 text-xs leading-relaxed">
          <p className="font-medium">{tx("setupTitle")}</p>
          <p className="mt-1 text-muted-foreground">{tx("setupMessage")}</p>
          {setup?.detail && <p className="mt-1 text-[11px] text-muted-foreground">{setup.detail}</p>}
          {(setup?.can_prepare_automatically ?? false) && (
            <div className="mt-2">
              <Button size="sm" onClick={onPrepare} disabled={preparing}>
                {preparing ? tx("preparing") : tx("prepareOptional")}
              </Button>
            </div>
          )}
          <p className="mt-2 text-[11px] text-muted-foreground">{tx("setupOptional")}</p>
        </div>
      )}

      <div className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
        <span>
          {groups.length === 0
            ? tx("appsWithSoundNone")
            : tx("appsWithSound", { count: groups.length, plural: groups.length === 1 ? "" : "s" })}
        </span>
        <span>
          {selectedCount === 0
            ? tx("noneSelected")
            : tx(selectedCount === 1 ? "selectedOne" : "selectedMany", { count: selectedCount })}
        </span>
      </div>

      <div className="min-h-0 flex-1 rounded-lg border bg-card">
        <ScrollArea className="h-full">
          <div className="divide-y">
            {groups.length === 0 && (
              <div className="p-6 text-center text-sm text-muted-foreground">{tx("noApps")}</div>
            )}
            {groups.map((group) => (
              <AppRow
                key={group.id}
                group={group}
                language={language}
                checked={!!selected[group.exe_name.toLowerCase()]}
                onCheckedChange={(v) => toggle(group.exe_name, v)}
                disabled={group.is_critical || loading}
              />
            ))}
          </div>
        </ScrollArea>
      </div>

      <div className="mt-3 rounded-lg border p-3">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "h-2 w-2 rounded-full",
              active ? "bg-foreground" : "bg-muted-foreground/40"
            )}
          />
          <div className="text-sm font-medium">
            {active ? tx("activeProtection") : tx("noProtection")}
          </div>
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          {active ? tx("activeDescription") : tx("inactiveDescription")}
        </p>
      </div>

      {(error || info) && (
        <div className="mt-2 rounded-md border px-3 py-2 text-xs leading-relaxed">{error || info}</div>
      )}

      <div className="mt-3 grid grid-cols-1 gap-2">
        {!active ? (
          <Button onClick={onProtect} disabled={loading || selectedCount === 0 || needsSetup}>
            <CheckCheck className="h-4 w-4" />
            {selectedCount > 0
              ? tx("hideRemoteCount", { count: selectedCount })
              : tx("hideRemote")}
          </Button>
        ) : (
          <Button variant="secondary" onClick={onRestore} disabled={loading}>
            <ShieldOff className="h-4 w-4" />
            {tx("restoreNormal")}
          </Button>
        )}
      </div>

      <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen} className="mt-3">
        <CollapsibleTrigger asChild>
          <button className="flex w-full items-center justify-between rounded-md border px-3 py-2 text-left text-xs text-muted-foreground hover:bg-accent">
            <span>{tx("advancedOptions")}</span>
            <ChevronDown
              className={cn("h-4 w-4 transition-transform", advancedOpen && "rotate-180")}
            />
          </button>
        </CollapsibleTrigger>
        <CollapsibleContent className="mt-2 max-h-[260px] space-y-3 overflow-y-auto overscroll-contain rounded-md border p-3 pr-2 text-xs">
          <p className="text-muted-foreground">{tx("optionsHint")}</p>

          <div className="space-y-1">
            <label className="text-muted-foreground">{tx("language")}</label>
            <select
              className="w-full rounded-md border bg-background px-2 py-1.5"
              value={language}
              onChange={(e) => {
                if (isLanguage(e.target.value)) void saveConfigPatch({ language: e.target.value });
              }}
            >
              <option value="en">English</option>
              <option value="es">Español</option>
              <option value="zh">中文</option>
            </select>
          </div>

          <div className="space-y-1">
            <label className="text-muted-foreground">{tx("headphones")}</label>
            <select
              className="w-full rounded-md border bg-background px-2 py-1.5"
              value={config?.preferred_physical_device_id || ""}
              onChange={(e) =>
                void saveConfigPatch({ preferred_physical_device_id: e.target.value || null })
              }
            >
              <option value="">{tx("automatic")}</option>
              {devices
                .filter((d) => d.is_physical_candidate)
                .map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
            </select>
          </div>

          <div className="space-y-1">
            <label className="text-muted-foreground">{tx("remoteChannel")}</label>
            <select
              className="w-full rounded-md border bg-background px-2 py-1.5"
              value={config?.preferred_shared_device_id || ""}
              onChange={(e) =>
                void saveConfigPatch({ preferred_shared_device_id: e.target.value || null })
              }
            >
              <option value="">{tx("automatic")}</option>
              {devices
                .filter((d) => d.is_virtual_shared_candidate)
                .map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
            </select>
          </div>

          <div className="space-y-1">
            <label className="text-muted-foreground">{tx("appearance")}</label>
            <select
              className="w-full rounded-md border bg-background px-2 py-1.5"
              value={config?.theme || "system"}
              onChange={(e) =>
                void saveConfigPatch({ theme: e.target.value as AppConfig["theme"] })
              }
            >
              <option value="system">{tx("sameWindows")}</option>
              <option value="light">{tx("light")}</option>
              <option value="dark">{tx("dark")}</option>
            </select>
          </div>

          <label className="flex items-center gap-2">
            <Checkbox
              checked={!!config?.show_inactive_recent}
              onCheckedChange={(v) => void saveConfigPatch({ show_inactive_recent: !!v })}
            />
            {tx("showRecent")}
          </label>

          <label className="flex items-center gap-2">
            <Checkbox
              checked={!!config?.close_to_tray}
              onCheckedChange={(v) => void saveConfigPatch({ close_to_tray: !!v })}
            />
            {tx("closeTray")}
          </label>

          <Button variant="outline" size="sm" className="w-full" onClick={onCopyReport}>
            <Copy className="h-3.5 w-3.5" />
            {tx("supportCopy")}
          </Button>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

function AppRow({
  group,
  language,
  checked,
  onCheckedChange,
  disabled,
}: {
  group: AppAudioGroup;
  language: Language;
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label
      className={cn(
        "flex cursor-pointer items-center gap-3 px-3 py-2.5 hover:bg-accent/50",
        disabled && "cursor-not-allowed opacity-50"
      )}
    >
      <Checkbox
        checked={checked}
        onCheckedChange={(v) => onCheckedChange(!!v)}
        disabled={disabled}
      />
      <div className="flex h-8 w-8 items-center justify-center overflow-hidden rounded-md border bg-background">
        {group.icon_data_url ? (
          <img src={group.icon_data_url} alt="" className="h-5 w-5" draggable={false} />
        ) : (
          <span className="text-[10px] text-muted-foreground">
            {group.display_name.slice(0, 2).toUpperCase()}
          </span>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-2">
          <div className="truncate text-sm font-medium">{group.display_name}</div>
          <div className="shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground">
            {group.excluded || checked ? t(language, "statePrivate") : t(language, "stateRemote")}
          </div>
        </div>
        <div className="truncate text-xs text-muted-foreground">{group.exe_name}</div>
        <div className="text-[11px] text-muted-foreground">{stateLabel(group, language)}</div>
      </div>
    </label>
  );
}

function friendlyError(raw: string, language: Language): string {
  const text = raw.replace(/^Error:\s*/i, "").trim();
  if (/shared|virtual|cable|audio compartido|dispositivo virtual|paquete|prepar/i.test(text)) {
    return t(language, "errorShared");
  }
  if (/policyconfig|predeterminado|default/i.test(text)) {
    return t(language, "errorPolicy");
  }
  if (/selecciona|elige|al menos|marca/i.test(text)) {
    return t(language, "errorSelect");
  }
  if (/already active|ya.*activa/i.test(text)) {
    return t(language, "errorActive");
  }
  return text.length > 180 ? `${text.slice(0, 180)}...` : text;
}
