import type { PanelLayout, TaskItem } from "$domain";

/** Identity/status changes are fresh notification policy; title churn is not. */
export function taskPolicyKey(
  tasks: readonly Pick<TaskItem, "id" | "status">[],
): string {
  return tasks
    .map((task) => `${task.id}\u0000${task.status}`)
    .sort()
    .join("\u0001");
}

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

/** Keep task policy from immediately undoing an explicit orb close. */
export function suppressUserCollapsedAutoOpen(
  layout: PanelLayout,
  userCollapsedPill: boolean,
): boolean {
  return layout === "collapsed" && userCollapsedPill;
}
