import type { SurfaceState } from "./surface-state";
import type { AppSettings } from "./settings";
import type { TaskItem } from "./task-item";

export type PanelLayout = "collapsed" | "peek" | "expanded";

export interface NotificationDecision {
  layout: PanelLayout;
  autoHideMs: number | null;
  peek: boolean;
}

/**
 * The layout to return to when the user dismisses an expanded task list.
 * Auto-hiding notifications are transient, so unread completion state must
 * not pin the panel open as a peek forever.
 */
export function notificationRestingLayout(decision: NotificationDecision): PanelLayout {
  return decision.peek && decision.autoHideMs === null ? "peek" : "collapsed";
}

/** True when an update removes the final running task from the snapshot. */
export function didFinishLastRunning(
  previous: readonly Pick<TaskItem, "status">[],
  next: readonly Pick<TaskItem, "status">[],
): boolean {
  return (
    previous.some((task) => task.status === "running") &&
    !next.some((task) => task.status === "running")
  );
}

export function isMuted(settings: Pick<AppSettings, "mutedUntil">, now = Date.now()): boolean {
  if (!settings.mutedUntil) return false;
  const until = Date.parse(settings.mutedUntil);
  return Number.isFinite(until) && until > now;
}

/**
 * Environment-style notifications.
 * working/waiting/failed: peek and stay. completed: peek ~5s.
 */
export function decideNotification(
  state: SurfaceState,
  options: { muted?: boolean; focusMode?: boolean } = {},
): NotificationDecision {
  if (options.muted) {
    return { layout: "collapsed", autoHideMs: null, peek: false };
  }

  switch (state.kind) {
    case "waiting":
      return { layout: "peek", autoHideMs: null, peek: true };
    case "failed":
      return { layout: "peek", autoHideMs: null, peek: true };
    case "completed":
      if (options.focusMode) {
        return { layout: "collapsed", autoHideMs: null, peek: false };
      }
      return { layout: "peek", autoHideMs: 5000, peek: true };
    case "working":
      return { layout: "peek", autoHideMs: null, peek: true };
    case "idle":
      return { layout: "collapsed", autoHideMs: null, peek: false };
  }
}
