/**
 * Thin IPC helpers. Keep UI free of raw `invoke` strings outside this folder.
 */
import { invoke } from "@tauri-apps/api/core";
import type { DailyUsage, DockSide, TaskItem } from "$domain";
import type { ClientSettings } from "$domain/settings";

export interface AppMeta {
  name: string;
  displayName: string;
  version: string;
  presentationMode: "work" | "pet";
  dataDirName: string;
}

export interface DockChanged {
  side: DockSide;
  along: number;
  preview: boolean;
  x: number;
  y: number;
}

export function getAppMeta(): Promise<AppMeta> {
  return invoke<AppMeta>("app_meta");
}

export function resizePanel(width: number, height: number): Promise<void> {
  return invoke("resize_panel", { width, height });
}

export function movePanel(x: number, y: number): Promise<void> {
  return invoke("move_panel", { x, y });
}

export function placeMainWindow(): Promise<void> {
  return invoke("place_main_window");
}

export function applyPanelLayout(
  layout: "collapsed" | "peek" | "expanded",
  position?: { x: number; y: number } | null,
  pinned = false,
  dynamicIslandCompatible?: boolean,
): Promise<void> {
  return invoke("apply_panel_layout", {
    layout: pinned ? `pinned-${layout}` : layout,
    x: position?.x,
    y: position?.y,
    dynamicIslandCompatible,
  });
}

export function preparePanelLayout(
  layout: "collapsed" | "peek" | "expanded",
  position?: { x: number; y: number } | null,
  pinned = false,
  dynamicIslandCompatible?: boolean,
): Promise<void> {
  return invoke("prepare_panel_layout", {
    layout: pinned ? `pinned-${layout}` : layout,
    x: position?.x,
    y: position?.y,
    dynamicIslandCompatible,
  });
}

export function resizePinnedPanel(width: number, height: number): Promise<DockChanged> {
  return invoke("resize_pinned_panel", { width, height });
}

/** Resize one animation frame without snapping to a discrete panel layout. */
export function resizePanelFrame(
  width: number,
  height: number,
  pinned: boolean,
): Promise<DockChanged> {
  return invoke("resize_panel_frame", { width, height, pinned });
}

export function dockAfterDrag(position?: { x: number; y: number } | null): Promise<DockChanged> {
  return invoke("dock_after_drag", {
    x: position?.x,
    y: position?.y,
  });
}

export function previewDock(): Promise<DockSide | null> {
  return invoke("preview_dock");
}

export function topPinTarget(
  layout: "collapsed" | "peek" | "expanded",
  dynamicIslandCompatible?: boolean,
): Promise<DockChanged> {
  return invoke("top_pin_target", {
    layout: `pinned-${layout}`,
    dynamicIslandCompatible,
  });
}

export function setPanelPinned(pinned: boolean): Promise<void> {
  return invoke("set_panel_pinned", { pinned });
}

export function getSettings(): Promise<ClientSettings> {
  return invoke("get_settings");
}

export function updateSettings(patch: Partial<ClientSettings> & { adapters?: ClientSettings["adapters"] }): Promise<ClientSettings> {
  return invoke("update_settings", { patch });
}

export interface BrowserOption {
  name: string;
  path: string;
}

export interface BrowserInfo {
  systemDefaultName: string;
  systemDefaultPath?: string;
  browsers: BrowserOption[];
}

export function getBrowserInfo(): Promise<BrowserInfo> {
  return invoke("browser_info");
}

export interface StorageInfo {
  defaultDirectory: string;
  activeDirectory: string;
  configuredDirectory?: string;
  restartRequired: boolean;
}

export function getStorageInfo(): Promise<StorageInfo> {
  return invoke("storage_info");
}

export function listTasks(): Promise<TaskItem[]> {
  return invoke("list_tasks");
}

export function listUsageMonth(month: string): Promise<DailyUsage[]> {
  return invoke("list_usage_month", { month });
}

export function saveUsageShareImage(fileName: string, bytes: number[]): Promise<string> {
  return invoke("save_usage_share_image", { fileName, bytes });
}

export function markRead(taskId: string): Promise<void> {
  return invoke("mark_read", { taskId });
}

export function markAllRead(): Promise<void> {
  return invoke("mark_all_read");
}

export function openTask(taskId: string): Promise<void> {
  return invoke("open_task", { taskId });
}

export function openLatest(): Promise<void> {
  return invoke("open_latest");
}

export function muteHour(): Promise<ClientSettings> {
  return invoke("mute_hour");
}

export function setFocus(enabled: boolean): Promise<ClientSettings> {
  return invoke("set_focus", { enabled });
}

export function openSettings(): Promise<void> {
  return invoke("open_settings");
}

export function popupPanelMenu(): Promise<void> {
  return invoke("popup_panel_menu");
}

export type PanelMenuAction =
  | "view-tasks"
  | "mute"
  | "focus"
  | "dynamic-island"
  | "pin"
  | "settings"
  | "quit";

export function runPanelMenuAction(action: PanelMenuAction): Promise<void> {
  return invoke("panel_menu_action", { action });
}

export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

export function restartApp(): Promise<void> {
  return invoke("restart_app");
}

export type AdapterSource = "codex" | "cursor" | "grok-cli" | "gemini-cli" | "workbuddy" | "marvis";

export interface AdapterBindInfo {
  inboxDir: string;
  bridgePath: string;
  bridgeFound: boolean;
}

export interface AdapterTestResult {
  ok: boolean;
  viaBridge: boolean;
  message: string;
}

export interface AdapterInstallStatus {
  source: AdapterSource;
  installed: boolean;
  configPath: string;
  bridgeInstalled: boolean;
  requiresTrust: boolean;
  message: string;
}

export function adapterBindInfo(): Promise<AdapterBindInfo> {
  return invoke("adapter_bind_info");
}

export function adapterInstallStatus(source: AdapterSource): Promise<AdapterInstallStatus> {
  return invoke("adapter_install_status", { source });
}

export function installAdapter(source: AdapterSource): Promise<AdapterInstallStatus> {
  return invoke("install_adapter", { source });
}

export function uninstallAdapter(source: AdapterSource): Promise<AdapterInstallStatus> {
  return invoke("uninstall_adapter", { source });
}

export function emitAdapterTest(source: AdapterSource): Promise<AdapterTestResult> {
  return invoke("emit_adapter_test", { source });
}
