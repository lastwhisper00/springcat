<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getWidgetTodayUsage, getWidgetNetwork, type TodayUsage, type NetworkSnapshot } from "$services/tauri";
  import { foreignReachabilityLabel, formatWidgetTokens, networkTypeLabel, networkNote } from "./widget-format";

  let { active = true }: { active?: boolean } = $props();
  const native = isTauri();
  let usage = $state<TodayUsage | null>(null);
  let network = $state<NetworkSnapshot | null>(null);
  let usageError = $state(false);
  let networkError = $state(false);
  const usageNote = $derived(!native ? "请在桌面端查看" : usageError ? "读取失败，稍后重试"
    : !usage ? "正在读取…" : usage.totalTokens === 0 ? "今日暂无采集记录" : "本机已采集用量");
  const usageDetails = $derived(usage
    ? `${usage.date}（本地日期）\n总量 ${usage.totalTokens.toLocaleString("zh-CN")} Token\n输入 ${usage.inputTokens.toLocaleString("zh-CN")} · 输出 ${usage.outputTokens.toLocaleString("zh-CN")}\n仅统计本机已采集的来源，不代表全部账户用量。`
    : "按本地日期汇总已采集的 Token 用量");
  const networkDetails = $derived(network
    ? `海外网络：${foreignReachabilityLabel(network)}\n延迟：${networkNote(network)}\n检测时间 ${new Date(network.checkedAt).toLocaleTimeString("zh-CN")}\n结果由两个海外 HTTPS 站点共同验证，不代表账号、地区或具体功能均可使用。`
    : "通过两个海外 HTTPS 站点验证可达性，延迟取首个成功响应。\n检测目标不会显示在界面中。");

  $effect(() => {
    if (!active || !native) return;
    let disposed = false;
    let usageBusy = false;
    let networkBusy = false;
    let unlisten = () => {};
    let usageTimer: ReturnType<typeof setTimeout> | undefined;

    async function refreshUsage() {
      if (disposed || usageBusy || document.hidden) return;
      usageBusy = true;
      try {
        const result = await getWidgetTodayUsage();
        if (!disposed) { usage = result; usageError = false; }
      } catch {
        if (!disposed) { usage = null; usageError = true; }
      } finally { usageBusy = false; }
    }
    async function refreshNetwork() {
      if (disposed || networkBusy || document.hidden) return;
      networkBusy = true;
      try {
        const result = await getWidgetNetwork();
        if (!disposed) { network = result; networkError = false; }
      } catch {
        if (!disposed) { network = null; networkError = true; }
      } finally { networkBusy = false; }
    }
    function refresh() { void refreshUsage(); void refreshNetwork(); }
    function queueUsageRefresh() {
      if (usageTimer) clearTimeout(usageTimer);
      usageTimer = setTimeout(() => void refreshUsage(), 400);
    }
    refresh();
    const timer = setInterval(refresh, 30_000);
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("online", refresh);
    window.addEventListener("offline", refresh);
    void listen("usage-updated", queueUsageRefresh).then((stop) => {
      if (disposed) stop(); else unlisten = stop;
    }).catch(() => { /* Periodic refresh remains available if event registration fails. */ });
    return () => {
      disposed = true;
      clearInterval(timer);
      if (usageTimer) clearTimeout(usageTimer);
      unlisten();
      document.removeEventListener("visibilitychange", refresh);
      window.removeEventListener("online", refresh);
      window.removeEventListener("offline", refresh);
    };
  });

  function keepPanelOpen(event: Event) {
    event.stopPropagation();
  }

  function isolateKeys(event: KeyboardEvent) {
    if (event.key !== "Escape") event.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Keyboard-scrollable region; handlers only isolate content from the draggable shell.) -->
<section
  class="widget-panel"
  aria-label="小组件概览"
  tabindex="0"
  onpointerdown={keepPanelOpen}
  onclick={keepPanelOpen}
  ondblclick={keepPanelOpen}
  onkeydown={isolateKeys}
