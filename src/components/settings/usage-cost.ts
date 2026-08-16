import {
  EMPTY_USAGE_TOTALS,
  addUsageTotals,
  type DailyUsage,
  type UsageSource,
  type UsageTotals,
} from "$domain/usage";

export const USD_CNY_ESTIMATE = 7.2;
export const RATE_CARD_DATE = "2026-08-15";

interface TokenRates {
  input: number;
  cachedInput: number;
  output: number;
}

interface ModelRateCard {
  canonicalModel: string;
  aliases: string[];
  short: TokenRates;
  long?: TokenRates;
}

export interface UsageCostEstimate {
  usd: number;
  cny: number;
  pricedTokens: number;
  totalTokens: number;
  coverage: number;
  unpricedModels: string[];
}

export interface ModelUsageCost {
  source: UsageSource;
  model: string;
  totals: UsageTotals;
  estimate: UsageCostEstimate;
}

// USD per 1M tokens. This is intentionally a small, auditable catalogue for
// models emitted by the local Codex and Grok logs; unknown models stay unpriced.
const MODEL_RATE_CARDS: ModelRateCard[] = [
  {
    canonicalModel: "gpt-5.6-sol",
    aliases: ["gpt-5.6-sol", "gpt-5.6"],
    short: { input: 5, cachedInput: 0.5, output: 30 },
    long: { input: 10, cachedInput: 1, output: 45 },
  },
  {
    canonicalModel: "gpt-5.6-terra",
    aliases: ["gpt-5.6-terra"],
    short: { input: 2, cachedInput: 0.2, output: 12 },
    long: { input: 4, cachedInput: 0.4, output: 18 },
  },
  {
    canonicalModel: "gpt-5.6-luna",
    aliases: ["gpt-5.6-luna"],
    short: { input: 0.2, cachedInput: 0.02, output: 1.2 },
    long: { input: 0.4, cachedInput: 0.04, output: 1.8 },
  },
  {
    canonicalModel: "gpt-5.5",
    aliases: ["gpt-5.5"],
    short: { input: 5, cachedInput: 0.5, output: 30 },
    long: { input: 10, cachedInput: 1, output: 45 },
  },
  {
    canonicalModel: "gpt-5.3-codex",
    aliases: ["gpt-5.3-codex"],
    short: { input: 1.75, cachedInput: 0.175, output: 14 },
  },
  {
    canonicalModel: "grok-4.5",
    aliases: ["grok-4.5", "grok-4.5-latest"],
    short: { input: 2, cachedInput: 0.3, output: 6 },
    long: { input: 4, cachedInput: 0.6, output: 12 },
  },
];

export function estimateUsageCost(rows: DailyUsage[]): UsageCostEstimate {
  let usd = 0;
  let pricedTokens = 0;
  let totalTokens = 0;
  const unpricedModels = new Set<string>();

  for (const row of rows) {
    const rowTokens = Math.max(0, row.totalTokens);
    totalTokens += rowTokens;
    const card = findRateCard(row.model);
    if (!card) {
      if (rowTokens > 0) unpricedModels.add(row.model?.trim() || "未识别模型");
      continue;
    }
    const rates = row.contextTier === "long" ? (card.long ?? card.short) : card.short;
    const inputTokens = Math.max(0, row.inputTokens);
    const cachedInputTokens = Math.min(inputTokens, Math.max(0, row.cachedInputTokens));
    const uncachedInputTokens = inputTokens - cachedInputTokens;
    usd +=
      (uncachedInputTokens * rates.input
        + cachedInputTokens * rates.cachedInput
        + Math.max(0, row.outputTokens) * rates.output) /
      1_000_000;
    pricedTokens += rowTokens;
  }

  return {
    usd,
    cny: usd * USD_CNY_ESTIMATE,
    pricedTokens,
    totalTokens,
    coverage: totalTokens > 0 ? pricedTokens / totalTokens : 0,
    unpricedModels: [...unpricedModels].sort(),
  };
}

export function groupUsageCostByModel(rows: DailyUsage[]): ModelUsageCost[] {
  const groups = new Map<string, DailyUsage[]>();
  for (const row of rows) {
    const model = row.model?.trim() || "未识别模型";
    const key = `${row.source}\u0000${model}`;
    groups.set(key, [...(groups.get(key) ?? []), row]);
  }

  return [...groups.values()]
    .map((modelRows) => ({
      source: modelRows[0].source,
      model: modelRows[0].model?.trim() || "未识别模型",
      totals: modelRows.reduce(
        (total, row) => addUsageTotals(total, row),
        { ...EMPTY_USAGE_TOTALS },
      ),
      estimate: estimateUsageCost(modelRows),
    }))
    .sort((left, right) => right.totals.totalTokens - left.totals.totalTokens);
}

export function formatEstimatedRmb(estimate: UsageCostEstimate): string {
  if (estimate.pricedTokens <= 0) return "--";
  if (estimate.cny > 0 && estimate.cny < 0.01) return "<¥0.01";
  return `¥${estimate.cny.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function formatCostCoverage(estimate: UsageCostEstimate): string {
  if (estimate.totalTokens <= 0) return "暂无用量";
  return `${Math.round(estimate.coverage * 100)}% Token 已匹配价格`;
}

function findRateCard(model: string | null): ModelRateCard | undefined {
  const normalized = model?.trim().toLowerCase();
  if (!normalized) return undefined;
  return MODEL_RATE_CARDS.find((card) =>
    card.aliases.some(
      (alias) => normalized === alias || normalized.startsWith(`${alias}-202`),
    ),
  );
}
