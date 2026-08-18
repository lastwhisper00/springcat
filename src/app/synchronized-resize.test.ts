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
    expect(synchronizedResizeEase(0.5)).toBeCloseTo(0.5);
    expect(synchronizedResizeEase(1)).toBe(1);
    expect(synchronizedResizeEase(2)).toBe(1);
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

    expect(seen).toEqual([
      { width: 360, height: 48 },
      { width: 440, height: 248 },
      { width: 520, height: 448 },
    ]);
  });
});
