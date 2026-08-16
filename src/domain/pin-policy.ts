import type { AppSettings } from "./settings";
import type { TaskItem } from "./task-item";

/** Resolve the persisted manual pin and the temporary running-task pin. */
export function shouldPinPanel(
  settings: Pick<AppSettings, "alwaysOnTop" | "autoPinWhileRunning">,
  tasks: readonly Pick<TaskItem, "status">[],
): boolean {
  return (
    settings.alwaysOnTop ||
    (settings.autoPinWhileRunning && tasks.some((task) => task.status === "running"))
  );
}
