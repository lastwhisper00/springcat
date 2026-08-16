import { describe, expect, it } from "vitest";
import { deriveSurfaceState } from "$domain";
import { tasksForKind } from "$app/demo/fixtures";
import { panelActionLabel, panelHeadline, panelSummary, shellSize } from "./copy";

describe("panel copy", () => {
  it("keeps the idle header free of status copy", () => {
    expect(panelHeadline({ kind: "idle" })).toBe("");
  });

  it("formats the completed headline from the spec example", () => {
    const state = deriveSurfaceState(tasksForKind("completed"));
    expect(panelHeadline(state)).toBe("Codex 已完成：修复登录页测试");
  });

  it("does not render a redundant completed-result button", () => {
    const state = deriveSurfaceState(tasksForKind("completed"));
    expect(panelActionLabel(state)).toBeUndefined();
  });

  it("does not add a peek summary while working", () => {
    const state = deriveSurfaceState(tasksForKind("working"));
    expect(panelSummary(state)).toBeUndefined();
  });

  it("keeps waiting and failed summaries for peek", () => {
    expect(panelSummary(deriveSurfaceState(tasksForKind("waiting")))).toBe(
      "需要确认后才能继续改测试",
    );
    expect(panelSummary(deriveSurfaceState(tasksForKind("failed")))).toBe(
      "登录页断言失败：expected 200, got 500",
    );
  });
});

describe("shellSize", () => {
  it("matches the documented logical sizes", () => {
    expect(shellSize("top", "collapsed", "card")).toEqual({ width: 44, height: 44 });
    expect(shellSize("left", "collapsed", "strip")).toEqual({ width: 44, height: 44 });
    expect(shellSize("right", "peek", "card")).toEqual({ width: 268, height: 48 });
    expect(shellSize("top", "peek", "strip", true)).toEqual({ width: 360, height: 48 });
    expect(shellSize("top", "peek", "strip", true, true)).toEqual({ width: 520, height: 48 });
    expect(shellSize("top", "expanded", "card")).toEqual({ width: 360, height: 448 });
    expect(shellSize("top", "expanded", "card", true, true)).toEqual({ width: 520, height: 448 });
  });
});
