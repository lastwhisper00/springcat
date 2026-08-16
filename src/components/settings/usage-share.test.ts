import { describe, expect, it } from "vitest";
import { buildUsageShareFilename } from "./usage-share";

describe("usage share card", () => {
  it("builds a stable filename for the selected period", () => {
    expect(buildUsageShareFilename({
      period: "week",
      range: { start: "2026-08-10", end: "2026-08-16" },
    })).toBe("SpringCat-AI周报-2026-08-10_2026-08-16.png");
  });
});
