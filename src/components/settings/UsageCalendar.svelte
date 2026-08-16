<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import type { DailyUsage, UsageSource, UsageTotals } from "$domain/usage";
  import { listUsageMonth, saveUsageShareImage } from "$services/tauri";
  import brandLogo from "../../assets/branding/透明.png";
  import {
    USAGE_SOURCE_META,
    aggregateUsage,
    buildCalendarCells,
    formatCompactTokens,
    monthKey as getMonthKey,
    monthLabel,
  } from "./usage-calendar";
  import {
    USD_CNY_ESTIMATE,
    estimateUsageCost,
    formatCostCoverage,
    formatEstimatedRmb,
    groupUsageCostByModel,
  } from "./usage-cost";
  import {
    dailyUsagePoints,
    periodMonthKeys,
    periodNavigationLabel,
    periodRange,
    periodRows as filterPeriodRows,
    rangeIncludes,
    shiftPeriod,
    type UsagePeriod,
  } from "./usage-period";
  import {
    buildUsageShareFilename,
    renderUsageShareCard,
  } from "./usage-share";

  let {
    boundSources,
    preview = false,
  }: {
    boundSources: UsageSource[];
    preview?: boolean;
  } = $props();

  const now = new Date();
  const todayKey = localDateKey(now);
  let period = $state<UsagePeriod>("month");
  let visibleMonth = $state(new Date(now.getFullYear(), now.getMonth(), 1));
  let selectedDate = $state(todayKey);
  let rows = $state<DailyUsage[]>([]);
  let loading = $state(true);
  let loadError = $state("");
  let shareBusy = $state(false);
  let shareOpen = $state(false);
  let sharePreviewUrl = $state("");
  let shareBlob = $state<Blob | null>(null);
  let shareNote = $state("");
  let datePickerOpen = $state(false);
  let datePickerMonth = $state(new Date(now.getFullYear(), now.getMonth(), 1));
  let datePickerRoot = $state<HTMLSpanElement>();
  let datePickerDialog = $state<HTMLDivElement>();
  let loadSequence = 0;

  const visibleMonthKey = $derived(getMonthKey(visibleMonth));
  const selectedRange = $derived(periodRange(selectedDate, period));
  const requestedMonths = $derived(periodMonthKeys(selectedRange, visibleMonthKey));
  const calendarRows = $derived(
    rows.filter((row) => row.date.startsWith(visibleMonthKey) && boundSources.includes(row.source)),
  );
  const cells = $derived(buildCalendarCells(visibleMonth, calendarRows));
  const datePickerCells = $derived(buildCalendarCells(datePickerMonth, []));
  const calendarTotals = $derived(aggregateUsage(calendarRows));
  const scopedRows = $derived(
    filterPeriodRows(rows, selectedRange).filter((row) => boundSources.includes(row.source)),
  );
  const scopedTotals = $derived(aggregateUsage(scopedRows));
  const scopedCost = $derived(estimateUsageCost(scopedRows));
  const dailyPoints = $derived(dailyUsagePoints(scopedRows, selectedRange));
  const activeDays = $derived(dailyPoints.filter((point) => point.totals.totalTokens > 0).length);
  const averageTokens = $derived(activeDays > 0 ? Math.round(scopedTotals.totalTokens / activeDays) : 0);
  const peakPoint = $derived(
    dailyPoints.reduce((peak, point) => point.totals.totalTokens > peak.totals.totalTokens ? point : peak, dailyPoints[0]),
  );
  const scopedBySource = $derived(
    boundSources.map((source) => ({
      source,
      totals: aggregateUsage(scopedRows.filter((row) => row.source === source)),
    })),
  );
  const sourcePeak = $derived(Math.max(0, ...scopedBySource.map((item) => item.totals.totalTokens)));
  const chartBackground = $derived(buildSourceGradient(scopedBySource, scopedTotals.totalTokens));
  const scopedModelCosts = $derived(groupUsageCostByModel(scopedRows));
  const navigationLabel = $derived(periodNavigationLabel(selectedRange, period));

  $effect(() => {
    void loadMonths(requestedMonths);
  });

  onMount(() => {
    const closeDatePickerFromOutside = (event: PointerEvent) => {
      if (
        datePickerOpen &&
        event.target instanceof Node &&
        !datePickerRoot?.contains(event.target)
      ) {
        datePickerOpen = false;
      }
    };
    const closeDatePickerFromKeyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && datePickerOpen) {
        datePickerOpen = false;
      }
    };
    document.addEventListener("pointerdown", closeDatePickerFromOutside);
    document.addEventListener("keydown", closeDatePickerFromKeyboard);

    if (preview) {
      return () => {
        document.removeEventListener("pointerdown", closeDatePickerFromOutside);
        document.removeEventListener("keydown", closeDatePickerFromKeyboard);
        closeShare();
      };
    }
    let disposed = false;
    let unlisten = () => {};
    void listen("usage-updated", () => void loadMonths(requestedMonths)).then((next) => {
      if (disposed) next();
      else unlisten = next;
    });
    return () => {
      disposed = true;
      unlisten();
      document.removeEventListener("pointerdown", closeDatePickerFromOutside);
      document.removeEventListener("keydown", closeDatePickerFromKeyboard);
      closeShare();
    };
  });

  async function loadMonths(months: string[]) {
    const sequence = ++loadSequence;
    loading = true;
    loadError = "";
    try {
      const batches = await Promise.all(
        months.map((month) => preview ? previewRows(month, boundSources) : listUsageMonth(month)),
      );
      if (sequence === loadSequence) rows = batches.flat();
    } catch (error) {
      if (sequence === loadSequence) {
        rows = [];
        loadError = error instanceof Error ? error.message : "无法读取用量数据";
      }
    } finally {
      if (sequence === loadSequence) loading = false;
    }
  }

  function changePeriod(amount: number) {
    selectedDate = shiftPeriod(selectedDate, period, amount);
    const [year, month] = selectedDate.split("-").map(Number);
    visibleMonth = new Date(year, month - 1, 1);
  }

  function selectPeriod(next: UsagePeriod) {
    period = next;
    const [year, month] = selectedDate.split("-").map(Number);
    visibleMonth = new Date(year, month - 1, 1);
  }

  function selectDate(date: string) {
    selectedDate = date;
    period = "day";
    const [year, month] = date.split("-").map(Number);
    if (year !== visibleMonth.getFullYear() || month - 1 !== visibleMonth.getMonth()) {
      visibleMonth = new Date(year, month - 1, 1);
    }
  }

  function openDatePicker() {
    if (datePickerOpen) {
      datePickerOpen = false;
      return;
    }
    const [year, month] = selectedDate.split("-").map(Number);
    datePickerMonth = new Date(year, month - 1, 1);
    datePickerOpen = true;
    requestAnimationFrame(() => datePickerDialog?.focus());
  }

  function changeDatePickerMonth(amount: number) {
    datePickerMonth = new Date(
      datePickerMonth.getFullYear(),
      datePickerMonth.getMonth() + amount,
      1,
    );
  }

  function pickDate(date: string) {
    selectDate(date);
    datePickerOpen = false;
  }

  function goToday() {
    visibleMonth = new Date(now.getFullYear(), now.getMonth(), 1);
    selectedDate = todayKey;
    datePickerOpen = false;
  }

  function sourceTokens(cell: { bySource: Partial<Record<UsageSource, UsageTotals>> }, source: UsageSource): number {
    return cell.bySource[source]?.totalTokens ?? 0;
  }

  function hasSourceData(source: UsageSource): boolean {
    return calendarRows.some((row) => row.source === source && row.totalTokens > 0);
  }

  function sourceBarWidth(tokens: number): number {
    if (tokens <= 0 || sourcePeak <= 0) return 0;
    return Math.max(2, (tokens / sourcePeak) * 100);
  }

  function sourceColor(source: UsageSource): string {
    if (source === "codex") return "var(--usage-codex)";
    if (source === "cursor") return "var(--usage-cursor)";
    return "var(--usage-grok)";
  }

  function buildSourceGradient(
    sources: Array<{ source: UsageSource; totals: UsageTotals }>,
    total: number,
  ): string {
    if (total <= 0) return "conic-gradient(var(--settings-border) 0 100%)";
    let position = 0;
    const segments = sources
      .filter((item) => item.totals.totalTokens > 0)
      .map((item) => {
        const start = position;
        position += (item.totals.totalTokens / total) * 100;
        return `${sourceColor(item.source)} ${start.toFixed(2)}% ${position.toFixed(2)}%`;
      });
    return `conic-gradient(${segments.join(", ")})`;
  }

  function sourceCollectionLabel(source: UsageSource): string {
    if (preview) return "界面预览";
    return source === "cursor" ? "个人版待接入" : "自动采集";
  }

  function sourceCost(source: UsageSource) {
    return estimateUsageCost(scopedRows.filter((row) => row.source === source));
  }

  function periodKicker(): string {
    return period === "day" ? "DAY" : period === "week" ? "WEEK" : "MONTH";
  }

  function periodTitle(): string {
    if (period === "day") return selectedDate === todayKey ? "今日 Token 消耗" : "单日 Token 消耗";
    return period === "week" ? "本周 Token 消耗" : "本月 Token 消耗";
  }

  function periodMetricLabel(): string {
    return period === "day" ? "当日 Token" : period === "week" ? "本周 Token" : "本月 Token";
  }

  function periodCostLabel(): string {
    return period === "day" ? "当日估算金额" : period === "week" ? "本周估算金额" : "本月估算金额";
  }

  async function generateShareCard() {
    shareBusy = true;
    shareNote = "";
    try {
      const blob = await renderUsageShareCard({
        period,
        range: selectedRange,
        periodLabel: navigationLabel,
        totals: scopedTotals,
        estimate: scopedCost,
        sources: scopedBySource,
        daily: dailyPoints,
        models: scopedModelCosts,
        activeDays,
      }, brandLogo);
      if (sharePreviewUrl) URL.revokeObjectURL(sharePreviewUrl);
      shareBlob = blob;
      sharePreviewUrl = URL.createObjectURL(blob);
      shareOpen = true;
    } catch (error) {
      shareNote = error instanceof Error ? error.message : "生成战报失败";
    } finally {
      shareBusy = false;
    }
  }

  async function copyShareImage() {
    if (!shareBlob) return;
    try {
      if (!navigator.clipboard?.write || typeof ClipboardItem === "undefined") {
        throw new Error("当前系统不支持直接复制图片");
      }
      await navigator.clipboard.write([new ClipboardItem({ "image/png": shareBlob })]);
      shareNote = "图片已复制，可以直接粘贴分享。";
    } catch (error) {
      shareNote = `${error instanceof Error ? error.message : "复制失败"}，请使用“保存 PNG”。`;
    }
  }

  async function saveShareImage() {
    if (!shareBlob) return;
    try {
      const bytes = Array.from(new Uint8Array(await shareBlob.arrayBuffer()));
      const path = await saveUsageShareImage(
        buildUsageShareFilename({ period, range: selectedRange }),
        bytes,
      );
      shareNote = `已保存到：${path}`;
    } catch (error) {
      shareNote = error instanceof Error ? error.message : "保存图片失败";
    }
  }

  function closeShare() {
    shareOpen = false;
    shareBlob = null;
    shareNote = "";
    if (sharePreviewUrl) URL.revokeObjectURL(sharePreviewUrl);
    sharePreviewUrl = "";
  }

  function localDateKey(value: Date): string {
    const pad = (part: number) => String(part).padStart(2, "0");
    return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`;
  }

  function dayLabel(value: string): string {
    const [year, month, day] = value.split("-").map(Number);
    return new Intl.DateTimeFormat("zh-CN", {
      month: "long",
      day: "numeric",
      weekday: "long",
    }).format(new Date(year, month - 1, day));
  }

  function previewRows(month: string, sources: UsageSource[]): DailyUsage[] {
    const activeSources = sources.length > 0 ? sources : (["codex", "cursor", "grok-cli"] as UsageSource[]);
    const days = [...new Set([2, 3, 5, 7, 8, 11, 12, 14, 17, 18, 21, 24, 26, 28, now.getDate()])]
      .filter((day) => day <= new Date(Number(month.slice(0, 4)), Number(month.slice(5, 7)), 0).getDate())
      .sort((left, right) => left - right);
    return days.flatMap((day, dayIndex) =>
      activeSources
        .filter((_, sourceIndex) => (dayIndex + sourceIndex) % 3 !== 1)
        .map((source, sourceIndex) => {
          const inputTokens = 18_000 + dayIndex * 8_700 + sourceIndex * 13_400;
          const outputTokens = 3_200 + dayIndex * 1_350 + sourceIndex * 1_900;
          return {
            date: `${month}-${String(day).padStart(2, "0")}`,
            source,
            model: source === "codex" ? "gpt-5.6-sol" : source === "grok-cli" ? "grok-4.5" : null,
            contextTier: "short",
            inputTokens,
            cachedInputTokens: Math.round(inputTokens * 0.58),
            outputTokens,
            reasoningTokens: Math.round(outputTokens * 0.42),
            totalTokens: inputTokens + outputTokens,
          };
        }),
    );
  }
</script>

<div class="usage-page">
  <div class="usage-toolbar">
    <div class="navigation-cluster">
      <div class="month-navigation" aria-label="统计周期导航">
        <button class="icon-button" type="button" aria-label="上一个统计周期" onclick={() => changePeriod(-1)}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12 5-5 5 5 5" /></svg>
        </button>
        <span bind:this={datePickerRoot} class="period-date-picker">
          <button
            class="period-date-button"
            type="button"
            aria-label={`选择统计日期，当前${navigationLabel}`}
            aria-haspopup="dialog"
            aria-expanded={datePickerOpen}
            onclick={openDatePicker}
          >
            <span>{navigationLabel}</span>
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <rect x="3.5" y="5" width="13" height="11" rx="2" />
              <path d="M6.5 3.5v3m7-3v3M3.5 8.5h13" />
            </svg>
          </button>
          {#if datePickerOpen}
            <div
              bind:this={datePickerDialog}
              class="date-picker-popover"
              role="dialog"
              aria-label="选择统计日期"
              tabindex="-1"
            >
              <div class="date-picker-heading">
                <div>
                  <span>选择日期</span>
                  <strong>{monthLabel(datePickerMonth)}</strong>
                </div>
                <div class="date-picker-month-actions">
                  <button type="button" aria-label="上一个月" onclick={() => changeDatePickerMonth(-1)}>
                    <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12 5-5 5 5 5" /></svg>
                  </button>
                  <button type="button" aria-label="下一个月" onclick={() => changeDatePickerMonth(1)}>
                    <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m8 5 5 5-5 5" /></svg>
                  </button>
                </div>
              </div>
              <div class="date-picker-weekdays" aria-hidden="true">
                {#each ["一", "二", "三", "四", "五", "六", "日"] as weekday}
                  <span>{weekday}</span>
                {/each}
              </div>
              <div class="date-picker-grid" role="grid" aria-label={monthLabel(datePickerMonth)}>
                {#each datePickerCells as cell}
                  <button
                    type="button"
                    class="date-picker-day"
                    class:outside={!cell.inMonth}
                    class:today={cell.isToday}
                    class:selected={selectedDate === cell.date}
                    aria-label={cell.date}
                    aria-current={cell.isToday ? "date" : undefined}
                    aria-pressed={selectedDate === cell.date}
                    onclick={() => pickDate(cell.date)}
                  >
                    <span>{cell.day}</span>
                  </button>
                {/each}
              </div>
              <div class="date-picker-footer">
                <span>选择后自动切换为日统计</span>
                <button type="button" onclick={() => pickDate(todayKey)}>今天</button>
              </div>
            </div>
          {/if}
        </span>
        <button class="icon-button" type="button" aria-label="下一个统计周期" onclick={() => changePeriod(1)}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m8 5 5 5-5 5" /></svg>
        </button>
      </div>
      <div class="period-switch" role="group" aria-label="统计周期">
        {#each [["day", "日"], ["week", "周"], ["month", "月"]] as option}
          <button
            type="button"
            class:active={period === option[0]}
            aria-pressed={period === option[0]}
            onclick={() => selectPeriod(option[0] as UsagePeriod)}
          >{option[1]}</button>
        {/each}
      </div>
    </div>
    <div class="toolbar-actions">
      <button class="share-button" type="button" disabled={shareBusy} onclick={() => void generateShareCard()}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3v9m0-9L6.5 6.5M10 3l3.5 3.5M5 10v5h10v-5" /></svg>
        {shareBusy ? "生成中…" : "分享战报"}
      </button>
      <button class="quiet-button" type="button" onclick={goToday}>回到今天</button>
    </div>
  </div>

  <section class="today-card" aria-label={`${navigationLabel} Token 消耗图表`}>
    <div class="today-heading">
      <div>
        <span class="section-kicker">{periodKicker()}</span>
        <h2>{periodTitle()}</h2>
        <p>{navigationLabel} · 本地日志自动汇总</p>
      </div>
      <div class="today-badges">
        {#if scopedCost.pricedTokens > 0}
          <span class="cost-badge">API 等价 ≈ {formatEstimatedRmb(scopedCost)}</span>
        {/if}
        <span class="live-badge"><i></i>{preview ? "预览数据" : "实时采集"}</span>
      </div>
    </div>

    <div class="today-chart-layout">
      <div class="donut-chart" style:--chart-background={chartBackground} aria-label={`${navigationLabel}共 ${scopedTotals.totalTokens} Token`}>
        <div>
          <span>周期总量</span>
          <strong>{formatCompactTokens(scopedTotals.totalTokens)}</strong>
          <small>Token</small>
        </div>
      </div>

      <div class="source-chart" aria-label="周期内各工具 Token 消耗">
        {#each scopedBySource as item}
          <div class="source-chart-row">
            <div class="source-chart-label">
              <span class:codex={item.source === "codex"} class:cursor={item.source === "cursor"} class:grok={item.source === "grok-cli"}>
                <i></i>{USAGE_SOURCE_META[item.source].label}
              </span>
              <em>{sourceCollectionLabel(item.source)}</em>
              <strong>{formatCompactTokens(item.totals.totalTokens)}</strong>
            </div>
            <div class="bar-track" aria-hidden="true">
              <i
                class:codex-bar={item.source === "codex"}
                class:cursor-bar={item.source === "cursor"}
                class:grok-bar={item.source === "grok-cli"}
                style:width={`${sourceBarWidth(item.totals.totalTokens)}%`}
              ></i>
            </div>
          </div>
        {/each}
        {#if boundSources.length === 0}
          <p class="today-empty">绑定 Codex 或 Grok CLI 后，这里会展示今日实时消耗。</p>
        {/if}
      </div>
    </div>

    <div class="token-type-grid" aria-label="周期 Token 类型明细">
      <span><i>输入</i><strong>{formatCompactTokens(scopedTotals.inputTokens)}</strong></span>
      <span><i>缓存命中</i><strong>{formatCompactTokens(scopedTotals.cachedInputTokens)}</strong></span>
      <span><i>输出</i><strong>{formatCompactTokens(scopedTotals.outputTokens)}</strong></span>
      <span><i>推理</i><strong>{formatCompactTokens(scopedTotals.reasoningTokens)}</strong></span>
    </div>
  </section>

  <div class="metric-grid" aria-label={`${navigationLabel}用量摘要`}>
    <article class="metric-card primary">
      <span>{periodMetricLabel()}</span>
      <strong>{formatCompactTokens(scopedTotals.totalTokens)}</strong>
      <small>{activeDays > 0 ? `${activeDays} 个活跃日` : "等待第一条用量记录"}</small>
    </article>
    <article class="metric-card">
      <span>{period === "day" ? "缓存命中率" : "日均 Token"}</span>
      <strong>{period === "day" ? scopedTotals.inputTokens > 0 ? `${Math.round(scopedTotals.cachedInputTokens / scopedTotals.inputTokens * 100)}%` : "--" : formatCompactTokens(averageTokens)}</strong>
      <small>{period === "day" ? "缓存输入 / 输入 Token" : "按活跃日期计算"}</small>
    </article>
    <article class="metric-card">
      <span>{period === "day" ? "输出 Token" : "单日峰值"}</span>
      <strong>{formatCompactTokens(period === "day" ? scopedTotals.outputTokens : peakPoint?.totals.totalTokens ?? 0)}</strong>
      <small>{period === "day" ? `其中推理 ${formatCompactTokens(scopedTotals.reasoningTokens)}` : peakPoint?.totals.totalTokens ? dayLabel(peakPoint.date) : "暂无数据"}</small>
    </article>
    <article class="metric-card cost-card">
      <span>{periodCostLabel()}</span>
      <strong>{formatEstimatedRmb(scopedCost)}</strong>
      <small>{formatCostCoverage(scopedCost)} · $1≈¥{USD_CNY_ESTIMATE.toFixed(2)}</small>
    </article>
  </div>

  <section class="calendar-card" aria-label="Token 用量日历">
    <div class="calendar-heading">
      <div class="source-legend" aria-label="统计来源">
        {#each boundSources as source}
          <span class:codex={source === "codex"} class:cursor={source === "cursor"} class:grok={source === "grok-cli"}>
            <i></i>{USAGE_SOURCE_META[source].label}
            <em>{sourceCollectionLabel(source)} · {hasSourceData(source) ? "已有数据" : "等待数据"}</em>
          </span>
        {/each}
        {#if boundSources.length === 0}
          <span class="muted-source">尚未绑定支持 Token 统计的工具</span>
        {/if}
      </div>
      {#if preview}<span class="preview-badge">界面预览数据</span>{/if}
    </div>

    <div class="weekdays" aria-hidden="true">
      {#each ["一", "二", "三", "四", "五", "六", "日"] as weekday}
        <span>{weekday}</span>
      {/each}
    </div>

    <div class="calendar-grid" aria-busy={loading}>
      {#each cells as cell}
        <button
          type="button"
          class="day-cell"
          class:outside={!cell.inMonth}
          class:today={cell.isToday}
          class:selected={selectedDate === cell.date}
          class:in-period={rangeIncludes(selectedRange, cell.date)}
          class:has-usage={cell.totals.totalTokens > 0}
          aria-label={`${cell.date}，${cell.totals.totalTokens > 0 ? `${cell.totals.totalTokens} Token` : "无用量记录"}`}
          onclick={() => selectDate(cell.date)}
        >
          <span class="day-number">{cell.day}</span>
          {#if cell.totals.totalTokens > 0}
            <strong>{formatCompactTokens(cell.totals.totalTokens)}</strong>
            <span class="source-bars" aria-hidden="true">
              {#each boundSources as source}
                {#if sourceTokens(cell, source) > 0}
                  <i
                    class:codex={source === "codex"}
                    class:cursor={source === "cursor"}
                    class:grok={source === "grok-cli"}
                    style:flex-grow={sourceTokens(cell, source)}
                  ></i>
                {/if}
              {/each}
            </span>
          {:else}
            <span class="empty-mark">·</span>
          {/if}
        </button>
      {/each}
    </div>

    {#if loading}
      <p class="calendar-note">正在读取 {monthLabel(visibleMonth)} 的数据…</p>
    {:else if loadError}
      <p class="calendar-note error">读取失败：{loadError}</p>
    {:else if calendarTotals.totalTokens === 0}
      <div class="empty-state">
        <span class="empty-icon">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3v3m10-3v3M4.5 9h15M6 5h12a2 2 0 0 1 2 2v12H4V7a2 2 0 0 1 2-2Z" /></svg>
        </span>
        <div>
          <strong>本月尚无 Token 记录</strong>
          <p>Codex 与 Grok CLI 会从本地结构化日志自动汇总；Cursor 个人版暂不计入。</p>
        </div>
      </div>
    {/if}
  </section>

  <section class="day-detail" aria-label="选中统计周期详情">
    <div class="day-detail-title">
      <div>
        <span>周期详情</span>
        <strong>{navigationLabel}</strong>
      </div>
      <div class="day-detail-totals">
        <b>{formatCompactTokens(scopedTotals.totalTokens)}</b>
        <small>API 等价 ≈ {formatEstimatedRmb(scopedCost)}</small>
      </div>
    </div>
    <div class="breakdown-grid">
      {#each scopedBySource as item}
        <article>
          <span class="tool-name" class:codex={item.source === "codex"} class:cursor={item.source === "cursor"} class:grok={item.source === "grok-cli"}>
            <i></i>{USAGE_SOURCE_META[item.source].label}
          </span>
          <strong>{formatCompactTokens(item.totals.totalTokens)}</strong>
          <small>
            {#if item.totals.totalTokens > 0}
              输入 {formatCompactTokens(item.totals.inputTokens)} · 输出 {formatCompactTokens(item.totals.outputTokens)} · ≈ {formatEstimatedRmb(sourceCost(item.source))}
            {:else}
              周期内无记录
            {/if}
          </small>
        </article>
      {/each}
    </div>
    {#if scopedModelCosts.length > 0}
      <div class="model-cost-heading">
        <span>模型明细</span>
        <small>按公开 API 标准价格估算，订阅套餐实际账单可能不同</small>
      </div>
      <div class="model-cost-list">
        {#each scopedModelCosts as item}
          <article>
            <div>
              <strong>{item.model}</strong>
              <small>{USAGE_SOURCE_META[item.source].label}</small>
            </div>
            <span>{formatCompactTokens(item.totals.totalTokens)}</span>
            <b>{formatEstimatedRmb(item.estimate)}</b>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  {#if shareOpen && sharePreviewUrl}
    <div class="share-overlay" role="presentation" onclick={(event) => event.currentTarget === event.target && closeShare()}>
      <div class="share-dialog" role="dialog" aria-modal="true" aria-label="分享战报预览">
        <div class="share-dialog-heading">
          <div><span>SHARE YOUR MOMENTUM</span><strong>AI 生产力战报</strong><small>{navigationLabel} · 已隐藏提示词与对话内容</small></div>
          <button class="share-close" type="button" aria-label="关闭分享预览" onclick={closeShare}>×</button>
        </div>
        <div class="share-preview"><img src={sharePreviewUrl} alt={`${navigationLabel} AI 生产力战报预览`} /></div>
        <div class="share-actions">
          <button class="quiet-button" type="button" onclick={() => void copyShareImage()}>复制图片</button>
          <button class="share-button primary-share" type="button" onclick={() => void saveShareImage()}>保存 PNG</button>
        </div>
        {#if shareNote}<p class="share-note">{shareNote}</p>{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .usage-page {
    display: grid;
    gap: 16px;
  }

  .usage-toolbar,
  .calendar-heading,
  .month-navigation,
  .navigation-cluster,
  .toolbar-actions,
  .period-switch,
  .source-legend,
  .day-detail-title,
  .tool-name {
    display: flex;
    align-items: center;
  }

  .usage-toolbar,
  .calendar-heading,
  .day-detail-title {
    justify-content: space-between;
  }

  .month-navigation {
    gap: 8px;
  }

  .navigation-cluster,
  .toolbar-actions {
    gap: 9px;
  }

  .period-date-picker {
    position: relative;
    z-index: 20;
  }

  .period-date-button {
    min-width: 176px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 36px;
    padding: 7px 12px;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--sc-text);
    text-align: center;
    font-size: 15px;
    font-weight: 680;
    cursor: pointer;
    transition: border-color 150ms ease, background 150ms ease, color 150ms ease;
  }

  .period-date-button:hover {
    border-color: var(--settings-border);
    background: var(--settings-hover);
    color: var(--settings-accent);
  }

  .period-date-button:focus-visible {
    outline: 2px solid var(--settings-accent);
    outline-offset: 2px;
  }

  .period-date-button svg {
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.7;
  }

  .date-picker-popover {
    position: absolute;
    z-index: 30;
    top: calc(100% + 10px);
    left: 0;
    width: 304px;
    padding: 14px;
    border: 1px solid color-mix(in srgb, var(--settings-accent) 22%, var(--settings-border));
    border-radius: 17px;
    outline: none;
    background:
      radial-gradient(circle at 12% 0%, color-mix(in srgb, var(--settings-accent) 11%, transparent), transparent 42%),
      var(--settings-card);
    box-shadow: 0 22px 58px rgb(0 0 0 / 32%), 0 2px 10px rgb(0 0 0 / 12%);
    animation: date-picker-in 150ms ease-out;
  }

  @keyframes date-picker-in {
    from {
      opacity: 0;
      transform: translateY(-5px) scale(0.985);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .date-picker-heading,
  .date-picker-month-actions,
  .date-picker-footer {
    display: flex;
    align-items: center;
  }

  .date-picker-heading,
  .date-picker-footer {
    justify-content: space-between;
  }

  .date-picker-heading > div:first-child {
    display: grid;
    gap: 2px;
  }

  .date-picker-heading > div:first-child span {
    color: var(--settings-accent);
    font-size: 8px;
    font-weight: 760;
    letter-spacing: 0.12em;
  }

  .date-picker-heading strong {
    color: var(--sc-text);
    font-size: 14px;
    letter-spacing: -0.01em;
  }

  .date-picker-month-actions {
    gap: 5px;
  }

  .date-picker-month-actions button {
    display: grid;
    width: 29px;
    height: 29px;
    padding: 0;
    place-items: center;
    border: 1px solid var(--settings-border);
    border-radius: 9px;
    background: var(--settings-control);
    color: var(--sc-muted);
    cursor: pointer;
  }

  .date-picker-month-actions button:hover {
    border-color: color-mix(in srgb, var(--settings-accent) 32%, var(--settings-border));
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
  }

  .date-picker-month-actions svg {
    width: 14px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.8;
  }

  .date-picker-weekdays,
  .date-picker-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
  }

  .date-picker-weekdays {
    margin-top: 13px;
    padding: 0 1px 6px;
    border-bottom: 1px solid var(--settings-border);
  }

  .date-picker-weekdays span {
    color: color-mix(in srgb, var(--sc-muted) 76%, transparent);
    text-align: center;
    font-size: 8px;
    font-weight: 650;
  }

  .date-picker-grid {
    gap: 3px;
    padding-top: 7px;
  }

  .date-picker-day {
    display: grid;
    aspect-ratio: 1;
    min-width: 0;
    padding: 0;
    place-items: center;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--sc-text);
    cursor: pointer;
    font-size: 10px;
    font-weight: 590;
    transition: border-color 120ms ease, background 120ms ease, color 120ms ease, transform 120ms ease;
  }

  .date-picker-day:hover {
    border-color: var(--settings-border-strong);
    background: var(--settings-hover);
    color: var(--settings-accent);
    transform: translateY(-1px);
  }

  .date-picker-day.outside {
    color: color-mix(in srgb, var(--sc-muted) 38%, transparent);
  }

  .date-picker-day.today:not(.selected) {
    border-color: color-mix(in srgb, var(--settings-accent) 42%, transparent);
    color: var(--settings-accent);
    font-weight: 720;
  }

  .date-picker-day.selected {
    border-color: var(--settings-accent);
    background: var(--settings-accent);
    color: var(--settings-accent-text);
    box-shadow: 0 5px 13px color-mix(in srgb, var(--settings-accent) 24%, transparent);
    font-weight: 760;
  }

  .date-picker-footer {
    gap: 12px;
    margin-top: 9px;
    padding: 10px 2px 0;
    border-top: 1px solid var(--settings-border);
  }

  .date-picker-footer span {
    color: var(--sc-muted);
    font-size: 8px;
  }

  .date-picker-footer button {
    padding: 5px 9px;
    border: 0;
    border-radius: 8px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
  }

  .date-picker-footer button:hover {
    background: var(--settings-accent);
    color: var(--settings-accent-text);
  }

  .period-switch {
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--settings-border);
    border-radius: 11px;
    background: var(--settings-surface);
  }

  .period-switch button {
    min-width: 35px;
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--sc-muted);
    cursor: pointer;
    font-size: 9px;
    font-weight: 650;
  }

  .period-switch button:hover {
    color: var(--sc-text);
  }

  .period-switch button.active {
    background: var(--settings-accent);
    color: var(--settings-accent-text);
    box-shadow: 0 4px 11px color-mix(in srgb, var(--settings-accent) 20%, transparent);
  }

  button {
    font: inherit;
  }

  .icon-button,
  .quiet-button,
  .share-button {
    border: 1px solid var(--settings-border);
    background: var(--settings-control);
    color: var(--sc-text);
    cursor: pointer;
  }

  .icon-button {
    display: grid;
    width: 32px;
    height: 32px;
    padding: 0;
    place-items: center;
    border-radius: 10px;
  }

  .icon-button svg {
    width: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
  }

  .quiet-button {
    padding: 7px 12px;
    border-radius: 9px;
  }

  .share-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-color: color-mix(in srgb, var(--settings-accent) 28%, var(--settings-border));
    border-radius: 9px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
    font-size: 9px;
    font-weight: 680;
  }

  .share-button svg {
    width: 14px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.6;
  }

  .share-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .icon-button:hover,
  .quiet-button:hover,
  .share-button:hover {
    border-color: color-mix(in srgb, var(--settings-accent) 42%, var(--settings-border));
    background: var(--settings-accent-soft);
  }

  .today-card {
    padding: 17px 18px 14px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--settings-accent) 24%, var(--settings-border));
    border-radius: 17px;
    background:
      radial-gradient(circle at 18% 0%, color-mix(in srgb, var(--settings-accent) 13%, transparent), transparent 42%),
      var(--settings-card);
    box-shadow: var(--settings-card-shadow);
  }

  .today-heading,
  .today-chart-layout,
  .source-chart-label,
  .token-type-grid span,
  .today-badges,
  .live-badge {
    display: flex;
    align-items: center;
  }

  .today-heading {
    justify-content: space-between;
  }

  .today-heading > div {
    display: grid;
    gap: 2px;
  }

  .section-kicker {
    color: var(--settings-accent);
    font-size: 8px;
    font-weight: 750;
    letter-spacing: 0.15em;
  }

  .today-heading h2 {
    margin: 0;
    font-size: 15px;
    letter-spacing: -0.02em;
  }

  .today-heading p {
    margin: 0;
    color: var(--sc-muted);
    font-size: 9px;
  }

  .live-badge {
    gap: 6px;
    padding: 5px 8px;
    border: 1px solid color-mix(in srgb, var(--sc-success) 24%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--sc-success) 9%, transparent);
    color: var(--sc-success);
    font-size: 9px;
  }

  .today-badges {
    justify-content: flex-end;
    gap: 7px;
  }

  .cost-badge {
    padding: 5px 8px;
    border: 1px solid color-mix(in srgb, var(--settings-accent) 26%, transparent);
    border-radius: 999px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
    font-size: 9px;
    font-weight: 650;
  }

  .live-badge i {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: currentColor;
    box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 13%, transparent);
  }

  .today-chart-layout {
    gap: 28px;
    margin-top: 15px;
  }

  .donut-chart {
    --chart-background: conic-gradient(var(--settings-border) 0 100%);
    position: relative;
    display: grid;
    flex: 0 0 138px;
    width: 138px;
    height: 138px;
    place-items: center;
    border-radius: 50%;
    background: var(--chart-background);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--settings-border) 55%, transparent);
  }

  .donut-chart::after {
    position: absolute;
    width: 96px;
    height: 96px;
    border-radius: inherit;
    background: var(--settings-card);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--settings-border) 70%, transparent);
    content: "";
  }

  .donut-chart > div {
    z-index: 1;
    display: grid;
    justify-items: center;
  }

  .donut-chart span,
  .donut-chart small {
    color: var(--sc-muted);
    font-size: 9px;
  }

  .donut-chart strong {
    margin: 3px 0 1px;
    font-size: 22px;
    line-height: 1;
    letter-spacing: -0.04em;
  }

  .source-chart {
    display: grid;
    flex: 1;
    gap: 13px;
    min-width: 0;
  }

  .source-chart-row {
    display: grid;
    gap: 6px;
  }

  .source-chart-label {
    min-width: 0;
    gap: 8px;
    font-size: 10px;
  }

  .source-chart-label > span {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 64px;
    font-weight: 650;
  }

  .source-chart-label > span i {
    width: 7px;
    height: 7px;
    border-radius: 99px;
    background: currentColor;
  }

  .source-chart-label em {
    overflow: hidden;
    color: var(--sc-muted);
    font-size: 8px;
    font-style: normal;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-chart-label strong {
    margin-left: auto;
    font-size: 11px;
  }

  .bar-track {
    height: 7px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--settings-surface);
  }

  .bar-track i {
    display: block;
    height: 100%;
    border-radius: inherit;
    transition: width 220ms ease;
  }

  .codex-bar { background: var(--usage-codex); }
  .cursor-bar { background: var(--usage-cursor); }
  .grok-bar { background: var(--usage-grok); }

  .today-empty {
    align-self: center;
    margin: 0;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .token-type-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--settings-border);
  }

  .token-type-grid span {
    justify-content: space-between;
    gap: 8px;
    padding: 7px 9px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--settings-surface) 76%, transparent);
  }

  .token-type-grid i {
    color: var(--sc-muted);
    font-size: 8px;
    font-style: normal;
  }

  .token-type-grid strong {
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: 1.2fr repeat(3, 1fr);
    gap: 10px;
  }

  .metric-card,
  .calendar-card,
  .day-detail {
    border: 1px solid var(--settings-border);
    background: var(--settings-card);
    box-shadow: var(--settings-card-shadow);
  }

  .metric-card {
    display: grid;
    min-height: 96px;
    padding: 14px 15px;
    border-radius: 14px;
  }

  .metric-card.primary {
    border-color: color-mix(in srgb, var(--settings-accent) 28%, var(--settings-border));
    background:
      radial-gradient(circle at 100% 0%, color-mix(in srgb, var(--settings-accent) 18%, transparent), transparent 58%),
      var(--settings-card);
  }

  .metric-card span,
  .day-detail-title span {
    color: var(--sc-muted);
    font-size: 11px;
  }

  .metric-card strong {
    margin-top: 6px;
    font-size: 23px;
    line-height: 1;
    letter-spacing: -0.03em;
  }

  .metric-card small {
    align-self: end;
    margin-top: 8px;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .cost-card strong {
    color: var(--settings-accent);
  }

  .calendar-card {
    padding: 14px;
    border-radius: 16px;
  }

  .calendar-heading {
    min-height: 28px;
    margin-bottom: 10px;
  }

  .source-legend {
    flex-wrap: wrap;
    gap: 8px 14px;
  }

  .source-legend > span,
  .tool-name {
    gap: 6px;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .source-legend i,
  .tool-name i {
    width: 7px;
    height: 7px;
    border-radius: 99px;
    background: currentColor;
  }

  .source-legend em {
    padding-left: 2px;
    color: color-mix(in srgb, var(--sc-muted) 72%, transparent);
    font-size: 9px;
    font-style: normal;
  }

  .codex { color: var(--usage-codex) !important; }
  .cursor { color: var(--usage-cursor) !important; }
  .grok { color: var(--usage-grok) !important; }

  .preview-badge {
    padding: 4px 7px;
    border: 1px solid color-mix(in srgb, var(--settings-accent) 22%, transparent);
    border-radius: 999px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
    font-size: 9px;
  }

  .weekdays,
  .calendar-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
  }

  .weekdays {
    border-top: 1px solid var(--settings-border);
    border-bottom: 1px solid var(--settings-border);
  }

  .weekdays span {
    padding: 7px 0;
    color: var(--sc-muted);
    text-align: center;
    font-size: 9px;
  }

  .calendar-grid {
    gap: 5px;
    padding-top: 7px;
  }

  .day-cell {
    position: relative;
    display: grid;
    min-width: 0;
    min-height: 58px;
    padding: 7px;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--sc-text);
    text-align: left;
    cursor: pointer;
    transition: border-color 140ms ease, background 140ms ease, transform 140ms ease;
  }

  .day-cell:hover {
    border-color: var(--settings-border);
    background: var(--settings-hover);
  }

  .day-cell.selected {
    border-color: color-mix(in srgb, var(--settings-accent) 52%, transparent);
    background: var(--settings-accent-soft);
  }

  .day-cell.in-period:not(.selected) {
    border-color: color-mix(in srgb, var(--settings-accent) 16%, transparent);
    background: color-mix(in srgb, var(--settings-accent) 4.5%, transparent);
  }

  .day-cell.outside {
    opacity: 0.34;
  }

  .day-number {
    display: grid;
    width: 19px;
    height: 19px;
    place-items: center;
    border-radius: 7px;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .day-cell.today .day-number {
    background: var(--settings-accent);
    color: var(--settings-accent-text);
    font-weight: 700;
  }

  .day-cell strong {
    align-self: end;
    overflow: hidden;
    font-size: 11px;
    text-overflow: ellipsis;
  }

  .empty-mark {
    align-self: end;
    color: color-mix(in srgb, var(--sc-muted) 38%, transparent);
  }

  .source-bars {
    display: flex;
    gap: 2px;
    height: 3px;
    margin-top: 4px;
  }

  .source-bars i {
    min-width: 4px;
    border-radius: 99px;
    background: currentColor;
  }

  .calendar-note {
    margin: 10px 2px 0;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .calendar-note.error {
    color: var(--sc-failed);
  }

  .empty-state {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 10px;
    padding: 12px;
    border-radius: 12px;
    background: var(--settings-surface);
  }

  .empty-icon {
    display: grid;
    flex: 0 0 34px;
    height: 34px;
    place-items: center;
    border-radius: 10px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
  }

  .empty-icon svg {
    width: 18px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.6;
  }

  .empty-state strong {
    font-size: 11px;
  }

  .empty-state p {
    margin: 3px 0 0;
    color: var(--sc-muted);
    font-size: 10px;
    line-height: 1.45;
  }

  .day-detail {
    padding: 14px 15px;
    border-radius: 16px;
  }

  .day-detail-title > div {
    display: grid;
    gap: 2px;
  }

  .day-detail-title strong {
    font-size: 13px;
  }

  .day-detail-title b {
    font-size: 18px;
    letter-spacing: -0.02em;
  }

  .day-detail-totals {
    display: grid;
    justify-items: end;
    gap: 3px;
  }

  .day-detail-totals small {
    color: var(--settings-accent);
    font-size: 9px;
  }

  .breakdown-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin-top: 12px;
  }

  .breakdown-grid article {
    display: grid;
    min-width: 0;
    padding: 10px;
    border-radius: 10px;
    background: var(--settings-surface);
  }

  .breakdown-grid strong {
    margin-top: 8px;
    font-size: 14px;
  }

  .breakdown-grid small {
    overflow: hidden;
    margin-top: 3px;
    color: var(--sc-muted);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-cost-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--settings-border);
  }

  .model-cost-heading span {
    font-size: 10px;
    font-weight: 680;
  }

  .model-cost-heading small {
    color: var(--sc-muted);
    font-size: 8px;
    text-align: right;
  }

  .model-cost-list {
    display: grid;
    gap: 6px;
    margin-top: 8px;
  }

  .model-cost-list article {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(72px, auto);
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--settings-surface);
  }

  .model-cost-list article > div {
    display: grid;
    min-width: 0;
  }

  .model-cost-list strong {
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-cost-list small,
  .model-cost-list span {
    color: var(--sc-muted);
    font-size: 9px;
  }

  .model-cost-list b {
    color: var(--settings-accent);
    font-size: 11px;
    text-align: right;
  }

  .share-overlay {
    position: fixed;
    z-index: 100;
    inset: 0;
    display: grid;
    padding: 24px;
    place-items: center;
    background: rgb(4 5 6 / 72%);
    backdrop-filter: blur(12px);
  }

  .share-dialog {
    display: grid;
    width: min(100%, 720px);
    max-height: calc(100vh - 48px);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--settings-accent) 28%, var(--settings-border));
    border-radius: 20px;
    background: var(--settings-card);
    box-shadow: 0 28px 90px rgb(0 0 0 / 46%);
  }

  .share-dialog-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 17px 18px 13px;
    border-bottom: 1px solid var(--settings-border);
  }

  .share-dialog-heading > div {
    display: grid;
    gap: 2px;
  }

  .share-dialog-heading span {
    color: var(--settings-accent);
    font-size: 7.5px;
    font-weight: 760;
    letter-spacing: 0.13em;
  }

  .share-dialog-heading strong {
    font-size: 15px;
  }

  .share-dialog-heading small {
    color: var(--sc-muted);
    font-size: 8.5px;
  }

  .share-close {
    display: grid;
    width: 29px;
    height: 29px;
    padding: 0 0 2px;
    place-items: center;
    border: 1px solid var(--settings-border);
    border-radius: 9px;
    background: var(--settings-control);
    color: var(--sc-muted);
    cursor: pointer;
    font-size: 19px;
  }

  .share-preview {
    display: grid;
    min-height: 0;
    padding: 16px;
    overflow: auto;
    place-items: center;
    background:
      radial-gradient(circle at 50% 0%, color-mix(in srgb, var(--settings-accent) 15%, transparent), transparent 50%),
      var(--settings-surface);
  }

  .share-preview img {
    display: block;
    width: auto;
    max-width: 100%;
    max-height: 470px;
    border-radius: 13px;
    box-shadow: 0 18px 48px rgb(0 0 0 / 34%);
  }

  .share-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 13px 17px;
    border-top: 1px solid var(--settings-border);
  }

  .primary-share {
    border-color: var(--settings-accent);
    background: var(--settings-accent);
    color: var(--settings-accent-text);
  }

  .share-note {
    margin: -5px 17px 13px;
    overflow-wrap: anywhere;
    color: var(--sc-muted);
    font-size: 8.5px;
    text-align: right;
  }

  @media (max-width: 760px) {
    .usage-toolbar,
    .navigation-cluster {
      align-items: stretch;
      flex-direction: column;
    }

    .toolbar-actions {
      justify-content: flex-end;
    }

    .month-navigation {
      justify-content: space-between;
    }

    .period-date-button {
      min-width: 0;
    }

    .period-switch {
      align-self: flex-start;
    }

    .today-heading {
      align-items: flex-start;
      gap: 10px;
    }

    .today-badges {
      flex-wrap: wrap;
    }

    .today-chart-layout {
      align-items: flex-start;
      gap: 18px;
    }

    .donut-chart {
      flex-basis: 112px;
      width: 112px;
      height: 112px;
    }

    .donut-chart::after {
      width: 78px;
      height: 78px;
    }

    .token-type-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .metric-grid {
      grid-template-columns: 1fr;
    }

    .breakdown-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
