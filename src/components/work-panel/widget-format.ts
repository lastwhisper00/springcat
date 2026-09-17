import type { NetworkSnapshot } from "$services/tauri";

export function formatWidgetTokens(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value) || value < 0) return "—";
  const divisor = value >= 1_000_000_000 ? 1_000_000_000 : value >= 1_000_000 ? 1_000_000 : value >= 1_000 ? 1_000 : 1;
  const suffix = divisor === 1_000_000_000 ? "B" : divisor === 1_000_000 ? "M" : divisor === 1_000 ? "K" : "";
  return `${(value / divisor).toLocaleString("zh-CN", { maximumFractionDigits: divisor === 1 ? 0 : 1 })}${suffix}`;
}

export function networkTypeLabel(network: NetworkSnapshot | null): string {
  if (!network) return "—";
  if (network.status === "offline") return "未连接";
  return {
    wifi: "Wi-Fi", ethernet: "以太网", cellular: "移动网络", vpn: "VPN / 隧道",
    other: "其他连接", unknown: "未识别", none: "未连接",
  }[network.connectionType];
}

export function networkNote(network: NetworkSnapshot | null): string {
  if (!network) return "正在检测…";
  if (network.status === "offline") return "网络已断开";
  if (network.foreignReachable) {
    return network.latencyMs === null ? "已连接" : `${network.latencyMs} ms`;
  }
  return "无法访问";
}

export function foreignReachabilityLabel(network: NetworkSnapshot | null): string {
  if (!network) return "检测中";
  if (network.status === "offline" || !network.foreignReachable) return "不可达";
  return "可达";
}
