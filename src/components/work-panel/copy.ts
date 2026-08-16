import type { DockSide, SurfaceState, TaskItem, TaskSource } from "$domain";

export const SOURCE_LABEL: Record<TaskSource, string> = {
  codex: "Codex",
  cursor: "Cursor",
  "grok-cli": "Grok",
  "gemini-cli": "Gemini CLI",
  workbuddy: "WorkBuddy",
  unknown: "未知",
};

export const STATUS_LABEL: Record<TaskItem["status"], string> = {
  running: "执行中",
  waiting: "等待确认",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export function currentTask(state: SurfaceState): TaskItem | undefined {
  return state.kind === "idle" ? undefined : state.task;
}

export function panelHeadline(state: SurfaceState): string {
  switch (state.kind) {
    case "idle":
      return "";
    case "working":
    case "waiting":
    case "failed":
      return state.task.title;
    case "completed":
      if (state.mergedCount && state.mergedCount > 1) {
        return `${state.mergedCount} 个任务已完成`;
      }
      return `${SOURCE_LABEL[state.task.source]} 已完成：${state.task.title}`;
  }
}

export function panelSummary(state: SurfaceState): string | undefined {
  switch (state.kind) {
    case "idle":
    case "working":
      return undefined;
    case "waiting":
      return state.task.summary ?? "等待你确认后继续";
    case "failed":
      return state.task.summary ?? "执行失败";
    case "completed":
      return state.task.summary;
  }
}

export function panelActionLabel(state: SurfaceState): string | undefined {
  switch (state.kind) {
    case "completed":
      return undefined;
    case "failed":
      return state.task.action?.label ?? "查看原因";
    case "waiting":
      return state.task.action?.label ?? "去处理";
    default:
      return undefined;
  }
}

export function formatDuration(task: TaskItem, now = Date.now()): string {
  const start = Date.parse(task.startedAt ?? task.updatedAt);
  const end = Date.parse(task.completedAt ?? task.updatedAt);
  const from = Number.isFinite(start) ? start : now;
  const to =
    task.status === "running" || task.status === "waiting"
      ? now
      : Number.isFinite(end)
        ? end
        : now;
  const seconds = Math.max(0, Math.round((to - from) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

export function formatClock(iso?: string): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}

export const ICON_SIZE = 44;
export const PEEK_SIZE = { width: 268, height: 48 };
export const PINNED_PEEK_SIZE = { width: 360, height: 48 };
export const DYNAMIC_ISLAND_PINNED_PEEK_SIZE = { width: 520, height: 48 };
export const EXPANDED_SIZE = { width: 360, height: 448 };
export const DYNAMIC_ISLAND_PINNED_EXPANDED_SIZE = { width: 520, height: 448 };

export function shellSize(
  _dockSide: DockSide,
  layout: "collapsed" | "peek" | "expanded",
  _sideVariant: "strip" | "card" = "strip",
  pinned = false,
  dynamicIslandCompatible = false,
): { width: number; height: number } {
  if (layout === "expanded") {
    return pinned && dynamicIslandCompatible
      ? DYNAMIC_ISLAND_PINNED_EXPANDED_SIZE
      : EXPANDED_SIZE;
  }
  if (layout === "peek") {
    if (!pinned) return PEEK_SIZE;
    return dynamicIslandCompatible ? DYNAMIC_ISLAND_PINNED_PEEK_SIZE : PINNED_PEEK_SIZE;
  }
  return { width: ICON_SIZE, height: ICON_SIZE };
}
