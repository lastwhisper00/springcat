<script lang="ts">
  import { flushSync } from "svelte";
  import type { DockSide, PanelLayout, SurfaceState } from "$domain";
  import { deriveSurfaceState } from "$domain";
  import WorkPanel from "$components/work-panel/WorkPanel.svelte";
  import { shellSize } from "$components/work-panel/copy";
  import { PANEL_SIZE, type PanelPage } from "$components/work-panel/panel-layout";
  import {
    closeCapsulePlan,
    foldPlan,
    idleFrame,
    openPlan,
    runMotionPlan,
    unfoldPlan,
    type MotionBeat,
    type MotionFlow,
    type MotionFrame,
  } from "$components/work-panel/panel-motion";
  import { tasksForKind, type DemoKind } from "./fixtures";

  const kinds: { id: DemoKind; label: string }[] = [
    { id: "mixed", label: "混合任务" },
    { id: "many", label: "8 项任务" },
    { id: "working", label: "Codex 执行" },
    { id: "working-many", label: "多来源执行" },
    { id: "waiting", label: "待确认" },
    { id: "completed", label: "已完成" },
    { id: "failed", label: "失败" },
    { id: "idle", label: "无任务" },
  ];

  let kind = $state<DemoKind>("many");
  let dockSide = $state<DockSide>("top");
  let layout = $state<PanelLayout>("expanded");
  let pinned = $state(false);
  let panelPage = $state<PanelPage>("tasks");
  let dynamicIslandCompatible = $state(false);
  let flow = $state<MotionFlow>("idle");
  let motionFrame = $state<MotionFrame>(idleFrame("expanded"));
  let readAll = $state(false);
  let demoNotice = $state("示例数据；这里使用与桌面程序相同的组件。");
  let monitorWidth = $state(640);
  const hostWidth = $derived(Math.min(
    Math.max(
      PANEL_SIZE.widgets.width,
      shellSize(dockSide, "expanded", "strip", pinned, dynamicIslandCompatible).width,
      shellSize(dockSide, "peek", "strip", pinned, dynamicIslandCompatible).width,
    ),
    Math.max(1, monitorWidth - 24),
  ));
  const tasks = $derived(tasksForKind(kind).map(task => readAll ? { ...task, unread: false } : task));
  const surface = $derived<SurfaceState>(deriveSurfaceState(tasks));
  $effect(() => { kind; readAll = false; });

  function cycleLayout() {
    if (flow !== "idle") return;
    if (layout === "collapsed") {
      void playOpen("expanded");
      return;
    }
    void playClose();
  }

  function settle(next: PanelLayout) {
    layout = next;
    flow = "idle";
    motionFrame = idleFrame(next);
  }

  async function playMotion(nextFlow: Exclude<MotionFlow, "idle">, plan: MotionBeat[]) {
    flushSync(() => {
      flow = nextFlow;
      motionFrame = plan[0].frame;
    });
    // Let the seed geometry paint before the shared timeline starts moving it.
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    await runMotionPlan(
      plan,
      (frame) => {
        motionFrame = frame;
      },
      (ms) =>
        new Promise((resolve) =>
          setTimeout(
            resolve,
            window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms,
          ),
        ),
    );
  }

  async function playOpen(next: PanelLayout) {
    if (flow !== "idle") return;
    layout = next;
    await playMotion("opening", openPlan(next));
    settle(next);
  }

  async function playClose() {
    if (flow !== "idle" || layout === "collapsed") return;
    if (layout === "expanded") {
      await playMotion("folding", foldPlan());
      settle("peek");
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    }
    await playMotion("closing", closeCapsulePlan());
    settle("collapsed");
  }

  async function playFold() {
    if (flow !== "idle" || layout !== "expanded") return;
    await playMotion("folding", foldPlan());
    settle("peek");
  }

  async function playUnfold() {
    if (flow !== "idle" || layout !== "peek") return;
    layout = "expanded";
    await playMotion("unfolding", unfoldPlan());
    settle("expanded");
  }

  async function togglePage() {
    if (flow !== "idle") return;
    dockSide = "top";
    flow = "unfolding";
    panelPage = panelPage === "tasks" ? "widgets" : "tasks";
    await new Promise(resolve => setTimeout(resolve,
      window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 400));
    settle("expanded");
  }
</script>

