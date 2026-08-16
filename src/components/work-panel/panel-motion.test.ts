import { describe, expect, it } from "vitest";
import {
  closePlan,
  edgeAlign,
  flowDuration,
  idleFrame,
  innerAlign,
  motionDuration,
  openPlan,
  playPlan,
  resolveAlign,
} from "./panel-motion";

describe("ball lanes", () => {
  it("maps edge to the docked screen side", () => {
    expect(edgeAlign("left")).toBe("start");
    expect(edgeAlign("right")).toBe("end");
    expect(edgeAlign("top")).toBe("start");
  });

  it("maps inner toward the desktop", () => {
    expect(innerAlign("left")).toBe("end");
    expect(innerAlign("right")).toBe("start");
    expect(innerAlign("top")).toBe("start");
  });

  it("resolves left open as start → end", () => {
    expect(resolveAlign("left", "edge")).toBe("start");
    expect(resolveAlign("left", "inner")).toBe("end");
  });

  it("mirrors the same lanes on left and right docks", () => {
    expect(resolveAlign("left", "edge")).toBe("start");
    expect(resolveAlign("right", "edge")).toBe("end");
    expect(resolveAlign("left", "inner")).toBe("end");
    expect(resolveAlign("right", "inner")).toBe("start");
  });
});

describe("plans", () => {
  it("opens expanded as icon → strip@edge → strip@inner → panel", () => {
    expect(openPlan("expanded").map((beat) => beat.frame)).toEqual([
      { stage: "icon", ball: "edge" },
      { stage: "strip", ball: "edge" },
      { stage: "strip", ball: "inner" },
      { stage: "panel", ball: "inner" },
    ]);
  });

  it("holds the icon seed before revealing the frame", () => {
    expect(openPlan("expanded")[0]?.hold).toBeGreaterThan(0);
  });

  it("opens peek without growing to panel", () => {
    const last = openPlan("peek").at(-1)?.frame;
    expect(last).toEqual({ stage: "strip", ball: "inner" });
  });

  it("closes as the reverse of open", () => {
    expect(closePlan("expanded").map((beat) => beat.frame)).toEqual([
      { stage: "panel", ball: "inner" },
      { stage: "strip", ball: "inner" },
      { stage: "strip", ball: "edge" },
      { stage: "icon", ball: "edge" },
    ]);
  });

  it("keeps open and close the same length", () => {
    expect(motionDuration(openPlan("expanded"))).toBe(flowDuration("closing", "expanded"));
  });
});

describe("idleFrame", () => {
  it("matches each layout", () => {
    expect(idleFrame("collapsed")).toEqual({ stage: "icon", ball: "edge" });
    expect(idleFrame("peek")).toEqual({ stage: "strip", ball: "inner" });
    expect(idleFrame("expanded")).toEqual({ stage: "panel", ball: "inner" });
  });
});

describe("playPlan", () => {
  it("applies every frame in order", async () => {
    const seen: string[] = [];
    await playPlan(
      openPlan("peek"),
      (frame) => seen.push(`${frame.stage}:${frame.ball}`),
      async () => undefined,
      () => false,
    );
    expect(seen).toEqual(["icon:edge", "strip:edge", "strip:inner", "strip:inner"]);
  });
});
