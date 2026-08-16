import { describe, expect, it } from "vitest";
import {
  decideNotification,
  didFinishLastRunning,
  isMuted,
  notificationRestingLayout,
} from "./notification-policy";
import type { SurfaceState, TaskItem } from "./index";

const task: TaskItem = {
  id: "t1",
  source: "codex",
  title: "fix",
  status: "completed",
  updatedAt: "2026-08-13T04:00:00.000Z",
  unread: true,
};

const completed: SurfaceState = { kind: "completed", task, unread: true };
const waiting: SurfaceState = { kind: "waiting", task: { ...task, status: "waiting" } };
const working: SurfaceState = { kind: "working", task: { ...task, status: "running", unread: false } };

describe("decideNotification", () => {
  it("keeps the current conversation visible while a task is running", () => {
    expect(decideNotification(working)).toEqual({ layout: "peek", autoHideMs: null, peek: true });
  });

  it("keeps waiting visible", () => {
    expect(decideNotification(waiting).autoHideMs).toBeNull();
    expect(decideNotification(waiting).peek).toBe(true);
  });

  it("hides completed after 5 seconds", () => {
    expect(decideNotification(completed)).toEqual({ layout: "peek", autoHideMs: 5000, peek: true });
  });

  it("dismisses transient completion peeks all the way to the orb", () => {
    expect(notificationRestingLayout(decideNotification(completed))).toBe("collapsed");
    expect(notificationRestingLayout(decideNotification(working))).toBe("peek");
    expect(notificationRestingLayout(decideNotification(waiting))).toBe("peek");
  });

  it("focus mode suppresses completed peeks", () => {
    expect(decideNotification(completed, { focusMode: true }).peek).toBe(false);
  });

  it("mute suppresses every peek", () => {
    expect(decideNotification(waiting, { muted: true }).peek).toBe(false);
  });
});

describe("didFinishLastRunning", () => {
  it("detects when the final running task reaches a terminal state", () => {
    expect(
      didFinishLastRunning(
        [{ status: "running" }, { status: "completed" }],
        [{ status: "completed" }, { status: "completed" }],
      ),
    ).toBe(true);
  });

  it("stays false while any task is still running", () => {
    expect(
      didFinishLastRunning(
        [{ status: "running" }, { status: "running" }],
        [{ status: "completed" }, { status: "running" }],
      ),
    ).toBe(false);
  });
});

describe("isMuted", () => {
  it("expires in the past", () => {
    expect(isMuted({ mutedUntil: "2020-01-01T00:00:00.000Z" })).toBe(false);
    expect(isMuted({ mutedUntil: "2099-01-01T00:00:00.000Z" })).toBe(true);
  });
});
