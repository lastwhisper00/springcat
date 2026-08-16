import { describe, expect, it, vi } from "vitest";
import { applySynchronizedResizeStep, synchronizedResizeEase } from "./synchronized-resize";

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
});
