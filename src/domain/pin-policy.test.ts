import { describe, expect, it } from "vitest";
import { shouldPinPanel } from "./pin-policy";

describe("shouldPinPanel", () => {
  it("lets a manual unpin suppress automatic pinning without disabling the preference", () => {
    const settings = { alwaysOnTop: false, autoPinWhileRunning: true };
    expect(shouldPinPanel(settings, [{ status: "running" }], true)).toBe(false);
    expect(shouldPinPanel(settings, [{ status: "running" }], false)).toBe(true);
    expect(shouldPinPanel({ ...settings, alwaysOnTop: true }, [{ status: "running" }], true)).toBe(true);
    expect(settings.autoPinWhileRunning).toBe(true);
  });
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
