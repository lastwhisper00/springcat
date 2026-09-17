import { describe, expect, it, vi } from "vitest";
import {
  animateSynchronizedResize,
  applySynchronizedResizeStep,
  synchronizedResizeEase,
} from "./synchronized-resize";

describe("synchronized dynamic-island resize", () => {
  it("grows native bounds before rendering a wider panel", async () => {
    const order: string[] = [];
    await applySynchronizedResizeStep({
      width: 520,
      height: 48,
      expanding: true,
      resizeNative: vi.fn(async () => {
        order.push("native");
      }),
      renderWidth: vi.fn(() => {
        order.push("visual");
      }),
    });
    expect(order).toEqual(["native", "visual"]);
  });

  it("renders a narrower panel before trimming native bounds", async () => {
    const order: string[] = [];
    await applySynchronizedResizeStep({
      width: 360,
      height: 48,
      expanding: false,
      resizeNative: vi.fn(async () => {
        order.push("native");
      }),
      renderWidth: vi.fn(() => {
        order.push("visual");
      }),
    });
    expect(order).toEqual(["visual", "native"]);
  });

  it("clamps the easing curve to stable endpoints", () => {
    expect(synchronizedResizeEase(-1)).toBe(0);
    expect(synchronizedResizeEase(0)).toBe(0);
    expect(synchronizedResizeEase(1)).toBe(1);
    expect(synchronizedResizeEase(2)).toBe(1);
  });

  it("moves early and settles late like the shared spatial curve", () => {
    // The iOS sheet curve covers most of the distance in the first half of
    // the timeline and then settles quietly; it must also be monotonic.
    expect(synchronizedResizeEase(0.25)).toBeGreaterThan(0.6);
    expect(synchronizedResizeEase(0.5)).toBeGreaterThan(0.9);
    let previous = 0;
    for (let i = 1; i <= 20; i += 1) {
      const eased = synchronizedResizeEase(i / 20);
      expect(eased).toBeGreaterThanOrEqual(previous);
      previous = eased;
    }
  });

  it("interpolates native width and height through the same timeline", async () => {
    const seen: { width: number; height: number }[] = [];
    const timestamps = [0, 50, 100];

    await animateSynchronizedResize({
      from: { width: 360, height: 48 },
      to: { width: 520, height: 448 },
      duration: 100,
      resize: async (dimensions) => {
        seen.push(dimensions);
      },
      now: () => 0,
      requestFrame: (callback) => {
        const timestamp = timestamps.shift();
        if (timestamp === undefined) throw new Error("unexpected animation frame");
        queueMicrotask(() => callback(timestamp));
      },
    });

    const midEase = synchronizedResizeEase(0.5);
    expect(seen).toEqual([
      { width: 360, height: 48 },
      { width: 360 + 160 * midEase, height: 48 + 400 * midEase },
      { width: 520, height: 448 },
    ]);
  });
});
