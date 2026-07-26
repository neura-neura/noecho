import { invoke } from "@tauri-apps/api/core";

import type { Language } from "./i18n";

export type PlaybackState = "inactive" | "active" | "expired" | "unknown";

export interface AppIdentity {
  exe_name: string;
  exe_path?: string | null;
  display_name?: string | null;
}

export interface AppAudioGroup {
  id: string;
  identity: AppIdentity;
  display_name: string;
  exe_name: string;
  exe_path?: string | null;
  icon_data_url?: string | null;
  state: PlaybackState;
  session_count: number;
  pids: number[];
  excluded: boolean;
  is_system: boolean;
  is_critical: boolean;
  volume: number;
  device_names: string[];
}

export interface AudioDevice {
  id: string;
  name: string;
  description?: string | null;
  is_default_multimedia: boolean;
  is_default_communications: boolean;
  is_virtual_shared_candidate: boolean;
  is_physical_candidate: boolean;
  state: number;
}

export interface ProtectionStatus {
  active: boolean;
  mode: "automatic" | "manual";
  message: string;
  excluded_count: number;
  excluded_apps: AppIdentity[];
  physical_device_name?: string | null;
  shared_device_name?: string | null;
  shared_device_available: boolean;
  warnings: string[];
}

export interface SetupStatus {
  state: "ready" | "needs_prepare" | "preparing" | "failed";
  ready: boolean;
  title: string;
  message: string;
  detail?: string | null;
  shared_device_name?: string | null;
  can_prepare_automatically: boolean;
  prepare_button_label: string;
}

export interface PrepareResult {
  success: boolean;
  status: SetupStatus;
  log: string[];
}

export interface AppConfig {
  theme: "system" | "light" | "dark";
  mode: "automatic" | "manual";
  start_with_windows: boolean;
  minimize_to_tray: boolean;
  close_to_tray: boolean;
  restore_on_exit: boolean;
  auto_recover_on_start: boolean;
  show_inactive_recent: boolean;
  preferred_physical_device_id?: string | null;
  preferred_shared_device_id?: string | null;
  device_change_behavior: "auto_follow" | "ask" | "keep_current";
  excluded_apps: AppIdentity[];
  language: Language;
  language_migrated: boolean;
}

export async function listAppGroups(): Promise<AppAudioGroup[]> {
  return invoke("list_app_groups");
}

export async function listDevices(): Promise<AudioDevice[]> {
  return invoke("list_devices");
}

export async function getStatus(): Promise<ProtectionStatus> {
  return invoke("get_status");
}

export async function getSetupStatus(): Promise<SetupStatus> {
  return invoke("get_setup_status");
}

export async function prepareSharedAudio(): Promise<PrepareResult> {
  return invoke("prepare_shared_audio");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function updateConfig(config: AppConfig): Promise<AppConfig> {
  return invoke("update_config", { config });
}

export async function activateProtection(apps: AppIdentity[]): Promise<ProtectionStatus> {
  return invoke("activate_protection", { apps });
}

export async function deactivateProtection(): Promise<ProtectionStatus> {
  return invoke("deactivate_protection");
}

export async function refreshRoutes(): Promise<void> {
  return invoke("refresh_routes");
}

export async function copyDiagnosticReport(): Promise<string> {
  return invoke("copy_diagnostic_report");
}

export function stateLabel(group: AppAudioGroup, language: Language): string {
  const labels = {
    now: { en: "Playing now", es: "Sonando ahora", zh: "正在播放" },
    silent: { en: "Silent", es: "En silencio", zh: "静音" },
    recent: { en: "Played recently", es: "Sonó hace poco", zh: "最近播放过" },
    unknown: { en: "Unknown", es: "Desconocido", zh: "未知" },
  } as const;
  if (group.session_count > 1 && group.state === "active") {
    return labels.now[language];
  }
  switch (group.state) {
    case "active":
      return labels.now[language];
    case "inactive":
      return labels.silent[language];
    case "expired":
      return labels.recent[language];
    default:
      return labels.unknown[language];
  }
}
