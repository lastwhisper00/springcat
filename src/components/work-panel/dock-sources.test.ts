import { describe, expect, it } from "vitest";
import {
  dockCarousel,
  dockCarouselTasks,
  dockMotion,
  runningTasks,
  runningSources,
  selectRepresentativeSource,
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

  it("counts running tasks separately from their unique sources across the full list", () => {
    const tasks = [
      ...Array.from({ length: 5 }, (_, index) =>
        task({ id: `waiting-${index}`, status: "waiting", source: "codex" }),
      ),
      task({ id: "a", status: "running", source: "cursor" }),
      task({ id: "b", status: "running", source: "cursor" }),
      task({ id: "c", status: "running", source: "zcode" }),
    ];

    expect(runningTasks(tasks)).toHaveLength(3);
    expect(runningSources(tasks)).toEqual(["cursor", "zcode"]);
  });
});

describe("selectRepresentativeSource", () => {
  it("keeps the previous source while any of its tasks is still running", () => {
    const tasks = [
      task({ id: "first", status: "running", source: "codex" }),
      task({ id: "finished", status: "completed", source: "cursor" }),
      task({ id: "active", status: "running", source: "cursor" }),
    ];

    expect(selectRepresentativeSource(tasks, "cursor")).toBe("cursor");
    expect(selectRepresentativeSource([...tasks].reverse(), "cursor")).toBe("cursor");
  });

  it("changes to the first running source only when the previous source stops", () => {
    const tasks = [
      task({ id: "waiting", status: "waiting", source: "cursor" }),
      task({ id: "active", status: "running", source: "codex" }),
      task({ id: "also-active", status: "running", source: "zcode" }),
    ];

    expect(selectRepresentativeSource(tasks, "cursor", "cursor")).toBe("codex");
    expect(selectRepresentativeSource(tasks, null, "cursor")).toBe("codex");
  });

  it("retains the last source when all tasks have stopped, including after clearing the list", () => {
    const tasks = [
      task({ id: "finished", status: "completed", source: "cursor" }),
      task({ id: "waiting", status: "waiting", source: "codex" }),
    ];

    expect(selectRepresentativeSource(tasks, "cursor", "codex")).toBe("cursor");
    expect(selectRepresentativeSource([], "cursor")).toBe("cursor");
  });

  it("uses the surface fallback or latest task on the initial selection", () => {
    const tasks = [
      task({ id: "older", status: "completed", source: "cursor" }),
      task({
        id: "latest",
        status: "waiting",
        source: "zcode",
        updatedAt: "2026-08-13T05:00:00.000Z",
      }),
    ];

    expect(selectRepresentativeSource(tasks, null, "codex")).toBe("codex");
    expect(selectRepresentativeSource(tasks, null)).toBe("zcode");
    expect(selectRepresentativeSource([], null)).toBeNull();
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
