import type { PanelLayout } from "$domain";

/** The ball is the panel's master toggle, regardless of the current open shape. */
export function orbTargetLayout(layout: PanelLayout): PanelLayout {
  return layout === "collapsed" ? "expanded" : "collapsed";
}

/** The pill toggles only the conversation drawer and never hides itself. */
export function pillTargetLayout(layout: PanelLayout): PanelLayout {
  if (layout === "collapsed") return layout;
  return layout === "expanded" ? "peek" : "expanded";
}

/** Inactivity folds only the task drawer and deliberately preserves the pill. */
export function drawerIdleTarget(layout: PanelLayout): PanelLayout {
  return layout === "expanded" ? "peek" : layout;
}

/** Keep automatic pin/task policy from undoing an explicit orb close. */
export function suppressPinnedAutoOpen(
  pinned: boolean,
  layout: PanelLayout,
  userCollapsedPinnedPill: boolean,
): boolean {
  return pinned && layout === "collapsed" && userCollapsedPinnedPill;
}
