import type { TaskEvent, TaskEventType, TaskSource } from "./task-event";
import { sanitizeSummary } from "./task-event";

export type TaskStatus =
  | "running"
  | "waiting"
  | "completed"
  | "failed"
  | "cancelled";

export interface TaskAction {
  label: string;
  deepLink?: string;
}

export interface TaskItem {
  id: string;
  source: TaskSource;
  title: string;
  summary?: string;
  status: TaskStatus;
  startedAt?: string;
  updatedAt: string;
  completedAt?: string;
  unread: boolean;
  action?: TaskAction;
}

const EVENT_TO_STATUS: Record<TaskEventType, TaskStatus> = {
  "task.started": "running",
  "task.progress": "running",
  "task.waiting": "waiting",
  "task.completed": "completed",
  "task.failed": "failed",
  "task.cancelled": "cancelled",
};

function defaultTitle(event: TaskEvent): string {
  return event.title?.trim() || "未命名任务";
}

function actionFor(event: TaskEvent, status: TaskStatus): TaskAction | undefined {
  if (!event.deepLink && status === "running") {
    return undefined;
  }

  if (status === "completed") {
    return { label: "查看结果", deepLink: event.deepLink };
  }
  if (status === "failed") {
    return { label: "查看原因", deepLink: event.deepLink };
  }
  if (status === "waiting") {
    return { label: "去处理", deepLink: event.deepLink };
  }
  if (event.deepLink) {
    return { label: "打开来源", deepLink: event.deepLink };
  }
  return undefined;
}

/** Fold a normalized event into a task record. Domain only — no UI fields. */
export function applyEventToTask(
  existing: TaskItem | undefined,
  event: TaskEvent,
): TaskItem {
  const status = EVENT_TO_STATUS[event.type];
  const title = event.title?.trim() || existing?.title || defaultTitle(event);
  const summary = sanitizeSummary(event.summary) ?? existing?.summary;
  const startedAt =
    event.type === "task.started"
      ? event.occurredAt
      : existing?.startedAt ?? (status === "running" ? event.occurredAt : undefined);
  const completedAt =
    status === "completed" || status === "failed" || status === "cancelled"
      ? event.occurredAt
      : undefined;
  const unread = status === "completed" || status === "failed" || status === "waiting";
  const action = actionFor(event, status) ?? existing?.action;

  const item: TaskItem = {
    id: event.taskId,
    source: event.source,
    title,
    status,
    updatedAt: event.occurredAt,
    unread,
  };

  if (summary) item.summary = summary;
  if (startedAt) item.startedAt = startedAt;
  if (completedAt) item.completedAt = completedAt;
  if (action) item.action = action;

  return item;
}
