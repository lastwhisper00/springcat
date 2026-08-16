import { describe, expect, it } from "vitest";
import type { DailyUsage } from "$domain/usage";
import {
  dailyUsagePoints,
  periodMonthKeys,
  periodNavigationLabel,
  periodRange,
  periodRows,
  shiftPeriod,
} from "./usage-period";

const usage: DailyUsage[] = [
  {
    date: "2026-08-31",
    source: "codex",
    model: "gpt-5.6-sol",
    contextTier: "short",
    inputTokens: 900,
    cachedInputTokens: 500,
    outputTokens: 100,
    reasoningTokens: 20,
    totalTokens: 1_000,
  },
  {
    date: "2026-09-01",
    source: "grok-cli",
    model: "grok-4.5",
    contextTier: "short",
    inputTokens: 450,
    cachedInputTokens: 200,
    outputTokens: 50,
    reasoningTokens: 10,
    totalTokens: 500,
  },
];

describe("usage periods", () => {
  it("builds monday-first weeks across month boundaries", () => {
    const range = periodRange("2026-09-02", "week");
    expect(range).toEqual({ start: "2026-08-31", end: "2026-09-06" });
    expect(periodMonthKeys(range, "2026-09")).toEqual(["2026-08", "2026-09"]);
    expect(periodRows(usage, range)).toHaveLength(2);
    expect(dailyUsagePoints(usage, range)).toHaveLength(7);
  });

  it("shifts by the selected period", () => {
    expect(shiftPeriod("2026-08-15", "day", 1)).toBe("2026-08-16");
    expect(shiftPeriod("2026-08-15", "week", -1)).toBe("2026-08-08");
    expect(shiftPeriod("2026-08-15", "month", 1)).toBe("2026-09-01");
  });

  it("formats navigation labels for day, week, and month", () => {
    expect(periodNavigationLabel(periodRange("2026-08-15", "day"), "day")).toBe("8月15日");
    expect(periodNavigationLabel(periodRange("2026-08-15", "week"), "week")).toBe("2026年8月10–16日");
    expect(periodNavigationLabel(periodRange("2026-08-15", "month"), "month")).toBe("2026年8月");
  });
});
