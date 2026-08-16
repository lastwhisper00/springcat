import { describe, expect, it } from "vitest";
import { shouldPinPanel } from "./pin-policy";

describe("shouldPinPanel", () => {
  it("preserves a manual pin regardless of task state", () => {
    expect(
      shouldPinPanel(
        { alwaysOnTop: true, autoPinWhileRunning: false },
        [{ status: "completed" }],
      ),
    ).toBe(true);
  });

  it("temporarily pins while any conversation is running", () => {
    const settings = { alwaysOnTop: false, autoPinWhileRunning: true };
    expect(shouldPinPanel(settings, [{ status: "completed" }, { status: "running" }])).toBe(
      true,
    );
    expect(shouldPinPanel(settings, [{ status: "completed" }, { status: "failed" }])).toBe(
      false,
    );
  });

  it("does not react to running tasks when automatic pinning is disabled", () => {
    expect(
      shouldPinPanel(
        { alwaysOnTop: false, autoPinWhileRunning: false },
        [{ status: "running" }],
      ),
    ).toBe(false);
  });
});
