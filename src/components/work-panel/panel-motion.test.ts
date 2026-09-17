import { describe, expect, it } from "vitest";
import {
  closeCapsulePlan,
  edgeAlign,
  foldPlan,
  idleFrame,
  innerAlign,
  MOTION,
  orbRollDirection,
  orbSize,
  orbSurfaceSide,
  openPlan,
  resolveAlign,
  runMotionPlan,
  unfoldPlan,
} from "./panel-motion";

describe("ball lanes", () => {
  it("maps edge to the docked screen side", () => {
    expect(edgeAlign("left")).toBe("start");
    expect(edgeAlign("right")).toBe("end");
    expect(edgeAlign("top")).toBe("center");
  });

  it("keeps the open identity at the leading edge for every dock", () => {
    expect(innerAlign("left")).toBe("start");
    expect(innerAlign("right")).toBe("start");
    expect(innerAlign("top")).toBe("start");
  });

  it("keeps the left-docked orb in its leading lane while opening", () => {
    expect(resolveAlign("left", "edge")).toBe("start");
    expect(resolveAlign("left", "inner")).toBe("start");
  });

  it("preserves side anchors while sharing the same open identity lane", () => {
    expect(resolveAlign("left", "edge")).toBe("start");
    expect(resolveAlign("right", "edge")).toBe("end");
    expect(resolveAlign("left", "inner")).toBe("start");
    expect(resolveAlign("right", "inner")).toBe("start");
  });
});

describe("backing surface anchor", () => {
  it("keeps the collapsed orb on the surface anchor after a dock change", () => {
    expect(orbSurfaceSide("right", "top", "collapsed", "idle", "icon")).toBe(
      "top",
    );
  });

  it("switches to the new dock only after the opening surface is prepared", () => {
    expect(orbSurfaceSide("right", "top", "expanded", "opening", "icon")).toBe(
      "top",
    );
    expect(orbSurfaceSide("right", "right", "expanded", "opening", "icon")).toBe(
      "right",
    );
    expect(orbSurfaceSide("right", "top", "expanded", "opening", "strip")).toBe(
      "right",
    );
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

  it("keeps the orb full-size until the capsule starts revealing", () => {
    expect(orbSize("icon")).toBe(36);
    expect(orbSize("strip")).toBe(28);
    expect(orbSize("panel")).toBe(28);
  });

  it("opens peek without growing to panel", () => {
    const last = openPlan("peek").at(-1)?.frame;
    expect(last).toEqual({ stage: "strip", ball: "inner" });
  });

  it("overlaps beats instead of stopping between phases", () => {
    // The ball launches while the pill reveal is still decelerating…
    expect(openPlan("peek")[1]?.hold).toBeLessThan(MOTION.strip);
    // …and the capsule contracts while the ball is still travelling.
    expect(closeCapsulePlan()[1]?.hold).toBeLessThan(MOTION.travel);
  });

  it("keeps the readable opening within its interaction budget", () => {
    const duration = (layout: "peek" | "expanded") =>
      openPlan(layout).reduce((sum, beat) => sum + beat.hold, 0);
    expect(duration("peek")).toBeLessThan(650);
    expect(duration("expanded")).toBeLessThan(1000);
  });

  it("closes only the capsule after the drawer has folded", () => {
    const plan = closeCapsulePlan();
    expect(plan.map((beat) => beat.frame)).toEqual([
      { stage: "strip", ball: "inner" },
      { stage: "strip", ball: "edge" },
      { stage: "icon", ball: "edge" },
    ]);
    expect(plan.at(-1)?.hold).toBe(MOTION.capsule);
  });

  it("keeps a top orb centered while the pill grows, then slides it inward", () => {
    expect(resolveAlign("top", "edge")).toBe("center");
    expect(resolveAlign("top", "inner")).toBe("start");
  });

  it("rolls in the same direction as travel and reverses on close", () => {
    expect(orbRollDirection("top", "opening", "strip", "inner")).toBe(
      "counterclockwise",
    );
    expect(orbRollDirection("top", "closing", "strip", "edge")).toBe(
      "clockwise",
    );
    expect(orbRollDirection("left", "opening", "strip", "inner")).toBe(
      "none",
    );
    expect(orbRollDirection("right", "opening", "strip", "inner")).toBe(
      "counterclockwise",
    );
  });

  it("keeps the completed closing turn through the terminal icon frame", () => {
    expect(orbRollDirection("top", "closing", "icon", "edge")).toBe("clockwise");
    expect(orbRollDirection("left", "closing", "icon", "edge")).toBe(
      "none",
    );
    expect(orbRollDirection("right", "closing", "icon", "edge")).toBe("clockwise");
  });

  it("does not roll while the drawer folds or the ball is stationary", () => {
    expect(orbRollDirection("top", "folding", "strip", "inner")).toBe("none");
    expect(orbRollDirection("top", "unfolding", "strip", "inner")).toBe("none");
    expect(orbRollDirection("top", "opening", "strip", "edge")).toBe("none");
    expect(orbRollDirection("top", "opening", "panel", "inner")).toBe("none");
  });

  it("folds only the drawer and keeps the ball in its pill lane", () => {
    expect(foldPlan().map((beat) => beat.frame)).toEqual([
      { stage: "panel", ball: "inner" },
      { stage: "strip", ball: "inner" },
    ]);
    expect(foldPlan()[0]?.hold).toBe(MOTION.reveal);
    expect(foldPlan().at(-1)?.hold).toBe(MOTION.fold);
  });

  it("unfolds from a stable pill frame without moving the ball", () => {
    expect(unfoldPlan().map((beat) => beat.frame)).toEqual([
      { stage: "strip", ball: "inner" },
      { stage: "panel", ball: "inner" },
    ]);
    expect(unfoldPlan()[0]?.hold).toBe(MOTION.reveal);
    expect(unfoldPlan().at(-1)?.hold).toBe(MOTION.panel);
  });
});

describe("idleFrame", () => {
  it("matches each layout", () => {
    expect(idleFrame("collapsed")).toEqual({ stage: "icon", ball: "edge" });
    expect(idleFrame("peek")).toEqual({ stage: "strip", ball: "inner" });
    expect(idleFrame("expanded")).toEqual({ stage: "panel", ball: "inner" });
  });
});

describe("runMotionPlan", () => {
  it.each([
    { name: "capsule close", plan: closeCapsulePlan(), exitDuration: MOTION.capsule },
    { name: "drawer fold", plan: foldPlan(), exitDuration: MOTION.fold },
  ])("waits for $name to settle before allowing native teardown", async ({ plan, exitDuration }) => {
    let releaseExit!: () => void;
    let markExitStarted!: () => void;
    const exitHold = new Promise<void>((resolve) => { releaseExit = resolve; });
    const exitStarted = new Promise<void>((resolve) => { markExitStarted = resolve; });
    let finished = false;
    const run = runMotionPlan(plan, () => undefined, async (ms) => {
      if (ms === exitDuration) {
        markExitStarted();
        await exitHold;
      }
    }).then(() => { finished = true; });

    await exitStarted;
    expect(finished).toBe(false);
    releaseExit();
    await run;
    expect(finished).toBe(true);
  });

  it("applies every frame in order", async () => {
    const seen: string[] = [];
    await runMotionPlan(
      openPlan("peek"),
      (frame) => seen.push(`${frame.stage}:${frame.ball}`),
      async () => undefined,
    );
    expect(seen).toEqual(["icon:edge", "strip:edge", "strip:inner", "strip:inner"]);
  });
});
