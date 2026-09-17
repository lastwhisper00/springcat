<script lang="ts">
  import type { DockSide, PanelLayout, SurfaceState, TaskItem, TaskSource } from "$domain";
  import TaskDrawer from "$components/task-drawer/TaskDrawer.svelte";
  import WidgetPanel from "./WidgetPanel.svelte";
  import type { PanelPage } from "./panel-layout";
  import DockIcon from "./DockIcon.svelte";
  import { runningSources, runningTasks, selectRepresentativeSource } from "./dock-sources";
  import { currentTask, SOURCE_LABEL, shellSize } from "./copy";
  import { idleFrame, MOTION, SURFACE_EASE, orbSize, orbSurfaceSide, resolveAlign,
    type MotionFrame, type MotionFlow } from "./panel-motion";

  let {
    surface, tasks = [], dockSide = "top", orbAnchorSide = dockSide,
    layout = "collapsed", pinned = false, dynamicIslandCompatible = false,
    widthOverride, availableWidth, synchronizedNativeResize = false, fillWindow = false,
    fixedNativeSurface = false,
    snapPreview = false, flow = "idle", motionFrame, notice = "",
    panelPage = "tasks", controlsBusy = false, widgetsSupported = true,
    onclick, onpilltoggle, onpintoggle, onpagetoggle, ondblclick, oncontextmenu,
    ontaskopen, onhoverchange, onmarkallread,
  }: {
    surface: SurfaceState; tasks?: TaskItem[]; dockSide?: DockSide;
    orbAnchorSide?: DockSide; layout?: PanelLayout; pinned?: boolean;
    dynamicIslandCompatible?: boolean; widthOverride?: number; availableWidth?: number;
    synchronizedNativeResize?: boolean; sideVariant?: "strip" | "card";
    fillWindow?: boolean; fixedNativeSurface?: boolean; snapPreview?: boolean; flow?: MotionFlow;
    motionFrame?: MotionFrame; notice?: string;
    panelPage?: PanelPage; controlsBusy?: boolean; widgetsSupported?: boolean;
    onclick?: () => void; onpilltoggle?: () => void;
    onpintoggle?: () => void; onpagetoggle?: () => void;
    ondblclick?: () => void; oncontextmenu?: (event: MouseEvent) => void;
    ontaskopen?: (task: TaskItem) => void; onhoverchange?: (hovered: boolean) => void;
    onmarkallread?: () => void;
  } = $props();

  const size = $derived(shellSize(dockSide, layout, "strip", pinned, dynamicIslandCompatible, panelPage));
  const renderedWidth = $derived(widthOverride ?? size.width);
  let lastSource = $state<TaskSource | null>(null);
  const source = $derived(selectRepresentativeSource(tasks, lastSource, currentTask(surface)?.source));
  $effect(() => { lastSource = source; });
  const running = $derived(runningTasks(tasks));
  const sources = $derived(runningSources(tasks));
  const waitingCount = $derived(tasks.filter(task => task.status === "waiting").length);
  const failedCount = $derived(tasks.filter(task => task.status === "failed").length);
  const runningLabel = $derived(running.length ? `${running.length} 项进行中` : "暂无进行中");
  const sourceDescription = $derived(sources.length
    ? `运行来源：${sources.map(item => SOURCE_LABEL[item]).join("、")}；当前显示 ${source ? SOURCE_LABEL[source] : "未知来源"}`
    : source ? `${SOURCE_LABEL[source]} · 暂无进行中的任务` : "暂无任务");
  const peekLabel = $derived(running.length ? runningLabel
    : waitingCount ? `${waitingCount} 项待确认`
    : failedCount ? `${failedCount} 项失败`
    : surface.kind === "completed" ? `${surface.mergedCount ?? 1} 项已完成` : "暂无进行中");
  const peekSourceLabel = $derived(sources.length > 1
    || (!running.length && currentTask(surface)?.source && currentTask(surface)?.source !== source)
      ? "任务汇总" : source ? SOURCE_LABEL[source] : "");
  const sharedStatusCopy = $derived(runningLabel === peekLabel);
  const frame = $derived(motionFrame ?? idleFrame(layout));
  const stage = $derived(frame.stage);
  const peekSize = $derived(shellSize(dockSide, "peek", "strip", pinned, dynamicIslandCompatible));
  const panelSize = $derived(shellSize(dockSide, "expanded", "strip", pinned, dynamicIslandCompatible, panelPage));
  const cardWidth = $derived(stage === "icon" ? 36 : Math.min(
    widthOverride ?? (stage === "panel" ? panelSize.width : peekSize.width),
    availableWidth ?? Number.POSITIVE_INFINITY,
  ));
  const ballSurfaceSide = $derived(orbSurfaceSide(dockSide, orbAnchorSide, layout, flow, stage));
  // Windows keeps a fixed backing canvas. Its center is the physical orb
  // anchor for every dock, including a collapsed orb being dragged across edges.
  const ballAlign = $derived(fixedNativeSurface && frame.ball === "edge"
    ? "center" : resolveAlign(ballSurfaceSide, frame.ball));
  const showCard = $derived(!(flow === "idle" && layout === "collapsed"));
  const showCopy = $derived(stage !== "icon" && (stage === "panel" || frame.ball === "inner"));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (Only the collapsed branch is a focusable button; the expanded region has tabindex -1.) -->
