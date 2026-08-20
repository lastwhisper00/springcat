import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type WindowKind = "overlay" | "settings" | "panel-menu" | "demo";

declare global {
  interface Window {
    __SPRINGCAT_WINDOW__?: WindowKind | string;
  }
}

function fromToken(value: string | null | undefined): WindowKind | null {
  if (value === "panel-menu" || value === "settings" || value === "overlay") return value;
  if (value === "main") return "overlay";
  return null;
}

export function resolveWindowKind(): WindowKind {
  if (typeof window === "undefined") return "demo";

  const injected = fromToken(window.__SPRINGCAT_WINDOW__);
  if (injected) return injected;

  if (isTauri()) {
    try {
      const labeled = fromToken(getCurrentWindow().label);
      if (labeled) return labeled;
    } catch {
      /* window API can miss during the first paint */
    }
  }

  const params = new URLSearchParams(window.location.search);
  if (window.location.hash === "#panel-menu" || params.get("window") === "panel-menu") {
    return "panel-menu";
  }
  if (window.location.hash === "#settings" || params.get("window") === "settings") {
    return "settings";
  }
  return isTauri() ? "overlay" : "demo";
}
