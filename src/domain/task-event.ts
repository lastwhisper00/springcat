import { SUMMARY_MAX_LENGTH } from "./constants";

export type TaskSource =
  | "codex"
  | "cursor"
  | "grok-cli"
  | "gemini-cli"
  | "workbuddy"
  | "unknown";

export type TaskEventType =
  | "task.started"
  | "task.progress"
  | "task.waiting"
  | "task.completed"
  | "task.failed"
  | "task.cancelled";

export interface TaskEvent {
  schemaVersion: 1;
  eventId: string;
  source: TaskSource;
  type: TaskEventType;
  taskId: string;
  sessionId?: string;
  parentTaskId?: string;
  projectName?: string;
  workspacePath?: string;
  title?: string;
  summary?: string;
  occurredAt: string;
  deepLink?: string;
  /** Local diagnostics only. Never shown in the work panel by default. */
  raw?: unknown;
}

const TASK_SOURCES: ReadonlySet<string> = new Set([
  "codex",
  "cursor",
  "grok-cli",
  "gemini-cli",
  "workbuddy",
  "unknown",
]);

const TASK_EVENT_TYPES: ReadonlySet<string> = new Set([
  "task.started",
  "task.progress",
  "task.waiting",
  "task.completed",
  "task.failed",
  "task.cancelled",
]);

export function isTaskSource(value: string): value is TaskSource {
  return TASK_SOURCES.has(value);
}

export function isTaskEventType(value: string): value is TaskEventType {
  return TASK_EVENT_TYPES.has(value);
}

/** Strip control characters and clamp length. Empty results become undefined. */
export function sanitizeSummary(summary: string | undefined): string | undefined {
  if (summary == null) {
    return undefined;
  }

  const cleaned = summary.replace(/[\u0000-\u001F\u007F]/g, "").trim();
  if (!cleaned) {
    return undefined;
  }

  return cleaned.length > SUMMARY_MAX_LENGTH
    ? cleaned.slice(0, SUMMARY_MAX_LENGTH)
    : cleaned;
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/**
 * Parse a loose JSON object into TaskEvent.
 * Unknown fields are ignored so future tool payloads stay compatible.
 */
export function parseTaskEvent(input: unknown): TaskEvent | null {
  if (!input || typeof input !== "object") {
    return null;
  }

  const value = input as Record<string, unknown>;
  const eventId = readString(value.eventId);
  const taskId = readString(value.taskId);
  const occurredAt = readString(value.occurredAt);
  const type = readString(value.type);
  const sourceRaw = readString(value.source) ?? "unknown";

  if (!eventId || !taskId || !occurredAt || !type || !isTaskEventType(type)) {
    return null;
  }

  const schemaVersion = value.schemaVersion === 1 ? 1 : 1;
  const source: TaskSource = isTaskSource(sourceRaw) ? sourceRaw : "unknown";

  const event: TaskEvent = {
    schemaVersion,
    eventId,
    source,
    type,
    taskId,
    occurredAt,
  };

  const sessionId = readString(value.sessionId);
  if (sessionId) event.sessionId = sessionId;
  const parentTaskId = readString(value.parentTaskId);
  if (parentTaskId) event.parentTaskId = parentTaskId;
  const projectName = readString(value.projectName);
  if (projectName) event.projectName = projectName;
  const workspacePath = readString(value.workspacePath);
  if (workspacePath) event.workspacePath = workspacePath;
  const title = readString(value.title);
  if (title) event.title = title;
  const summary = sanitizeSummary(readString(value.summary));
  if (summary) event.summary = summary;
  const deepLink = readString(value.deepLink);
  if (deepLink) event.deepLink = deepLink;
  if ("raw" in value) event.raw = value.raw;

  return event;
}
