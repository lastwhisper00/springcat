<script lang="ts">
  import type { DockSide, PanelLayout, SurfaceState } from "$domain";
  import { deriveSurfaceState } from "$domain";
  import WorkPanel from "$components/work-panel/WorkPanel.svelte";
  import {
    closePlan,
    foldPlan,
    idleFrame,
    openPlan,
    runMotionPlan,
    unfoldPlan,
    type MotionBeat,
    type MotionFlow,
    type MotionFrame,
  } from "$components/work-panel/panel-motion";
  import { DEMO_TASK_LIST, tasksForKind, type DemoKind } from "./fixtures";

  const kinds: { id: DemoKind; label: string }[] = [
    { id: "idle", label: "idle" },
    { id: "working", label: "working" },
    { id: "working-many", label: "2 running" },
    { id: "waiting", label: "waiting" },
    { id: "completed", label: "completed" },
    { id: "failed", label: "failed" },
    { id: "completed-many", label: "3 completed" },
  ];

  let kind = $state<DemoKind>("idle");
  let dockSide = $state<DockSide>("right");
  let layout = $state<PanelLayout>("collapsed");
  let pinned = $state(false);
  let dynamicIslandCompatible = $state(false);
  let flow = $state<MotionFlow>("idle");
  let motionFrame = $state<MotionFrame>(idleFrame("collapsed"));

  const tasks = $derived(kind === "idle" ? [] : kind === "waiting" ? DEMO_TASK_LIST : tasksForKind(kind));
  const surface = $derived<SurfaceState>(
    deriveSurfaceState(kind === "waiting" ? DEMO_TASK_LIST : tasksForKind(kind)),
  );

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
    flow = nextFlow;
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
    await playMotion("closing", closePlan(layout));
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
</script>

<div class="page">
  <header class="intro">
    <p class="kicker">工作面板</p>
    <h1>贴边图标</h1>
    <p>空闲只留一颗圆点。开合共用三拍：窄条出现 → 球体滑到内侧 → 再展开列表。左侧内侧在右，右侧内侧在左。</p>
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
          {side}
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
          {item}
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

  <div class="monitor" data-dock={dockSide} data-pinned={pinned}>
    <WorkPanel {surface} {tasks} {dockSide} {layout} {pinned} {dynamicIslandCompatible} {flow} {motionFrame} onclick={cycleLayout} />
  </div>
</div>

<style>
  .page {
    min-height: 100%;
    padding: 28px 32px 48px;
    color: #24302b;
  }

  .kicker {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #5d6f66;
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

  .monitor :global(.shell) {
    position: absolute;
  }

  .monitor[data-dock="top"] :global(.shell) {
    top: 8px;
    right: 24px;
  }

  .monitor[data-dock="top"][data-pinned="true"] :global(.shell) {
    top: 0;
    right: auto;
    left: 50%;
    transform: translateX(-50%);
  }

  .monitor[data-dock="right"] :global(.shell) {
    top: 72px;
    right: 6px;
  }

  .monitor[data-dock="left"] :global(.shell) {
    top: 72px;
    left: 6px;
  }
</style>
