import type { UsageSource, UsageTotals } from "$domain/usage";
import type { ModelUsageCost, UsageCostEstimate } from "./usage-cost";
import type { DailyUsagePoint, UsagePeriod, UsagePeriodRange } from "./usage-period";

export interface UsageShareSource {
  source: UsageSource;
  totals: UsageTotals;
}

export interface UsageShareCardInput {
  period: UsagePeriod;
  range: UsagePeriodRange;
  periodLabel: string;
  totals: UsageTotals;
  estimate: UsageCostEstimate;
  sources: UsageShareSource[];
  daily: DailyUsagePoint[];
  models: ModelUsageCost[];
  activeDays: number;
}

const SOURCE_META: Record<UsageSource, { label: string; color: string }> = {
  codex: { label: "Codex", color: "#ceff70" },
  cursor: { label: "Cursor", color: "#ff9874" },
  "grok-cli": { label: "Grok CLI", color: "#62ddc8" },
};

const SHARE_CARD_WIDTH = 1200;
const SHARE_CARD_HEIGHT = 2000;
const HERO_CONTENT_LEFT = 112;
const HERO_CONTENT_RIGHT = 1088;
const HERO_STATS_WIDTH = 232;
const HERO_STATS_GAP = 16;

export function buildUsageShareFilename(input: Pick<UsageShareCardInput, "period" | "range">): string {
  const period = input.period === "day" ? "日" : input.period === "week" ? "周" : "月";
  const suffix = input.range.start === input.range.end
    ? input.range.start
    : `${input.range.start}_${input.range.end}`;
  return `SpringCat-AI${period}报-${suffix}.png`;
}

export async function renderUsageShareCard(
  input: UsageShareCardInput,
  logoUrl: string,
): Promise<Blob> {
  const canvas = document.createElement("canvas");
  canvas.width = SHARE_CARD_WIDTH;
  canvas.height = SHARE_CARD_HEIGHT;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("当前环境无法生成分享图片");

  drawBackground(context, canvas.width, canvas.height);
  await drawHeader(context, logoUrl, input);
  drawHero(context, input);
  drawTrend(context, input);
  drawSourceBreakdown(context, input);
  drawModelStrip(context, input);
  drawFooter(context, input);

  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => blob ? resolve(blob) : reject(new Error("生成 PNG 失败")),
      "image/png",
      0.96,
    );
  });
}

function drawBackground(ctx: CanvasRenderingContext2D, width: number, height: number) {
  const background = ctx.createLinearGradient(0, 0, width, height);
  background.addColorStop(0, "#080907");
  background.addColorStop(0.55, "#0d0e0c");
  background.addColorStop(1, "#070806");
  ctx.fillStyle = background;
  ctx.fillRect(0, 0, width, height);

  const glow = ctx.createRadialGradient(940, 120, 20, 940, 120, 660);
  glow.addColorStop(0, "rgba(242, 239, 230, 0.11)");
  glow.addColorStop(0.46, "rgba(242, 239, 230, 0.035)");
  glow.addColorStop(1, "rgba(0, 0, 0, 0)");
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, width, 800);

  ctx.save();
  ctx.globalAlpha = 0.085;
  ctx.strokeStyle = "#d7d3c8";
  ctx.lineWidth = 1;
  for (let x = 70; x < width; x += 72) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }
  for (let y = 40; y < height; y += 72) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  }
  ctx.restore();
}

async function drawHeader(
  ctx: CanvasRenderingContext2D,
  logoUrl: string,
  input: UsageShareCardInput,
) {
  roundedRect(ctx, 70, 64, 82, 82, 23);
  ctx.fillStyle = "#f2efe6";
  ctx.fill();
  try {
    const logo = await loadImage(logoUrl);
    ctx.drawImage(logo, 79, 73, 64, 64);
  } catch {
    ctx.fillStyle = "#0a0b09";
    ctx.font = font(34, 800);
    ctx.fillText("S", 100, 118);
  }

  ctx.fillStyle = "#f2efe6";
  ctx.font = font(31, 760);
  ctx.fillText("SpringCat", 174, 100);
  ctx.fillStyle = "#8d8b84";
  ctx.font = font(18, 600);
  ctx.fillText("AI PRODUCTIVITY INSIGHTS", 176, 130);

  ctx.textAlign = "right";
  ctx.fillStyle = "#d2cfc6";
  ctx.font = font(19, 650);
  ctx.fillText(input.periodLabel, 1128, 100);
  ctx.fillStyle = "#74736d";
  ctx.font = font(16, 500);
  ctx.fillText("LOCAL-FIRST · EXACT TOKEN LOGS", 1128, 130);
  ctx.textAlign = "left";
}

