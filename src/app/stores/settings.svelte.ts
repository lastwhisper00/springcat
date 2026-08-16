import { DEFAULT_SETTINGS, normalizeSettings, type ClientSettings } from "$domain/settings";

let current: ClientSettings = {
  ...normalizeSettings(DEFAULT_SETTINGS),
  doubleClickAction: "open-latest",
  monitorDocks: {},
};

export const settingsStore = {
  get value(): ClientSettings {
    return current;
  },
  set value(next: ClientSettings) {
    current = next;
  },
};