<div class="page">
  <header class="intro">
    <h1>核心界面预览</h1>
    <p>任务列表、摘要胶囊与收起光球。球内是智能体 Logo，微光表示执行中。</p>
  </header>

  <div class="controls">
    <fieldset>
      <legend>状态</legend>
      {#each kinds as item}
        <button type="button" class:active={kind === item.id} onclick={() => (kind = item.id)}>{item.label}</button>
      {/each}
    </fieldset>
    <fieldset>
      <legend>吸附边</legend>
      {#each ["top", "left", "right"] as side}
        <button type="button" class:active={dockSide === side} onclick={() => (dockSide = side as DockSide)}>
          {({ top: "顶部", left: "左侧", right: "右侧" })[side]}
        </button>
      {/each}
    </fieldset>
    <fieldset>
      <legend>布局</legend>
      {#each ["collapsed", "peek", "expanded"] as item}
        <button
          type="button"
          class:active={layout === item}
          onclick={() => {
            if (item === "collapsed" && layout !== "collapsed") {
              void playClose();
              return;
            }
            if (item === "peek" && layout === "expanded") {
              void playFold();
              return;
            }
            if (item === "expanded" && layout === "peek") {
              void playUnfold();
              return;
            }
            if (layout === "collapsed" && item !== "collapsed") {
              void playOpen(item as PanelLayout);
              return;
            }
            settle(item as PanelLayout);
          }}
        >
          {({ collapsed: "收起光球", peek: "摘要胶囊", expanded: "展开列表" })[item]}
        </button>
      {/each}
    </fieldset>
    <fieldset>
      <legend>灵动岛</legend>
      <button
        type="button"
        class:active={dynamicIslandCompatible}
        onclick={() => (dynamicIslandCompatible = !dynamicIslandCompatible)}
      >{dynamicIslandCompatible ? "已兼容" : "标准"}</button>
    </fieldset>
    <fieldset>
      <legend>置顶</legend>
      <button
        type="button"
        class:active={pinned}
        onclick={() => {
          pinned = !pinned;
          if (pinned) dockSide = "top";
        }}
      >{pinned ? "已置顶" : "未置顶"}</button>
    </fieldset>
  </div>

  <div class="monitor" data-dock={dockSide} data-pinned={pinned} bind:clientWidth={monitorWidth}>
    <div class="demo-surface" style:width={`${hostWidth}px`}>
      <WorkPanel {surface} {tasks} {dockSide} {layout} {pinned} {panelPage} {dynamicIslandCompatible} {flow} {motionFrame}
        fillWindow availableWidth={hostWidth}
        controlsBusy={flow !== "idle"}
        onclick={cycleLayout} onpilltoggle={() => layout === "expanded" ? void playFold() : void playUnfold()}
        onpintoggle={() => { pinned = !pinned; if (pinned) dockSide = "top"; }}
        onpagetoggle={togglePage} onmarkallread={() => { readAll = true; demoNotice = "已标记已读，任务状态保持不变。"; }}
        ontaskopen={(task) => { demoNotice = `示例：桌面程序会打开「${task.title}」的来源工具。`; }} />
    </div>
  </div>
  <p class="preview-note" role="status">{demoNotice}</p>
</div>

<style>
  .page {
    min-height: 100%;
    padding: 28px 32px 48px;
    color: #24302b;
    background: #f5f6f8;
    color-scheme: light;
  }

  h1 {
    margin: 0 0 8px;
    font-size: 28px;
  }

  .intro p {
    margin: 0;
    max-width: 46rem;
    color: #4d5c55;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin: 24px 0;
  }

  fieldset {
    border: 1px solid rgba(80, 100, 90, 0.2);
    border-radius: 12px;
    padding: 8px 10px;
    display: flex;
    gap: 6px;
    background: rgba(255, 255, 255, 0.45);
  }

  legend {
    padding: 0 4px;
    font-size: 11px;
    color: #5d6f66;
  }

  button {
    border: 0;
    border-radius: 8px;
    padding: 6px 10px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  button.active {
    background: #2c3a34;
    color: #f4f7f5;
  }

  .monitor {
    position: relative;
    min-height: 640px;
    border-radius: 18px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.18), rgba(255, 255, 255, 0.04)),
      #3d4a44;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }

  .demo-surface {
    position: absolute;
    height: 420px;
  }

  .monitor[data-dock="top"] .demo-surface {
    top: 24px;
    left: 50%;
    transform: translateX(-50%);
  }

  .monitor[data-dock="top"][data-pinned="true"] .demo-surface {
    top: 0;
    right: auto;
    left: 50%;
    transform: translateX(-50%);
  }

  .monitor[data-dock="right"] .demo-surface {
    top: 72px;
    right: 6px;
  }

  .monitor[data-dock="left"] .demo-surface {
    top: 72px;
    left: 6px;
  }
  .preview-note { margin-top: 12px; color: #536171; font-size: 13px; }
  @media (max-width: 600px) {
    .page { padding: 20px 14px; }
    .controls { gap: 8px; }
    fieldset { flex-wrap: wrap; }
  }
</style>