function drawHero(ctx: CanvasRenderingContext2D, input: UsageShareCardInput) {
  const gradient = ctx.createLinearGradient(70, 200, 1130, 724);
  gradient.addColorStop(0, "#d9ff72");
  gradient.addColorStop(0.58, "#91eda1");
  gradient.addColorStop(1, "#63d8c6");
  roundedRect(ctx, 70, 198, 1060, 526, 36);
  ctx.fillStyle = gradient;
  ctx.fill();
  ctx.strokeStyle = "rgba(223, 255, 190, 0.72)";
  ctx.lineWidth = 1;
  ctx.stroke();

  ctx.fillStyle = "#355146";
  ctx.font = font(20, 760);
  ctx.fillText(periodKicker(input.period), HERO_CONTENT_LEFT, 248);
  ctx.fillStyle = "#09120d";
  const headline = periodHeadline(input.period);
  ctx.font = fittedFont(ctx, headline, 36, 30, 720, HERO_CONTENT_RIGHT - HERO_CONTENT_LEFT);
  ctx.fillText(headline, HERO_CONTENT_LEFT, 294);

  ctx.fillStyle = "#07100b";
  const totalLabel = formatShareTokens(input.totals.totalTokens);
  ctx.font = fittedFont(ctx, totalLabel, 108, 84, 820, HERO_CONTENT_RIGHT - HERO_CONTENT_LEFT - 142);
  ctx.fillText(totalLabel, 108, 420);
  const tokenWidth = ctx.measureText(totalLabel).width;
  ctx.fillStyle = "#315347";
  ctx.font = font(24, 620);
  ctx.fillText("TOKEN", 124 + tokenWidth, 415);

  const costLabel = `API 等价 ≈ ${formatRmb(input.estimate.cny)}`;
  ctx.font = font(22, 720);
  const costWidth = Math.min(520, Math.max(330, ctx.measureText(costLabel).width + 52));
  roundedRect(ctx, HERO_CONTENT_LEFT, 460, costWidth, 58, 29);
  ctx.fillStyle = "#10110f";
  ctx.fill();
  ctx.fillStyle = "#eaffcf";
  ctx.font = fittedFont(ctx, costLabel, 22, 18, 720, costWidth - 52);
  ctx.fillText(costLabel, 138, 497);

  ctx.fillStyle = "#29483d";
  const bragLine = buildBragLine(input.totals.totalTokens, input.activeDays);
  ctx.font = fittedFont(ctx, bragLine, 19, 16, 520, HERO_CONTENT_RIGHT - HERO_CONTENT_LEFT);
  ctx.fillText(bragLine, HERO_CONTENT_LEFT, 560);

  const stats = [
    ["输入", input.totals.inputTokens],
    ["缓存命中", input.totals.cachedInputTokens],
    ["输出", input.totals.outputTokens],
    ["推理", input.totals.reasoningTokens],
  ] as const;
  stats.forEach(([label, value], index) => {
    const x = HERO_CONTENT_LEFT + index * (HERO_STATS_WIDTH + HERO_STATS_GAP);
    const y = 598;
    roundedRect(ctx, x, y, HERO_STATS_WIDTH, 96, 20);
    ctx.fillStyle = "rgba(5, 28, 18, 0.105)";
    ctx.fill();
    ctx.fillStyle = "#31584a";
    ctx.font = font(17, 550);
    ctx.fillText(label, x + 21, y + 31);
    ctx.fillStyle = "#09150e";
    const valueLabel = formatShareTokens(value);
    ctx.font = fittedFont(ctx, valueLabel, 30, 23, 760, HERO_STATS_WIDTH - 42);
    ctx.fillText(valueLabel, x + 21, y + 70);
  });
}

