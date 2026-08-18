import { describe, expect, it } from "vitest";
import {
  drawerIdleTarget,
  orbTargetLayout,
  pillTargetLayout,
  suppressPinnedAutoOpen,
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

  it("preserves an explicit orb close for the current pinned period", () => {
    expect(suppressPinnedAutoOpen(true, "collapsed", true)).toBe(true);
  });

  it("does not block a fresh pin, an orb reopen, or a later unpinned period", () => {
    expect(suppressPinnedAutoOpen(true, "collapsed", false)).toBe(false);
    expect(suppressPinnedAutoOpen(true, "expanded", true)).toBe(false);
    expect(suppressPinnedAutoOpen(false, "collapsed", true)).toBe(false);
  });
});
