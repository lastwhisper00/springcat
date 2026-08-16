import type { SurfaceState, TaskItem, TaskSource } from "$domain";

export type DockMotion = "idle" | "run" | "wait" | "fail" | "done";

export function runningTasks(tasks: TaskItem[]): TaskItem[] {
  return tasks.filter((task) => task.status === "running");
}

export function runningSources(tasks: TaskItem[]): TaskSource[] {
  const seen = new Set<TaskSource>();
  const sources: TaskSource[] = [];
  for (const task of runningTasks(tasks)) {
    if (seen.has(task.source)) continue;
    seen.add(task.source);
    sources.push(task.source);
  }
  return sources;
}

export function dockMotion(state: SurfaceState): DockMotion {
  switch (state.kind) {
    case "working":
      return "run";
    case "waiting":
      return "wait";
    case "failed":
      return "fail";
    case "completed":
      return state.unread ? "done" : "idle";
    default:
      return "idle";
  }
}

export function dockCarousel(tasks: TaskItem[], state: SurfaceState): TaskSource[] {
  const running = runningSources(tasks);
  if (running.length > 0) return running;
  if (state.kind === "idle") return [];
  return [state.task.source];
}

export function dockCarouselTasks(tasks: TaskItem[], state: SurfaceState): TaskItem[] {
  const running = runningTasks(tasks);
  if (running.length === 0) return state.kind === "idle" ? [] : [state.task];
  if (state.kind !== "working") return running;

  return [state.task, ...running.filter((task) => task.id !== state.task.id)];
}