function drawTrend(ctx: CanvasRenderingContext2D, input: UsageShareCardInput) {
  panel(ctx, 70, 758, 1060, 432);
  ctx.fillStyle = "#eff8ec";
  ctx.font = font(25, 720);
  ctx.fillText("消耗趋势", 108, 812);
  ctx.fillStyle = "#7d9888";
  ctx.font = font(16, 520);
  ctx.fillText(`${input.activeDays} 个活跃日 · 峰值 ${formatShareTokens(Math.max(0, ...input.daily.map((point) => point.totals.totalTokens)))}`, 108, 843);

  const left = 112;
  const top = 884;
  const width = 980;
  const height = 236;
  const values = input.daily.map((point) => point.totals.totalTokens);
  const max = Math.max(1, ...values);
  const points = values.map((value, index) => ({
    x: values.length <= 1 ? left + width / 2 : left + (index / (values.length - 1)) * width,
    y: top + height - (value / max) * height,
  }));

  ctx.strokeStyle = "rgba(163, 215, 178, 0.14)";
  ctx.lineWidth = 1;
  for (let index = 0; index <= 3; index += 1) {
    const y = top + (height / 3) * index;
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(left + width, y);
    ctx.stroke();
  }

  if (points.length === 1) {
    roundedRect(ctx, points[0].x - 36, points[0].y, 72, top + height - points[0].y, 18);
    ctx.fillStyle = "#ceff70";
    ctx.fill();
  } else {
    const area = ctx.createLinearGradient(0, top, 0, top + height);
    area.addColorStop(0, "rgba(181, 255, 113, 0.38)");
    area.addColorStop(1, "rgba(98, 221, 200, 0.018)");
    ctx.beginPath();
    ctx.moveTo(points[0].x, top + height);
    points.forEach((point) => ctx.lineTo(point.x, point.y));
    ctx.lineTo(points[points.length - 1].x, top + height);
    ctx.closePath();
    ctx.fillStyle = area;
    ctx.fill();

    ctx.beginPath();
    points.forEach((point, index) => index === 0 ? ctx.moveTo(point.x, point.y) : ctx.lineTo(point.x, point.y));
    ctx.strokeStyle = "#ceff70";
    ctx.lineWidth = 5;
    ctx.lineJoin = "round";
    ctx.stroke();
  }

  ctx.fillStyle = "#718779";
  ctx.font = font(14, 550);
  const labelIndexes = [...new Set([0, Math.floor((input.daily.length - 1) / 2), input.daily.length - 1])];
  labelIndexes.forEach((index) => {
    const point = points[index];
    if (!point) return;
    ctx.textAlign = index === 0 ? "left" : index === input.daily.length - 1 ? "right" : "center";
    ctx.fillText(shortDate(input.daily[index].date), point.x, 1162);
  });
  ctx.textAlign = "left";
}

function drawSourceBreakdown(ctx: CanvasRenderingContext2D, input: UsageShareCardInput) {
  panel(ctx, 70, 1224, 1060, 370);
  ctx.fillStyle = "#eff8ec";
  ctx.font = font(25, 720);
  ctx.fillText("AI 工具贡献", 108, 1278);
  ctx.fillStyle = "#7d9888";
  ctx.font = font(16, 520);
  ctx.fillText("按 Token 总量统计", 108, 1308);

  const nonEmpty = input.sources.filter((item) => item.totals.totalTokens > 0);
  const peak = Math.max(1, ...nonEmpty.map((item) => item.totals.totalTokens));
  nonEmpty.slice(0, 3).forEach((item, index) => {
    const y = 1360 + index * 70;
    const meta = SOURCE_META[item.source];
    ctx.fillStyle = meta.color;
    ctx.beginPath();
    ctx.arc(119, y + 7, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#d9e5d9";
    ctx.font = font(18, 650);
    ctx.fillText(meta.label, 140, y + 13);
    roundedRect(ctx, 300, y - 2, 660, 16, 8);
    ctx.fillStyle = "rgba(167, 211, 178, 0.13)";
    ctx.fill();
    roundedRect(ctx, 300, y - 2, Math.max(8, (item.totals.totalTokens / peak) * 660), 16, 8);
    ctx.fillStyle = meta.color;
    ctx.fill();
    ctx.textAlign = "right";
    ctx.fillStyle = "#eff8ec";
    ctx.font = font(19, 720);
    ctx.fillText(formatShareTokens(item.totals.totalTokens), 1090, y + 13);
    ctx.textAlign = "left";
  });
  if (nonEmpty.length === 0) {
    ctx.fillStyle = "#718779";
    ctx.font = font(20, 550);
    ctx.fillText("等待第一条 Token 记录", 108, 1374);
  }
}

function drawModelStrip(ctx: CanvasRenderingContext2D, input: UsageShareCardInput) {
  const models = input.models.filter((item) => item.totals.totalTokens > 0).slice(0, 3);
  ctx.fillStyle = "#829688";
  ctx.font = font(16, 650);
  ctx.fillText("TOP MODELS", 72, 1655);
  let x = 72;
  models.forEach((item) => {
    const label = `${item.model}  ${formatShareTokens(item.totals.totalTokens)}`;
    ctx.font = font(17, 630);
    const width = Math.min(330, ctx.measureText(label).width + 42);
    roundedRect(ctx, x, 1680, width, 52, 26);
    ctx.save();
    ctx.globalAlpha = 0.11;
    ctx.fillStyle = SOURCE_META[item.source].color;
    ctx.fill();
    ctx.restore();
    ctx.save();
    ctx.globalAlpha = 0.25;
    ctx.strokeStyle = SOURCE_META[item.source].color;
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.restore();
    ctx.fillStyle = SOURCE_META[item.source].color;
    ctx.fillText(truncateText(ctx, label, width - 40), x + 20, 1713);
    x += width + 12;
  });
}

function drawFooter(ctx: CanvasRenderingContext2D, input: UsageShareCardInput) {
  ctx.strokeStyle = "rgba(216, 212, 201, 0.16)";
  ctx.beginPath();
  ctx.moveTo(70, 1812);
  ctx.lineTo(1130, 1812);
  ctx.stroke();
  ctx.fillStyle = "#b6b3aa";
  ctx.font = font(18, 650);
  ctx.fillText("MAKE AI VISIBLE. MAKE PROGRESS SHAREABLE.", 72, 1860);
  ctx.textAlign = "right";
  ctx.fillStyle = "#ceff70";
  ctx.font = font(24, 760);
  ctx.fillText("springcat.cn", 1128, 1860);
  ctx.fillStyle = "#686761";
  ctx.font = font(15, 520);
  ctx.textAlign = "left";
  ctx.fillText("SpringCat · 本地日志自动汇总 · 不包含提示词与对话内容", 72, 1902);
  ctx.textAlign = "right";
  ctx.fillText(`价格覆盖 ${Math.round(input.estimate.coverage * 100)}% · API 等价估算，非实际账单`, 1128, 1902);
  ctx.textAlign = "left";
}

function panel(ctx: CanvasRenderingContext2D, x: number, y: number, width: number, height: number) {
  roundedRect(ctx, x, y, width, height, 28);
  const gradient = ctx.createLinearGradient(x, y, x + width, y + height);
  gradient.addColorStop(0, "rgba(16, 25, 19, 0.97)");
  gradient.addColorStop(1, "rgba(12, 18, 15, 0.97)");
  ctx.fillStyle = gradient;
  ctx.fill();
  ctx.strokeStyle = "rgba(154, 222, 172, 0.18)";
  ctx.lineWidth = 2;
  ctx.stroke();
}

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
) {
  const r = Math.min(radius, width / 2, height / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + width, y, x + width, y + height, r);
  ctx.arcTo(x + width, y + height, x, y + height, r);
  ctx.arcTo(x, y + height, x, y, r);
  ctx.arcTo(x, y, x + width, y, r);
  ctx.closePath();
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Logo 加载失败"));
    image.src = url;
  });
}

