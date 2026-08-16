export type PresentationMode = "work" | "pet";
export type DockSide = "top" | "left" | "right";

export interface AdapterToggles {
  codex: boolean;
  cursor: boolean;
  grokCli: boolean;
  geminiCli: boolean;
  workBuddy: boolean;
}

export interface AppSettings {
  presentationMode: PresentationMode;
  dockSide: DockSide;
  dynamicIslandCompatible: boolean;
  alwaysOnTop: boolean;
  /** Temporarily pin the panel while at least one conversation is running. */
  autoPinWhileRunning: boolean;
  autostart: boolean;
  mutedUntil?: string;
  focusMode: boolean;
  /** 1 / 7 / 30 days; 0 means do not persist history. */
  historyRetentionDays: number;
  /** Absolute directory for task cache, inbox files, and logs. */
  cacheDirectory?: string;
  /** Browser executable used for external HTTP(S) links; omitted follows the OS default. */
  browserPath?: string;
  adapters: AdapterToggles;
}

export type DoubleClickAction = "open-latest" | "none";

export interface MonitorDock {
  side: DockSide;
  along: number;
}

export interface ClientSettings extends AppSettings {
  doubleClickAction: DoubleClickAction;
  monitorDocks: Record<string, MonitorDock>;
}

export const DEFAULT_SETTINGS: AppSettings = {
  presentationMode: "work",
  dockSide: "top",
  dynamicIslandCompatible: false,
  alwaysOnTop: true,
  autoPinWhileRunning: false,
  autostart: false,
  focusMode: false,
  historyRetentionDays: 7,
  adapters: {
    codex: true,
    cursor: true,
    grokCli: true,
    geminiCli: true,
    workBuddy: true,
  },
};

const PET_MODE_IMPLEMENTED = false;

function isDockSide(value: unknown): value is DockSide {
  return value === "top" || value === "left" || value === "right";
}

function isRetention(value: unknown): value is number {
  return value === 0 || value === 1 || value === 7 || value === 30;
}

/** Merge partial settings and fall back to work mode until pet mode exists. */
export function normalizeSettings(input?: Partial<AppSettings> | null): AppSettings {
  const adapters = {
    ...DEFAULT_SETTINGS.adapters,
    ...input?.adapters,
  };

  let presentationMode: PresentationMode =
    input?.presentationMode === "pet" ? "pet" : "work";
  if (presentationMode === "pet" && !PET_MODE_IMPLEMENTED) {
    presentationMode = "work";
  }

  const settings: AppSettings = {
    presentationMode,
    dockSide: isDockSide(input?.dockSide) ? input.dockSide : DEFAULT_SETTINGS.dockSide,
    dynamicIslandCompatible:
      input?.dynamicIslandCompatible ?? DEFAULT_SETTINGS.dynamicIslandCompatible,
    alwaysOnTop: input?.alwaysOnTop ?? DEFAULT_SETTINGS.alwaysOnTop,
    autoPinWhileRunning:
      input?.autoPinWhileRunning ?? DEFAULT_SETTINGS.autoPinWhileRunning,
    autostart: input?.autostart ?? DEFAULT_SETTINGS.autostart,
    focusMode: input?.focusMode ?? DEFAULT_SETTINGS.focusMode,
    historyRetentionDays: isRetention(input?.historyRetentionDays)
      ? input.historyRetentionDays
      : DEFAULT_SETTINGS.historyRetentionDays,
    adapters,
  };

  if (input?.mutedUntil) {
    settings.mutedUntil = input.mutedUntil;
  }
  const cacheDirectory = input?.cacheDirectory?.trim();
  if (cacheDirectory) {
    settings.cacheDirectory = cacheDirectory;
  }
  const browserPath = input?.browserPath?.trim();
  if (browserPath) {
    settings.browserPath = browserPath;
  }

  return settings;
}
