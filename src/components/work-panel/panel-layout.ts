/** Logical-pixel layouts for the work panel. S02/S04 will consume these. */

export const PANEL_SIZE = {
  icon: { width: 48, height: 48 },
  peek: { width: 268, height: 48 },
  pinnedPeek: { width: 360, height: 48 },
  dynamicIslandPinnedPeek: { width: 520, height: 48 },
  expanded: { width: 360, height: 448 },
  dynamicIslandPinnedExpanded: { width: 520, height: 448 },
} as const;

export const DOCK_PREVIEW_PX = { min: 48, max: 72 } as const;
export const COLLAPSE_DELAY_MS = { min: 600, max: 1000 } as const;
