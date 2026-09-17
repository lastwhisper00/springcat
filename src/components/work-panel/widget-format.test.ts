import { describe, expect, it } from "vitest";
import { foreignReachabilityLabel, formatWidgetTokens, networkNote, networkTypeLabel } from "./widget-format";
import type { NetworkSnapshot } from "$services/tauri";

describe("widget values", () => {
  it("distinguishes zero usage from unavailable data and keeps large totals compact", () => {
    expect(formatWidgetTokens(undefined)).toBe("—");
    expect(formatWidgetTokens(0)).toBe("0");
    expect(formatWidgetTokens(12800)).toBe("12.8K");
    expect(formatWidgetTokens(1240000)).toBe("1.2M");
    expect(formatWidgetTokens(1500000000)).toBe("1.5B");
  });

  it("does not equate a failed probe with a disconnected network", () => {
    const network: NetworkSnapshot = {
      connectionType: "wifi", status: "internet", foreignReachable: false,
      reachableService: null, latencyMs: null,
      probes: [
        { service: "ChatGPT", reachable: false, latencyMs: null },
        { service: "Google", reachable: false, latencyMs: null },
      ],
      checkedAt: "2026-09-17T12:00:00+08:00",
    };
    expect(networkTypeLabel(network)).toBe("Wi-Fi");
    expect(foreignReachabilityLabel(network)).toBe("不可达");
    expect(networkNote(network)).toBe("无法访问");
    expect(networkNote({ ...network, foreignReachable: true, reachableService: "ChatGPT", latencyMs: 20 })).toBe("20 ms");
    expect(foreignReachabilityLabel({ ...network, foreignReachable: true })).toBe("可达");
    expect(networkTypeLabel({ ...network, status: "offline" })).toBe("未连接");
  });
});