<section
  class="shell" class:fill={fillWindow} class:preview={snapPreview}
  class:icon={!showCard} class:open={showCard}
  data-kind={surface.kind} data-dock={dockSide} data-layout={layout}
  data-page={panelPage}
  data-pinned={pinned} data-dynamic-island={dynamicIslandCompatible}
  data-fixed-native-surface={fixedNativeSurface}
  data-synchronized-native-resize={synchronizedNativeResize}
  data-flow={flow} data-stage={stage} data-ball={ballAlign}
  style:width={fillWindow ? "100%" : `${renderedWidth}px`}
  style:height={fillWindow ? "100%" : `${size.height}px`}
  style:--sc-card-width={`${cardWidth}px`}
  style:--sc-panel-height={`${panelSize.height}px`}
  style:--sc-step-fold={`${MOTION.fold}ms`}
  style:--sc-ease-spatial={SURFACE_EASE}
  style:--sc-step-strip={`${MOTION.strip}ms`}
  style:--sc-step-capsule={`${MOTION.capsule}ms`}
  style:--sc-step-travel={`${MOTION.travel}ms`}
  style:--sc-step-panel={`${MOTION.panel}ms`}
  role={showCard ? "region" : "button"} tabindex={showCard ? -1 : 0}
  aria-label={`任务面板，${runningLabel}。${sourceDescription}`}
  title={!showCard ? sourceDescription : undefined}
  {onclick} {oncontextmenu}
  ondblclick={(event) => {
    if ((event.target as HTMLElement | null)?.closest("button, .drawer")) return;
    ondblclick?.();
  }}
  onkeydown={(event) => {
    if (event.target !== event.currentTarget || !onclick) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault(); event.stopPropagation(); onclick();
    }
  }}
