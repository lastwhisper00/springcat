import { describe, expect, it } from "vitest";
import type { TaskItem } from "$domain";
import { reconcileTaskOrder, sortTaskIds } from "./task-order";

const task = (id: string, updatedAt = "2026-09-16T10:00:00Z"): TaskItem => ({
  id,
  source: "codex",
  title: id,
  status: "running",
  unread: false,
  updatedAt,
});

describe("task reading order", () => {
  it("starts newest first and preserves existing rows as statuses and dates update", () => {
    const older = task("older", "2026-09-16T09:00:00Z");
    const newer = task("newer");
    const initial = reconcileTaskOrder(null, [older, newer]);
    expect(initial).toEqual(["newer", "older"]);
    expect(reconcileTaskOrder(initial, [
      { ...older, status: "waiting", updatedAt: "2026-09-16T11:00:00Z" },
      newer,
      task("new", "2026-09-16T12:00:00Z"),
    ])).toEqual(["newer", "older", "new"]);
  });

  it("drops removed tasks and appends new tasks immediately", () => {
    const tasks = [task("retained"), task("new", "2026-09-16T12:00:00Z")];
    expect(reconcileTaskOrder(["removed", "retained"], tasks)).toEqual(["retained", "new"]);
    expect(sortTaskIds(tasks)).toEqual(["new", "retained"]);
  });

  it("populates a drawer that first loaded without any tasks", () => {
    expect(reconcileTaskOrder([], [task("first")])).toEqual(["first"]);
  });
  it("keeps every task accessible beyond the previous fifty-task cutoff", () => {
    const tasks = Array.from({ length: 57 }, (_, i) => task(String(i)));
    const order = reconcileTaskOrder(tasks.slice(0, 50).map((item) => item.id), tasks);
    expect(order).toEqual(tasks.map((item) => item.id));
  });

  it("orders an arriving batch by update time after the existing rows", () => {
    const existing = task("existing");
    const newest = task("newest", "2026-09-16T12:00:00Z");
    const older = task("older", "2026-09-16T11:00:00Z");
    const tasks = [older, existing, newest];
    expect(reconcileTaskOrder(["existing"], tasks)).toEqual(["existing", "newest", "older"]);
    expect(reconcileTaskOrder(null, tasks)).toEqual(["newest", "older", "existing"]);
  });

  it("removes all rows when all tasks are deleted", () => {
    expect(reconcileTaskOrder(["removed"], [])).toEqual([]);
  });
});
