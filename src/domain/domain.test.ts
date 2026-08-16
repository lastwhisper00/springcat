import { describe, expect, it } from "vitest";
import { parseTaskEvent, sanitizeSummary } from "./task-event";
import { applyEventToTask, type TaskItem } from "./task-item";
import { deriveSurfaceState } from "./surface-state";
import { normalizeSettings } from "./settings";

function item(partial: Partial<TaskItem> & Pick<TaskItem, "id" | "status">): TaskItem {
  return {
    source: "codex",
    title: partial.title ?? partial.id,
    updatedAt: partial.updatedAt ?? "2026-08-13T04:00:00.000Z",
    unread: partial.unread ?? false,
    ...partial,
  };
}

describe("sanitizeSummary", () => {
  it("strips control characters and clamps length", () => {
    expect(sanitizeSummary("ok\u0000job")).toBe("okjob");
    expect(sanitizeSummary("   ")).toBeUndefined();
    expect(sanitizeSummary("x".repeat(200))?.length).toBe(160);
  });
});

describe("parseTaskEvent", () => {
  it("keeps known fields and ignores unknown ones", () => {
    const parsed = parseTaskEvent({
      schemaVersion: 1,
      eventId: "e1",
      source: "codex",
      type: "task.completed",
      taskId: "t1",
      title: "fix login tests",
      summary: "done",
      occurredAt: "2026-08-13T04:00:00.000Z",
      extraToolField: { nested: true },
      prompt: "should not leak",
    });

    expect(parsed).toMatchObject({
      eventId: "e1",
      source: "codex",
      type: "task.completed",
      taskId: "t1",
      title: "fix login tests",
    });
    expect(parsed && "extraToolField" in parsed).toBe(false);
    expect(parsed && "prompt" in parsed).toBe(false);
  });

  it("rejects malformed payloads", () => {
    expect(parseTaskEvent(null)).toBeNull();
    expect(parseTaskEvent({ eventId: "e1" })).toBeNull();
  });

  it("accepts WorkBuddy as a first-class task source", () => {
    expect(
      parseTaskEvent({
        schemaVersion: 1,
        eventId: "wb-1",
        source: "workbuddy",
        type: "task.completed",
        taskId: "session-1",
        occurredAt: "2026-08-14T07:00:00.000Z",
      })?.source,
    ).toBe("workbuddy");
  });

  it("accepts Gemini CLI as a first-class task source", () => {
    expect(
      parseTaskEvent({
        schemaVersion: 1,
        eventId: "gemini-1",
        source: "gemini-cli",
        type: "task.completed",
        taskId: "session-1",
        occurredAt: "2026-08-14T08:00:00.000Z",
      })?.source,
    ).toBe("gemini-cli");
  });
});

describe("applyEventToTask", () => {
  it("maps completed events to unread completed tasks", () => {
    const started = applyEventToTask(
      undefined,
      parseTaskEvent({
        schemaVersion: 1,
        eventId: "e1",
        source: "cursor",
        type: "task.started",
        taskId: "t9",
        title: "refactor",
        occurredAt: "2026-08-13T04:00:00.000Z",
      })!,
    );
    const completed = applyEventToTask(
      started,
      parseTaskEvent({
        schemaVersion: 1,
        eventId: "e2",
        source: "cursor",
        type: "task.completed",
        taskId: "t9",
        occurredAt: "2026-08-13T04:01:00.000Z",
      })!,
    );

    expect(started.status).toBe("running");
    expect(completed.status).toBe("completed");
    expect(completed.unread).toBe(true);
    expect(completed.startedAt).toBe("2026-08-13T04:00:00.000Z");
  });
});

describe("deriveSurfaceState", () => {
  it("returns idle for an empty list", () => {
    expect(deriveSurfaceState([])).toEqual({ kind: "idle" });
  });

  it("prefers a running conversation over pending historical notifications", () => {
    const state = deriveSurfaceState([
      item({ id: "a", status: "running" }),
      item({ id: "b", status: "completed", unread: true, updatedAt: "2026-08-13T05:00:00.000Z" }),
      item({ id: "c", status: "failed", updatedAt: "2026-08-13T06:00:00.000Z" }),
      item({ id: "d", status: "waiting", updatedAt: "2026-08-13T03:00:00.000Z" }),
    ]);
    expect(state.kind).toBe("working");
    if (state.kind === "working") {
      expect(state.task.id).toBe("a");
    }
  });

  it("prefers failed over completed-unread when nothing is running", () => {
    const state = deriveSurfaceState([
      item({ id: "done", status: "completed", unread: true }),
      item({ id: "err", status: "failed" }),
    ]);
    expect(state.kind).toBe("failed");
  });

  it("merges multiple unread completions", () => {
    const state = deriveSurfaceState([
      item({ id: "c1", status: "completed", unread: true, updatedAt: "2026-08-13T04:00:00.000Z" }),
      item({ id: "c2", status: "completed", unread: true, updatedAt: "2026-08-13T04:02:00.000Z" }),
      item({ id: "c3", status: "completed", unread: true, updatedAt: "2026-08-13T04:01:00.000Z" }),
    ]);
    expect(state).toMatchObject({
      kind: "completed",
      unread: true,
      mergedCount: 3,
    });
    if (state.kind === "completed") {
      expect(state.task.id).toBe("c2");
    }
  });

  it("ignores read completions and cancelled tasks when choosing working", () => {
    const state = deriveSurfaceState([
      item({ id: "old", status: "completed", unread: false }),
      item({ id: "x", status: "cancelled" }),
      item({ id: "now", status: "running", updatedAt: "2026-08-13T04:10:00.000Z" }),
    ]);
    expect(state.kind).toBe("working");
    if (state.kind === "working") {
      expect(state.task.id).toBe("now");
    }
  });
});

describe("normalizeSettings", () => {
  it("falls back to work mode while pet mode is unimplemented", () => {
    const settings = normalizeSettings({ presentationMode: "pet", dockSide: "left" });
    expect(settings.presentationMode).toBe("work");
    expect(settings.dockSide).toBe("left");
    expect(settings.dynamicIslandCompatible).toBe(false);
  });

  it("preserves dynamic island compatibility", () => {
    expect(normalizeSettings({ dynamicIslandCompatible: true }).dynamicIslandCompatible).toBe(true);
  });

  it("defaults automatic running-task pinning off for existing settings", () => {
    expect(normalizeSettings({ alwaysOnTop: false }).autoPinWhileRunning).toBe(false);
    expect(normalizeSettings({ autoPinWhileRunning: true }).autoPinWhileRunning).toBe(true);
  });

  it("trims and preserves a configured cache directory", () => {
    expect(normalizeSettings({ cacheDirectory: "  D:\\SpringCat Cache  " }).cacheDirectory).toBe(
      "D:\\SpringCat Cache",
    );
    expect(normalizeSettings({ cacheDirectory: "   " }).cacheDirectory).toBeUndefined();
  });

  it("trims the optional external browser path", () => {
    expect(normalizeSettings({ browserPath: "  C:\\Apps\\Browser.exe  " }).browserPath).toBe(
      "C:\\Apps\\Browser.exe",
    );
    expect(normalizeSettings({ browserPath: "   " }).browserPath).toBeUndefined();
  });
});