>
  <header class="widget-heading">
    <h2>概览</h2>
    <p>{native ? "本地数据 · 每 30 秒刷新" : "实时数据请在桌面端查看"}</p>
  </header>

  <div class="widget-grid">
    <article class="widget usage" aria-labelledby="usage-widget-title">
      <h3 id="usage-widget-title">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M4 14V9m6 5V4m6 10v-7M3 17h14" />
        </svg>
        今日 Token 用量
      </h3>
      <div class="primary-value" title={usageDetails} aria-label={usage ? `今日已采集 ${usage.totalTokens} Token` : usageNote}>
        <strong>{formatWidgetTokens(usage?.totalTokens)}</strong><span>Token</span>
      </div>
      <dl class="compact-stats" title={usageDetails}>
        <div><dt>输入</dt><dd>{formatWidgetTokens(usage?.inputTokens)}</dd></div>
        <div><dt>输出</dt><dd>{formatWidgetTokens(usage?.outputTokens)}</dd></div>
      </dl>
      {#if usageError || !usage}<p class="inline-state" role="status">{usageNote}</p>{/if}
    </article>

    <article class="widget network" aria-labelledby="network-widget-title">
      <h3 id="network-widget-title">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M2.5 7a11.5 11.5 0 0 1 15 0M5.2 10a7.5 7.5 0 0 1 9.6 0M8 13a3.4 3.4 0 0 1 4 0" />
          <circle cx="10" cy="16" r=".6" />
        </svg>
        网络环境
      </h3>
      <div class="primary-value reachability" title={networkDetails} aria-label={`海外网络${networkError ? "检测失败" : foreignReachabilityLabel(network)}`}>
        <i aria-hidden="true" data-reachable={network?.foreignReachable === true}></i>
        <strong>{networkError ? "不可用" : foreignReachabilityLabel(network)}</strong><span>海外网络</span>
      </div>
      <dl class="compact-stats" title={networkDetails}>
        <div><dt>连接</dt><dd>{networkTypeLabel(network)}</dd></div>
        <div><dt>延迟</dt><dd>{network?.latencyMs != null ? networkNote(network) : "—"}</dd></div>
      </dl>
      {#if !native || networkError || !network}<p class="inline-state" role="status">{!native ? "请在桌面端查看" : networkError ? "检测失败，稍后重试" : "正在检测…"}</p>{/if}
    </article>

    {#each [1, 2] as slot}
      <article class="widget empty" aria-label={`待添加组件，占位 ${slot}`}>
        <span class="add-mark" aria-hidden="true"><svg viewBox="0 0 20 20"><path d="M6 10h8m-4-4v8" /></svg></span>
        <div>
          <h3>添加组件</h3>
          <p>预留信息位置</p>
        </div>
      </article>
    {/each}
  </div>
</section>

<style>
  .widget-panel {
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    min-height: 0;
    width: 100%;
    padding: 8px 24px 20px;
    overflow: auto;
    overscroll-behavior: contain;
    container-type: inline-size;
    color: var(--sc-text, #f5f5f5);
    font: inherit;
    scrollbar-width: thin;
    scrollbar-color: #494949 transparent;
  }
  .widget-panel::-webkit-scrollbar { width: 6px; }
  .widget-panel::-webkit-scrollbar-track { background: transparent; }
  .widget-panel::-webkit-scrollbar-thumb { background: #494949; border-radius: 3px; }
  .widget-panel::-webkit-scrollbar-thumb:hover { background: #686868; }
  .widget-panel:focus-visible { outline: 1px solid #858585; outline-offset: -3px; border-radius: 8px; }
  .widget-panel ::selection { color: #fff; background: #454545; }
  .widget-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; margin: 0 0 12px; }
  h2, h3, p, dl, dd { margin: 0; }
  h2 { font-size: 15px; font-weight: 500; line-height: 22px; }
  .widget-heading p { color: #a3a3a3; font-size: 12px; line-height: 18px; }
  .widget-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
  .widget { box-sizing: border-box; min-width: 0; min-height: 144px; padding: 14px; border: 1px solid #242424; border-radius: 12px; background: #111; }
  h3 { display: flex; align-items: center; gap: 8px; font-size: 13px; font-weight: 500; line-height: 20px; }
  svg { width: 16px; height: 16px; flex: none; fill: none; stroke: #9f9f9f; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
  .primary-value { display: flex; align-items: baseline; gap: 6px; min-height: 30px; margin: 10px 0 8px; }
  .primary-value strong { overflow: hidden; color: #f5f5f5; font-size: 22px; font-weight: 500; line-height: 30px; letter-spacing: -.02em; text-overflow: ellipsis; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .primary-value span { color: #969696; font-size: 10px; font-weight: 400; line-height: 16px; }
  .reachability { align-items: center; gap: 7px; }
  .reachability i { width: 5px; height: 5px; flex: none; border-radius: 50%; background: #666; }
  .reachability i[data-reachable="true"] { background: var(--sc-completed, #8db9a2); }
  .reachability strong { font-size: 20px; }
  .reachability span { margin-left: auto; }
  dl { color: #a3a3a3; font-size: 12px; font-weight: 400; line-height: 20px; }
  dd { color: #d4d4d4; font-variant-numeric: tabular-nums; }
  .compact-stats { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border-top: 1px solid #292929; padding-top: 8px; }
  .compact-stats div { min-width: 0; }
  .compact-stats div + div { border-left: 1px solid #292929; padding-left: 10px; }
  .compact-stats dt { color: #888; font-size: 10px; line-height: 14px; }
  .compact-stats dd { overflow: hidden; margin-top: 1px; color: #d4d4d4; font-size: 12px; line-height: 18px; text-overflow: ellipsis; white-space: nowrap; }
  .inline-state { margin-top: 3px; color: #969696; font-size: 10px; line-height: 14px; }
  .empty { display: flex; align-items: center; justify-content: center; gap: 10px; border-color: #202020; background: #0b0b0b; }
  .add-mark { display: grid; place-items: center; width: 26px; height: 26px; flex: none; border: 1px solid #343434; border-radius: 8px; }
  .add-mark svg { width: 14px; height: 14px; stroke: #898989; }
  .empty h3 { color: #b5b5b5; font-size: 12px; font-weight: 400; }
  .empty p { margin-top: 2px; color: #858585; font-size: 10px; line-height: 16px; }
  @container (max-width: 680px) {
    .widget-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @container (max-width: 411px) {
    .widget-heading { flex-direction: column; gap: 2px; }
    .widget-grid { grid-template-columns: minmax(0, 1fr); }
  }
</style>
