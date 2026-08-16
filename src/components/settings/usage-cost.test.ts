import { describe, expect, it } from "vitest";
import type { DailyUsage } from "$domain/usage";
import { estimateUsageCost, formatEstimatedRmb, groupUsageCostByModel } from "./usage-cost";

function usage(overrides: Partial<DailyUsage> = {}): DailyUsage {
  return {
    date: "2026-08-15",
    source: "codex",
    model: "gpt-5.6-sol",
    contextTier: "short",
    inputTokens: 37_200_000,
    cachedInputTokens: 36_200_000,
    outputTokens: 145_300,
    reasoningTokens: 53_900,
    totalTokens: 37_345_300,
    ...overrides,
  };
}

describe("usage cost estimates", () => {
  it("prices cached input separately and does not double-count reasoning", () => {
    const estimate = estimateUsageCost([usage()]);
    expect(estimate.usd).toBeCloseTo(27.459, 6);
    expect(formatEstimatedRmb(estimate)).toBe("¥197.70");
    expect(estimate.coverage).toBe(1);
  });

  it("applies long-context rates per aggregated tier", () => {
    const estimate = estimateUsageCost([
      usage({
        contextTier: "long",
        inputTokens: 1_000_000,
        cachedInputTokens: 800_000,
        outputTokens: 100_000,
        totalTokens: 1_100_000,
      }),
    ]);
    expect(estimate.usd).toBeCloseTo(7.3, 6);
  });

  it("keeps unknown models visible and excludes them from the estimate", () => {
    const rows = [
      usage({ model: "gpt-5.6-luna", inputTokens: 1_000_000, cachedInputTokens: 0, outputTokens: 0, totalTokens: 1_000_000 }),
      usage({ source: "grok-cli", model: "future-model", inputTokens: 3_000_000, cachedInputTokens: 0, outputTokens: 0, totalTokens: 3_000_000 }),
    ];
    const estimate = estimateUsageCost(rows);
    expect(estimate.usd).toBeCloseTo(0.2, 6);
    expect(estimate.coverage).toBeCloseTo(0.25, 6);
    expect(estimate.unpricedModels).toEqual(["future-model"]);
    expect(groupUsageCostByModel(rows)).toHaveLength(2);
  });
});
