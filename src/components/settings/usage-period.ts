import type { DailyUsage, UsageTotals } from "$domain/usage";
import { aggregateUsage } from "./usage-calendar";

export type UsagePeriod = "day" | "week" | "month";

export interface UsagePeriodRange {
  start: string;
  end: string;
}

export interface DailyUsagePoint {
  date: string;
  totals: UsageTotals;
}

export function periodRange(anchor: string, period: UsagePeriod): UsagePeriodRange {
  const date = parseDateKey(anchor);
  if (period === "day") return { start: anchor, end: anchor };
  if (period === "month") {
    return {
      start: dateKey(new Date(date.getFullYear(), date.getMonth(), 1, 12)),
      end: dateKey(new Date(date.getFullYear(), date.getMonth() + 1, 0, 12)),
    };
  }
  const mondayOffset = (date.getDay() + 6) % 7;
  return {
    start: dateKey(addDays(date, -mondayOffset)),
    end: dateKey(addDays(date, 6 - mondayOffset)),
  };
}

export function shiftPeriod(anchor: string, period: UsagePeriod, amount: number): string {
  const date = parseDateKey(anchor);
  if (period === "month") {
    return dateKey(new Date(date.getFullYear(), date.getMonth() + amount, 1, 12));
  }
  return dateKey(addDays(date, amount * (period === "week" ? 7 : 1)));
}

export function periodRows(rows: DailyUsage[], range: UsagePeriodRange): DailyUsage[] {
  return rows.filter((row) => row.date >= range.start && row.date <= range.end);
}

export function periodMonthKeys(range: UsagePeriodRange, visibleMonth: string): string[] {
  const keys = new Set([visibleMonth]);
  let cursor = parseDateKey(`${range.start.slice(0, 7)}-01`);
  const end = parseDateKey(`${range.end.slice(0, 7)}-01`);
  while (cursor <= end) {
    keys.add(dateKey(cursor).slice(0, 7));
    cursor = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 1, 12);
  }
  return [...keys].sort();
}

export function dailyUsagePoints(rows: DailyUsage[], range: UsagePeriodRange): DailyUsagePoint[] {
  const points: DailyUsagePoint[] = [];
  let cursor = parseDateKey(range.start);
  const end = parseDateKey(range.end);
  while (cursor <= end) {
    const key = dateKey(cursor);
    points.push({
      date: key,
      totals: aggregateUsage(rows.filter((row) => row.date === key)),
    });
    cursor = addDays(cursor, 1);
  }
  return points;
}

export function periodNavigationLabel(range: UsagePeriodRange, period: UsagePeriod): string {
  const start = parseDateKey(range.start);
  const end = parseDateKey(range.end);
  if (period === "month") return `${start.getFullYear()}年${start.getMonth() + 1}月`;
  if (period === "day") return `${start.getMonth() + 1}月${start.getDate()}日`;
  if (start.getFullYear() !== end.getFullYear()) {
    return `${start.getFullYear()}.${start.getMonth() + 1}.${start.getDate()}–${end.getFullYear()}.${end.getMonth() + 1}.${end.getDate()}`;
  }
  if (start.getMonth() !== end.getMonth()) {
    return `${start.getFullYear()}年${start.getMonth() + 1}月${start.getDate()}日–${end.getMonth() + 1}月${end.getDate()}日`;
  }
  return `${start.getFullYear()}年${start.getMonth() + 1}月${start.getDate()}–${end.getDate()}日`;
}

export function rangeIncludes(range: UsagePeriodRange, date: string): boolean {
  return date >= range.start && date <= range.end;
}

function parseDateKey(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day, 12);
}

function dateKey(value: Date): string {
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`;
}

function addDays(value: Date, amount: number): Date {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate() + amount, 12);
}