>
  <div class="ball-slot" data-orb-control="true" data-drag-afford="true">
    <DockIcon {surface} {source} executing={running.length > 0}
      additionalSources={Math.max(0, sources.length - 1)}
      size={orbSize(stage)} drag />
  </div>

  {#if showCard}
    <div class="card" data-drag-afford={layout === "peek" ? "true" : undefined}>
      <header class="chrome" data-pill-control="true" data-drag-afford="true" role="button" tabindex="0"
        aria-label={layout === "expanded" ? (panelPage === "widgets" ? "收起组件面板" : "收起任务列表") : (panelPage === "widgets" ? "展开组件面板" : "展开任务列表")}
        aria-expanded={layout === "expanded"}
        onclick={(event) => { if (onpilltoggle) { event.stopPropagation(); onpilltoggle(); } }}
        onkeydown={(event) => {
          if (event.target !== event.currentTarget || !onpilltoggle) return;
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault(); event.stopPropagation(); onpilltoggle();
          }
        }}>
        <span class="ball-spacer"></span>
        <div class="copy" class:ready={showCopy}>
          {#if sharedStatusCopy}
            <div class="shared-copy">
              {#if peekSourceLabel}
                <span class="source-prefix" aria-hidden={stage === "panel"}>
                  <span class="source">{peekSourceLabel}</span>
                </span>
              {/if}
              <span class="shared-status" title={sourceDescription}>{runningLabel}</span>
            </div>
            <span class="sr-only">{sourceDescription}</span>
          {:else}
            <div class="peek-copy" aria-hidden={stage === "panel"}>
              {#if peekSourceLabel}<span class="source">{peekSourceLabel}</span>{/if}
              <span class="headline" title={`${sourceDescription} · ${peekLabel}`}>{peekLabel}</span>
            </div>
            <div class="panel-copy" aria-hidden={stage !== "panel"}>
              <span class="execution" title={sourceDescription}>{runningLabel}</span>
              <span class="sr-only">{sourceDescription}</span>
            </div>
          {/if}
        </div>
        <div class="panel-controls" inert={stage !== "panel"} aria-hidden={stage !== "panel"}>
          <button class="panel-action" class:active={pinned} type="button"
            aria-label={pinned ? "取消置顶" : "置顶到屏幕顶部"}
            title={pinned ? "取消置顶" : "固定到屏幕顶部"}
            aria-pressed={pinned} disabled={controlsBusy || !onpintoggle}
            onpointerdown={(event) => event.stopPropagation()}
            onclick={(event) => { event.stopPropagation(); onpintoggle?.(); }}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 3h6v5l3 3v2H6v-2l3-3Z" fill={pinned ? "currentColor" : "none"}/><path d="M12 13v8"/></svg>
          </button>
          <button class="panel-action" class:active={panelPage === "widgets"} type="button"
            aria-label={panelPage === "widgets" ? "返回任务列表" : "打开组件面板"}
            title={panelPage === "widgets" ? "返回任务列表" : "组件面板"}
            aria-pressed={panelPage === "widgets"} disabled={controlsBusy || !widgetsSupported || !onpagetoggle}
            onpointerdown={(event) => event.stopPropagation()}
            onclick={(event) => { event.stopPropagation(); onpagetoggle?.(); }}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6h14M5 12h14M5 18h14"/></svg>
          </button>
        </div>
      </header>

      {#if layout === "expanded" || flow === "unfolding"}
        {#if notice}
          <div class="operation-notice" role="status">{notice}</div>
        {/if}
        <div class="drawer-slot" class:ready={stage === "panel"} inert={stage !== "panel"}>
          {#key panelPage}
            <div class="page-content">
              {#if panelPage === "widgets"}
                <WidgetPanel active={stage === "panel" && flow === "idle"} />
              {:else}
                <TaskDrawer {tasks} {ontaskopen} {onhoverchange} {onmarkallread} />
              {/if}
            </div>
          {/key}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  @font-face {
    font-family: "SpringCat Sans";
    src: url("../../assets/fonts/NotoSansSC-VF.ttf") format("truetype");
    font-style: normal;
    font-weight: 100 900;
    font-display: swap;
  }
  .shell {
    --sc-bg: #000; --sc-surface: #000; --sc-text: #f5f5f5;
    --sc-muted: #a3a3a3; --sc-border: #292929; --sc-accent: #c7c7c7;
    --sc-working: #a6bfe3; --sc-waiting: #d9b779;
    --sc-completed: #9cbca9; --sc-failed: #db9698;
    --sc-orb-bg: #000; --sc-orb-text: #f5f5f5; --sc-orb-line: #404040; --sc-orb-working: #a6bfe3;
    --sc-row-hover: rgba(255,255,255,.06); --sc-row-pressed: rgba(255,255,255,.09);
    --sc-radius: 16px; --sc-pill-height: 42px;
    --sc-width-motion: var(--sc-step-strip); --sc-height-motion: var(--sc-step-panel);
    position: relative; color: var(--sc-text); background: transparent;
    font-family: "SpringCat Sans", "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
    font-synthesis: none; font-size: 13px; line-height: 20px;
    container-type: inline-size;
  }
  .shell:focus-visible { outline: 2px solid var(--sc-accent); outline-offset: -2px; border-radius: 24px; }
  .shell { --sc-surface-left: 0px; --sc-card-top: 0px; }
  .shell[data-dock="top"] { --sc-surface-left: calc((100% - var(--sc-card-width)) / 2); }
  .shell[data-dock="right"] { --sc-surface-left: calc(100% - var(--sc-card-width)); }
  .shell[data-fixed-native-surface="true"][data-dock="left"] { --sc-surface-left: calc(50% - 24px); }
  .shell[data-fixed-native-surface="true"][data-dock="right"] { --sc-surface-left: calc(50% + 24px - var(--sc-card-width)); }
  .shell[data-stage="icon"] { --sc-card-top: 6px; }
  .shell[data-stage="icon"][data-ball="start"] { --sc-surface-left: 6px; }
  .shell[data-stage="icon"][data-ball="center"] { --sc-surface-left: calc(50% - 18px); }
  .shell[data-stage="icon"][data-ball="end"] { --sc-surface-left: calc(100% - 42px); }
  .shell[data-flow="unfolding"] { --sc-width-motion: var(--sc-step-panel); }
  .shell[data-flow="folding"] { --sc-width-motion: var(--sc-step-fold); --sc-height-motion: var(--sc-step-fold); }
  .shell[data-flow="closing"] { --sc-width-motion: var(--sc-step-capsule); --sc-height-motion: var(--sc-step-capsule); }
  .shell[data-synchronized-native-resize="true"] { --sc-width-motion: 0ms; --sc-height-motion: 0ms; }
  .ball-slot {
    position: absolute; z-index: 3; top: 6px; width: 36px; height: 36px;
    display: grid; place-items: center; cursor: grab; touch-action: none;
    left: var(--sc-surface-left);
    transition: left var(--sc-width-motion) var(--sc-ease-spatial), top var(--sc-height-motion) var(--sc-ease-spatial);
  }
  .shell.open[data-stage="strip"] .ball-slot { top: 3px; left: calc(var(--sc-surface-left) + 6px); }
  .shell.open[data-stage="strip"][data-ball="center"] .ball-slot { left: calc(50% - 18px); }
  .shell.open[data-stage="strip"][data-ball="end"] .ball-slot { left: calc(100% - 42px); }
  .shell.open[data-stage="strip"][data-ball="start"][data-flow="closing"] .ball-slot { left: 6px; }
  .shell.open[data-stage="panel"] .ball-slot { left: calc(var(--sc-surface-left) + 20px); top: 8px; }
  .shell.preview :global(.orb) { outline: 2px solid var(--sc-accent); outline-offset: 2px; }
  .ball-spacer { width: 28px; height: 28px; flex: none; }
  .card {
    position: absolute; top: var(--sc-card-top); left: var(--sc-surface-left);
    display: flex; flex-direction: column; width: var(--sc-card-width); height: 36px;
    overflow: hidden; border-radius: 18px; background: var(--sc-surface);
    transition: width var(--sc-width-motion) var(--sc-ease-spatial),
      height var(--sc-height-motion) var(--sc-ease-spatial),
      left var(--sc-width-motion) var(--sc-ease-spatial),
      top var(--sc-height-motion) var(--sc-ease-spatial),
      border-radius var(--sc-height-motion) var(--sc-ease-spatial);
  }
  .shell[data-stage="strip"] .card { height: 42px; border-radius: 21px; }
  .shell[data-stage="panel"] .card { height: var(--sc-panel-height); border-radius: 16px; }
  .shell[data-pinned="true"]:not([data-stage="icon"]) .card {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }
  .chrome {
    position: relative; display: flex; align-items: center; gap: 10px;
    height: 42px; flex: 0 0 auto; padding: 0 10px; cursor: grab; touch-action: none;
    transition: height var(--sc-height-motion) var(--sc-ease-spatial), padding var(--sc-width-motion) var(--sc-ease-spatial);
  }
  .chrome:active,.ball-slot:active { cursor: grabbing; }
  .shell[data-stage="panel"] .chrome { height: 52px; padding-inline: 24px; }
  .copy { position: relative; height: 20px; flex: 1; min-width: 0; opacity: 0; transition: opacity 140ms ease; }
  .copy.ready { opacity: 1; }
  .shared-copy { display: flex; align-items: center; height: 20px; min-width: 0; }
  .shared-status { color: var(--sc-muted); font-size: 13px; font-weight: 400; white-space: nowrap; }
  .source-prefix { display: inline-flex; flex: 0 1 auto; overflow: hidden; max-width: 96px; padding-right: 8px; opacity: 1; white-space: nowrap; transition: max-width var(--sc-width-motion) var(--sc-ease-spatial), padding-right var(--sc-width-motion) var(--sc-ease-spatial), opacity 140ms 80ms ease; }
  .shell[data-stage="panel"] .source-prefix { max-width: 0; padding-right: 0; opacity: 0; transition-delay: 0ms; }
  .peek-copy,.panel-copy { position: absolute; inset: 0; display: flex; align-items: center; gap: 8px; min-width: 0; transition: opacity 90ms ease; }
  .peek-copy { transition-delay: 90ms; }
  .panel-copy { opacity: 0; pointer-events: none; }
  .shell[data-stage="panel"] .panel-copy { opacity: 1; pointer-events: auto; transition-delay: 90ms; }
  .shell[data-stage="panel"] .peek-copy { opacity: 0; pointer-events: none; transition-delay: 0ms; }
  .panel-controls { display: flex; align-items: center; justify-content: flex-end; gap: 6px; flex: 0 0 auto; width: 70px; max-width: 0; opacity: 0; overflow: hidden; transition: max-width var(--sc-width-motion) var(--sc-ease-spatial), opacity 140ms ease; }
  .shell[data-stage="panel"] .panel-controls { max-width: 70px; opacity: 1; }
  .source { color: var(--sc-muted); font-size: 12px; white-space: nowrap; }
  .headline { color: var(--sc-text); font-size: 13px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .execution { font-size: 13px; color: var(--sc-muted); white-space: nowrap; }
  .panel-action { display: grid; place-items: center; width: 32px; height: 32px; flex: none; padding: 0; border: 0; border-radius: 6px; background: transparent; color: var(--sc-muted); cursor: pointer; }
  .panel-action:hover,.panel-action.active { background: var(--sc-row-hover); color: var(--sc-text); }
  .panel-action:disabled { opacity: .45; cursor: default; }
  .panel-action svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .drawer-slot { min-height: 0; flex: 1; display: flex; opacity: 0; transform: translateY(-4px); transition: opacity 160ms ease, transform 240ms var(--sc-ease-spatial); }
  .drawer-slot :global(.drawer) { width: 100%; flex: 1; min-height: 0; }
  .page-content { display: flex; flex: 1; min-width: 0; min-height: 0; animation: sc-page-in 180ms 70ms ease both; }
  @keyframes sc-page-in { from { opacity: 0; } to { opacity: 1; } }
  .drawer-slot.ready { opacity: 1; transform: translateY(0); transition-delay: 90ms; }
  .shell[data-flow="folding"] .drawer-slot { opacity: 0; transform: translateY(-4px); transition-delay: 0ms; transition-duration: 100ms; }
  .operation-notice { flex: none; margin: 0 24px; padding: 8px 0; color: var(--sc-failed); font-size: 13px; line-height: 20px; }
  .panel-action:focus-visible,.chrome:focus-visible { outline: 2px solid var(--sc-accent); outline-offset: -2px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; }
  .shell[data-pinned="true"][data-dynamic-island="true"] .copy { max-width: 34%; margin-left: auto; }
  @container (max-width: 400px) {
    .shell[data-stage="panel"] .chrome { padding-inline: 18px; }
    .shell.open[data-stage="panel"] .ball-slot { left: calc(var(--sc-surface-left) + 14px); }
    .operation-notice { margin-inline: 18px; }
  }
  @media (prefers-reduced-motion: reduce) {
    .page-content { animation: none; }
    .card,.ball-slot,.copy,.drawer-slot,.chrome,.peek-copy,.panel-copy,.panel-controls,.source-prefix { transition: none !important; }
  }
</style>
