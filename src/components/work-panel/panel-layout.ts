import type { PanelLayout } from "$domain";

export type PanelPage = "tasks" | "widgets";

/** Logical-pixel layouts for the work panel. */

export const PANEL_SIZE = {
  icon: { width: 48, height: 48 },
  peek: { width: 268, height: 48 },
  pinnedPeek: { width: 360, height: 48 },
  dynamicIslandPinnedPeek: { width: 520, height: 48 },
  expanded: { width: 440, height: 420 },
  dynamicIslandPinnedExpanded: { width: 440, height: 420 },
  widgets: { width: 800, height: 260 },
} as const;

export function nativePanelLayout(layout: PanelLayout, pinned: boolean, page: PanelPage = "tasks"): string {
  const shape = layout === "expanded" && page === "widgets" ? "widgets" : layout;
  return pinned ? `pinned-${shape}` : shape;
}

export const DOCK_PREVIEW_PX = { min: 48, max: 72 } as const;
export const COLLAPSE_DELAY_MS = { min: 600, max: 1000 } as const;
