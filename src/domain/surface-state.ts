import type { TaskItem } from "./task-item";

export type SurfaceState =
  | { kind: "idle" }
  | { kind: "working"; task: TaskItem }
  | { kind: "waiting"; task: TaskItem }
  | { kind: "failed"; task: TaskItem }
  | {
      kind: "completed";
      task: TaskItem;
      unread: boolean;
      mergedCount?: number;
    };

function byRecency(a: TaskItem, b: TaskItem): number {
  const time = b.updatedAt.localeCompare(a.updatedAt);
  return time !== 0 ? time : b.id.localeCompare(a.id);
}

function latest(tasks: TaskItem[]): TaskItem {
  return [...tasks].sort(byRecency)[0];
}

/**
 * Pure function: one surface state from all known tasks.
 * Keep in sync with src-tauri/src/domain/surface_state.rs
 *
 * Priority: running > waiting > failed > completed-unread > idle
 *
 * An active conversation must stay visible even when older completions are
 * still unread, otherwise progress events make the panel alternate between
 * the current title and an aggregate completion notification.
 */
export function deriveSurfaceState(tasks: TaskItem[]): SurfaceState {
  const running = tasks.filter((task) => task.status === "running");
  if (running.length > 0) {
    return { kind: "working", task: latest(running) };
  }

  const waiting = tasks.filter((task) => task.status === "waiting");
  if (waiting.length > 0) {
    return { kind: "waiting", task: latest(waiting) };
  }

  const failed = tasks.filter((task) => task.status === "failed");
  if (failed.length > 0) {
    return { kind: "failed", task: latest(failed) };
  }

  const completedUnread = tasks.filter(
    (task) => task.status === "completed" && task.unread,
  );
  if (completedUnread.length === 1) {
    return { kind: "completed", task: completedUnread[0], unread: true };
  }
  if (completedUnread.length > 1) {
    return {
      kind: "completed",
      task: latest(completedUnread),
      unread: true,
      mergedCount: completedUnread.length,
    };
  }

  return { kind: "idle" };
}
