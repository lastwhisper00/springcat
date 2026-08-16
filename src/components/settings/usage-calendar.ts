import {
  EMPTY_USAGE_TOTALS,
  addUsageTotals,
  type DailyUsage,
  type UsageSource,
  type UsageTotals,
} from "$domain/usage";

export interface CalendarCell {
  date: string;
  day: number;
  inMonth: boolean;
  isToday: boolean;
  totals: UsageTotals;
  bySource: Partial<Record<UsageSource, UsageTotals>>;
}

export const USAGE_SOURCE_META: Record<UsageSource, { label: string; short: string }> = {
  codex: { label: "Codex", short: "CX" },
  cursor: { label: "Cursor", short: "CU" },
  "grok-cli": { label: "Grok CLI", short: "GR" },
};

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

function localDateKey(value: Date): string {
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`;
}

export function monthKey(value: Date): string {
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}`;
}

export function monthLabel(value: Date): string {
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "long" }).format(value);
}

export function shiftMonth(value: Date, amount: number): Date {
  return new Date(value.getFullYear(), value.getMonth() + amount, 1);
}

export function buildCalendarCells(
  visibleMonth: Date,
  rows: DailyUsage[],
  today = new Date(),
): CalendarCell[] {
  const year = visibleMonth.getFullYear();
  const month = visibleMonth.getMonth();
  const first = new Date(year, month, 1);
  const mondayOffset = (first.getDay() + 6) % 7;
  const todayKey = localDateKey(today);
  const usage = aggregateByDate(rows);

  return Array.from({ length: 42 }, (_, index) => {
    const value = new Date(year, month, 1 - mondayOffset + index);
    const date = localDateKey(value);
    const entry = usage.get(date);
    return {
      date,
      day: value.getDate(),
      inMonth: value.getMonth() === month,
      isToday: date === todayKey,
      totals: entry?.totals ?? { ...EMPTY_USAGE_TOTALS },
      bySource: entry?.bySource ?? {},
    };
  });
}

export function aggregateUsage(rows: DailyUsage[]): UsageTotals {
  return rows.reduce(
    (total, row) => addUsageTotals(total, row),
    { ...EMPTY_USAGE_TOTALS },
  );
}

export function formatCompactTokens(value: number): string {
  if (value <= 0) return "--";
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toLocaleString("zh-CN", { maximumFractionDigits: 1 })}M`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toLocaleString("zh-CN", { maximumFractionDigits: 1 })}K`;
  }
  return value.toLocaleString("zh-CN");
}

function aggregateByDate(rows: DailyUsage[]): Map<
  string,
  { totals: UsageTotals; bySource: Partial<Record<UsageSource, UsageTotals>> }
> {
  const result = new Map<
    string,
    { totals: UsageTotals; bySource: Partial<Record<UsageSource, UsageTotals>> }
  >();
  for (const row of rows) {
    const current = result.get(row.date) ?? {
      totals: { ...EMPTY_USAGE_TOTALS },
      bySource: {},
    };
    const sourceTotal = current.bySource[row.source] ?? { ...EMPTY_USAGE_TOTALS };
    current.bySource[row.source] = addUsageTotals(sourceTotal, row);
    current.totals = addUsageTotals(current.totals, row);
    result.set(row.date, current);
  }
  return result;
}

