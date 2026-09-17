import { describe, expect, it } from "vitest";
import { nativePanelLayout } from "./panel-layout";
import { shellSize } from "./copy";

describe("widget page layout", () => {
  it("widens only the expanded page and leaves resting shapes intact", () => {
    expect(shellSize("top", "expanded", "strip", false, false, "widgets")).toEqual({ width: 800, height: 260 });
    expect(shellSize("top", "expanded", "strip", true, true, "widgets")).toEqual({ width: 800, height: 260 });
    expect(shellSize("top", "expanded")).toEqual({ width: 440, height: 420 });
    expect(shellSize("top", "peek", "strip", false, false, "widgets")).toEqual(shellSize("top", "peek"));
    expect(shellSize("top", "collapsed", "strip", true, false, "widgets")).toEqual({ width: 48, height: 48 });
  });

  it("sends explicit native widget layouts and restores the task layout on return", () => {
    expect(nativePanelLayout("expanded", false, "widgets")).toBe("widgets");
    expect(nativePanelLayout("expanded", true, "widgets")).toBe("pinned-widgets");
    expect(nativePanelLayout("expanded", true, "tasks")).toBe("pinned-expanded");
    expect(nativePanelLayout("peek", true, "widgets")).toBe("pinned-peek");
    expect(nativePanelLayout("collapsed", false, "widgets")).toBe("collapsed");
  });
});
