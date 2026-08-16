import { describe, expect, it } from "vitest";
import {
  dockCarousel,
  dockCarouselTasks,
  dockMotion,
  runningSources,
} from "./dock-sources";
import type { TaskItem } from "$domain";

function task(partial: Partial<TaskItem> & Pick<TaskItem, "id" | "status" | "source">): TaskItem {
  return {
    title: partial.id,
    updatedAt: "2026-08-13T04:00:00.000Z",
    unread: false,
    ...partial,
  };
}

describe("runningSources", () => {
  it("returns unique running tools in order", () => {
    expect(
      runningSources([
        task({ id: "a", status: "running", source: "codex" }),
        task({ id: "b", status: "waiting", source: "grok-cli" }),
        task({ id: "c", status: "running", source: "cursor" }),
        task({ id: "d", status: "running", source: "codex" }),
      ]),
    ).toEqual(["codex", "cursor"]);
  });
});

describe("dockCarousel", () => {
  it("falls back to the surface source when nothing is running", () => {
    const waiting = task({ id: "w", status: "waiting", source: "cursor", unread: true });
    expect(dockCarousel([waiting], { kind: "waiting", task: waiting })).toEqual(["cursor"]);
    expect(dockCarousel([], { kind: "idle" })).toEqual([]);
  });
});

describe("dockCarouselTasks", () => {
  it("keeps the surface task first and includes every running conversation", () => {
    const older = task({ id: "older", status: "running", source: "codex" });
    const latest = task({ id: "latest", status: "running", source: "cursor" });
    const sameSource = task({ id: "same-source", status: "running", source: "cursor" });

    expect(
      dockCarouselTasks([older, latest, sameSource], { kind: "working", task: latest }).map(
        (item) => item.id,
      ),
    ).toEqual(["latest", "older", "same-source"]);
  });

  it("falls back to the surface task when there is no running conversation", () => {
    const waiting = task({ id: "waiting", status: "waiting", source: "grok-cli" });
    expect(dockCarouselTasks([waiting], { kind: "waiting", task: waiting })).toEqual([waiting]);
    expect(dockCarouselTasks([], { kind: "idle" })).toEqual([]);
  });
});

describe("dockMotion", () => {
  it("maps surface kinds", () => {
    const item = task({ id: "t", status: "running", source: "codex" });
    expect(dockMotion({ kind: "idle" })).toBe("idle");
    expect(dockMotion({ kind: "working", task: item })).toBe("run");
    expect(dockMotion({ kind: "waiting", task: item })).toBe("wait");
    expect(dockMotion({ kind: "failed", task: item })).toBe("fail");
    expect(dockMotion({ kind: "completed", task: item, unread: true })).toBe("done");
  });
});