function font(size: number, weight: number): string {
  return `${weight} ${size}px system-ui, "Microsoft YaHei", "PingFang SC", sans-serif`;
}

function fittedFont(
  ctx: CanvasRenderingContext2D,
  text: string,
  preferredSize: number,
  minimumSize: number,
  weight: number,
  maxWidth: number,
): string {
  for (let size = preferredSize; size > minimumSize; size -= 1) {
    const candidate = font(size, weight);
    ctx.font = candidate;
    if (ctx.measureText(text).width <= maxWidth) return candidate;
  }
  return font(minimumSize, weight);
}

function truncateText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
  if (ctx.measureText(text).width <= maxWidth) return text;
  let result = text;
  while (result.length > 0 && ctx.measureText(`${result}…`).width > maxWidth) {
    result = result.slice(0, -1);
  }
  return `${result}…`;
}

function formatShareTokens(value: number): string {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(value >= 10_000_000_000 ? 1 : 2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(value >= 100_000_000 ? 1 : 2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toLocaleString("zh-CN");
}

function formatRmb(value: number): string {
  return `¥${value.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

function periodKicker(period: UsagePeriod): string {
  return period === "day" ? "DAILY AI REPORT" : period === "week" ? "WEEKLY AI REPORT" : "MONTHLY AI REPORT";
}

function periodHeadline(period: UsagePeriod): string {
  return period === "day" ? "今天，我与 AI 一起完成了这些工作" : period === "week" ? "这一周，我把 AI 变成了生产力" : "这个月，我的 AI 生产力战报";
}

function buildBragLine(total: number, activeDays: number): string {
  if (total >= 1_000_000_000) return `累计突破十亿 Token · ${activeDays} 个活跃日，把复杂工作交给算力。`;
  if (total >= 100_000_000) return `高强度 AI 协作 · ${activeDays} 个活跃日，每一次调用都在放大产出。`;
  if (total > 0) return `${activeDays} 个活跃日 · 让 AI 使用从感受变成可见的生产力。`;
  return "第一条 AI 生产力记录，正在路上。";
}

function shortDate(value: string): string {
  const [, month, day] = value.split("-");
  return `${Number(month)}/${Number(day)}`;
}
