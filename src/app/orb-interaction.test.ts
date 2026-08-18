import { describe, expect, it } from "vitest";
import {
  drawerIdleTarget,
  orbTargetLayout,
  pillTargetLayout,
  suppressUserCollapsedAutoOpen,
  taskPolicyKey,
} from "./orb-interaction";

describe("orb interaction", () => {
  it("opens the pill and drawer from the collapsed ball", () => {
    expect(orbTargetLayout("collapsed")).toBe("expanded");
  });

  it("uses the ball as a master close control for both open layouts", () => {
    expect(orbTargetLayout("peek")).toBe("collapsed");
    expect(orbTargetLayout("expanded")).toBe("collapsed");
  });

  it("uses the pill to toggle only the conversation drawer", () => {
    expect(pillTargetLayout("peek")).toBe("expanded");
    expect(pillTargetLayout("expanded")).toBe("peek");
    expect(pillTargetLayout("collapsed")).toBe("collapsed");
  });

  it("folds an idle drawer without collapsing the pill", () => {
    expect(drawerIdleTarget("expanded")).toBe("peek");
  });

  it("preserves an explicit orb close while task policy still wants a pill", () => {
    expect(suppressUserCollapsedAutoOpen("collapsed", true)).toBe(true);
  });

  it("does not block policy without an explicit close or after the orb reopens", () => {
    expect(suppressUserCollapsedAutoOpen("collapsed", false)).toBe(false);
    expect(suppressUserCollapsedAutoOpen("expanded", true)).toBe(false);
  });

  it("resets an explicit close only for task identity or status changes", () => {
    const current = [{ id: "task-1", status: "running" as const }];
    expect(taskPolicyKey(current)).toBe(taskPolicyKey([...current]));
    expect(taskPolicyKey(current)).not.toBe(
      taskPolicyKey([{ id: "task-1", status: "completed" }]),
    );
    expect(taskPolicyKey(current)).not.toBe(
      taskPolicyKey([{ id: "task-2", status: "running" }]),
    );
  });
});
