import { describe, expect, it } from "vitest";
import type { DailyUsage } from "$domain/usage";
import { aggregateUsage, buildCalendarCells, formatCompactTokens, monthKey } from "./usage-calendar";

const rows: DailyUsage[] = [
  {
    date: "2026-08-14",
    source: "codex",
    model: "gpt-5.6-sol",
    contextTier: "short",
    inputTokens: 1_000,
    cachedInputTokens: 600,
    outputTokens: 200,
    reasoningTokens: 50,
    totalTokens: 1_200,
  },
  {
    date: "2026-08-14",
    source: "grok-cli",
    model: "grok-4.5",
    contextTier: "short",
    inputTokens: 500,
    cachedInputTokens: 0,
    outputTokens: 100,
    reasoningTokens: 20,
    totalTokens: 600,
  },
];

describe("usage calendar", () => {
  it("builds a monday-first six-week grid", () => {
    const cells = buildCalendarCells(new Date(2026, 7, 1), rows, new Date(2026, 7, 14));
    expect(cells).toHaveLength(42);
    expect(cells[0].date).toBe("2026-07-27");
    expect(cells.find((cell) => cell.date === "2026-08-14")).toMatchObject({
      isToday: true,
      totals: { totalTokens: 1_800 },
    });
  });

  it("aggregates month totals without adding breakdowns twice", () => {
    expect(aggregateUsage(rows)).toEqual({
      inputTokens: 1_500,
      cachedInputTokens: 600,
      outputTokens: 300,
      reasoningTokens: 70,
      totalTokens: 1_800,
    });
  });

  it("formats month keys and compact token counts", () => {
    expect(monthKey(new Date(2026, 7, 1))).toBe("2026-08");
    expect(formatCompactTokens(0)).toBe("--");
    expect(formatCompactTokens(12_400)).toBe("12.4K");
    expect(formatCompactTokens(1_250_000)).toBe("1.3M");
  });
});
