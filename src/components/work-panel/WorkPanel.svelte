<script lang="ts">
  import type { DockSide, PanelLayout, SurfaceState, TaskItem } from "$domain";
  import TaskDrawer from "$components/task-drawer/TaskDrawer.svelte";
  import DockIcon from "./DockIcon.svelte";
  import { dockCarouselTasks } from "./dock-sources";
  import {
    currentTask,
    panelActionLabel,
    panelHeadline,
    panelSummary,
    SOURCE_LABEL,
    shellSize,
  } from "./copy";
  import {
    closePlan,
    idleFrame,
    openPlan,
    playPlan,
    resolveAlign,
    type BallLane,
    type MotionFlow,
    type MotionStage,
  } from "./panel-motion";

  let {
    surface,
    tasks = [],
    dockSide = "top",
    layout = "collapsed",
    pinned = false,
    dynamicIslandCompatible = false,
    widthOverride,
    synchronizedNativeResize = false,
    fillWindow = false,
    snapPreview = false,
    flow = "idle",
    onclick,
    ondblclick,
    oncontextmenu,
    onaction,
    ontaskopen,
  }: {
    surface: SurfaceState;
    tasks?: TaskItem[];
    dockSide?: DockSide;
    layout?: PanelLayout;
    pinned?: boolean;
    dynamicIslandCompatible?: boolean;
    widthOverride?: number;
    synchronizedNativeResize?: boolean;
    sideVariant?: "strip" | "card";
    fillWindow?: boolean;
    snapPreview?: boolean;
    flow?: MotionFlow;
    onclick?: () => void;
    ondblclick?: () => void;
    oncontextmenu?: (event: MouseEvent) => void;
    onaction?: () => void;
    ontaskopen?: (task: TaskItem) => void;
  } = $props();

  const size = $derived(
    shellSize(dockSide, layout, "strip", pinned, dynamicIslandCompatible),
  );
  const renderedWidth = $derived(widthOverride ?? size.width);
  const carouselTasks = $derived(dockCarouselTasks(tasks, surface));
  const carouselIdentity = $derived(carouselTasks.map((item) => item.id).join("\u0000"));
  let carouselIndex = $state(0);

  const task = $derived(
    carouselTasks.length > 0
      ? carouselTasks[carouselIndex % carouselTasks.length]
      : currentTask(surface),
  );
  const headline = $derived(
    surface.kind === "working" && task ? task.title : panelHeadline(surface),
  );
  const summary = $derived(panelSummary(surface));
  const action = $derived(panelActionLabel(surface));
  const unread = $derived(surface.kind === "completed" && surface.unread);

  let stage = $state<MotionStage>("icon");
  let ballLane = $state<BallLane>("edge");

  const ballAlign = $derived(resolveAlign(dockSide, ballLane));
  const showCard = $derived(!(flow === "idle" && layout === "collapsed"));
  const isIcon = $derived(!showCard);
  const showCopy = $derived(stage !== "icon" && (stage === "panel" || ballLane === "inner"));

  $effect(() => {
    const identity = carouselIdentity;
    carouselIndex = 0;
    if (!identity) return;

    const count = identity.split("\u0000").length;
    if (count <= 1) return;

    const timer = setInterval(() => {
      carouselIndex = (carouselIndex + 1) % count;
    }, 2400);
    return () => clearInterval(timer);
  });

  $effect(() => {
    const nextFlow = flow;
    const nextLayout = layout;
    let cancelled = false;
    const reduced =
      typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    if (nextFlow === "idle") {
      const idle = idleFrame(nextLayout);
      stage = idle.stage;
      ballLane = idle.ball;
      return;
    }

    const plan = nextFlow === "opening" ? openPlan(nextLayout) : closePlan(nextLayout);
    void playPlan(
      plan,
      (frame) => {
        stage = frame.stage;
        ballLane = frame.ball;
      },
      (ms) => new Promise((resolve) => setTimeout(resolve, reduced ? 0 : ms)),
      () => cancelled,
    );

    return () => {
      cancelled = true;
    };
  });
</script>

<section
  class="shell"
  class:fill={fillWindow}
  class:preview={snapPreview}
  class:icon={isIcon}
  class:open={showCard}
  data-kind={surface.kind}
  data-dock={dockSide}
  data-layout={layout}
  data-pinned={pinned}
  data-dynamic-island={dynamicIslandCompatible}
  data-synchronized-native-resize={synchronizedNativeResize}
  data-has-action={Boolean(action)}
  data-flow={flow}
  data-stage={stage}
  data-ball={ballAlign}
  data-ball-lane={ballLane}
  style:width={fillWindow ? "100%" : `${renderedWidth}px`}
  style:height={fillWindow ? "100%" : `${size.height}px`}
  style:--sc-card-width={fillWindow ? "100%" : `${renderedWidth}px`}
  role="button"
  tabindex="0"
  {onclick}
  {ondblclick}
  {oncontextmenu}
  onkeydown={(event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onclick?.();
    }
  }}
>
  <div class="ball-slot">
    <DockIcon {surface} source={task?.source ?? null} swapKey={task?.id} size={showCard ? 32 : 36} drag />
  </div>

  {#if showCard}
    <div class="card" data-drag-afford={layout === "peek" ? "true" : undefined}>
      <header class="chrome">
        {#if ballAlign === "start"}
          <span class="ball-spacer"></span>
        {/if}
        {#if task}
          <span class="source" class:ready={showCopy}>{SOURCE_LABEL[task.source]}</span>
        {/if}
        <div class="copy" class:ready={showCopy}>
          {#key task?.id ?? surface.kind}
            <div class="copy-swap">
              <div class="line">
                {#if headline}<strong class="headline" title={headline}>{headline}</strong>{/if}
                {#if unread}<i class="unread" title="未读"></i>{/if}
              </div>
              {#if summary}
                <p class="summary">{summary}</p>
              {/if}
            </div>
          {/key}
        </div>
        {#if action}
          <button
            class="action"
            class:ready={showCopy}
            type="button"
            onclick={(event) => {
              event.stopPropagation();
              onaction?.();
            }}>{action}</button
          >
        {/if}
        {#if ballAlign === "end"}
          <span class="ball-spacer"></span>
        {/if}
      </header>

      {#if layout === "expanded" || flow === "closing"}
        <div class="drawer-slot" class:ready={stage === "panel"}>
          <TaskDrawer {tasks} {ontaskopen} />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .shell {
    position: relative;
    display: grid;
    place-items: center;
    color: var(--sc-text);
    cursor: pointer;
    background: transparent;
    --sc-step-strip: var(--sc-motion-strip);
    --sc-step-travel: var(--sc-motion-travel);
    --sc-step-panel: var(--sc-motion-panel);
    --sc-width-motion: var(--sc-step-strip);
  }

  .shell[data-synchronized-native-resize="true"] {
    --sc-width-motion: 0ms;
  }

  .shell.open {
    display: block;
  }

  .shell.icon {
    border-radius: 50%;
    background: transparent;
  }

  .ball-slot {
    z-index: 3;
  }

  .shell.open .ball-slot {
    position: absolute;
    top: 8px;
    left: 8px;
    transition:
      top var(--sc-step-strip) var(--sc-ease),
      left var(--sc-step-travel) var(--sc-ease);
  }

  .shell.open[data-stage="icon"] .ball-slot {
    top: 6px;
  }

  .shell.open[data-ball="end"] .ball-slot {
    left: calc(100% - 40px);
  }

  .shell.open[data-stage="icon"][data-ball="start"] .ball-slot {
    left: 6px;
  }

  .shell.open[data-stage="icon"][data-ball="end"] .ball-slot {
    left: calc(100% - 38px);
  }

  .ball-spacer {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
  }

  .shell.icon:hover :global(.orb) {
    transform: scale(1.06);
    transition: transform 180ms var(--sc-ease);
  }

  .shell.preview :global(.orb) {
    box-shadow:
      0 0 0 3px color-mix(in srgb, var(--sc-accent) 45%, transparent),
      0 3px 8px color-mix(in srgb, var(--sc-text) 16%, transparent);
  }

  .card {
    position: absolute;
    top: 0;
    left: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    height: 44px;
    width: 44px;
    opacity: 0;
    transform: scale(0.76, 0.86);
    transform-origin: 22px 22px;
    border-radius: 50%;
    background-color: transparent;
    border: 1px solid transparent;
    box-shadow: none;
    isolation: isolate;
    will-change: width, height, opacity, transform;
    transition:
      height var(--sc-step-panel) var(--sc-ease),
      width var(--sc-width-motion) cubic-bezier(0.16, 1, 0.3, 1),
      opacity 150ms ease-out,
      transform var(--sc-step-strip) cubic-bezier(0.16, 1.12, 0.3, 1),
      background-color 140ms ease-out,
      border-color 180ms ease-out,
      box-shadow var(--sc-step-strip) ease-out,
      border-radius var(--sc-step-strip) var(--sc-ease),
      left var(--sc-step-strip) var(--sc-ease);
  }

  .shell[data-ball="end"] .card {
    transform-origin: calc(100% - 22px) 22px;
  }

  .shell[data-ball="end"][data-stage="icon"] .card {
    left: calc(100% - 44px);
  }

  .shell[data-stage="strip"] .card,
  .shell[data-stage="panel"] .card {
    width: var(--sc-card-width);
    opacity: 1;
    transform: scale(1);
    background-color: var(--sc-bg);
    border-color: var(--sc-border);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.38);
    left: 0;
  }

  /* A pinned pill sits over window title bars while they are being dragged.
     Keep that surface opaque so Windows' live-move frame cannot show through
     the otherwise translucent panel background as a horizontal black strip. */
  .shell[data-pinned="true"][data-stage="strip"] .card,
  .shell[data-pinned="true"][data-stage="panel"] .card {
    background-color: Canvas;
  }

  .shell[data-stage="strip"] .card {
    height: 48px;
    border-radius: 24px;
    /* The native peek window is exactly the pill's bounds. An outer shadow is
       clipped by that rectangular window and shows up as four faint corners. */
    box-shadow: none;
  }

  .shell[data-layout="peek"] .card {
    cursor: grab;
  }

  .shell[data-layout="peek"] .card:active {
    cursor: grabbing;
  }

  .shell[data-stage="panel"] .card {
    height: 100%;
    border-radius: var(--sc-radius);
  }

  .shell[data-flow="opening"][data-stage="strip"] .card::after {
    content: "";
    position: absolute;
    z-index: 4;
    top: -1px;
    left: 8%;
    width: 28%;
    height: 1px;
    border-radius: 999px;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.5), transparent);
    pointer-events: none;
    animation: sc-frame-glint 420ms ease-out both;
  }

  @keyframes sc-frame-glint {
    from {
      transform: translateX(-65%);
      opacity: 0;
    }
    35% {
      opacity: 0.65;
    }
    to {
      transform: translateX(230%);
      opacity: 0;
    }
  }

  .chrome {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    overflow: hidden;
    min-height: 48px;
    padding: 8px 12px;
  }

  .copy,
  .source,
  .action,
  .drawer-slot {
    opacity: 0;
    transition: opacity 160ms var(--sc-ease);
  }

  .copy.ready,
  .source.ready,
  .action.ready,
  .drawer-slot.ready {
    opacity: 1;
  }

  .copy {
    min-width: 0;
    flex: 1;
    overflow: hidden;
  }

  /* The source keeps its own non-shrinking column. The conversation title is
     centered inside the remaining safe area and ellipsizes there, so it can
     never paint across the AI tool name. */
  .shell[data-pinned="true"][data-layout="peek"] .line {
    justify-content: center;
  }

  .shell[data-pinned="true"][data-layout="peek"] .summary {
    text-align: center;
  }

  /* A centered notch can cover the middle of a pinned pill. Compatibility
     mode confines copy to the right-hand safe area and lets long headlines
     ellipsize there instead of flowing back underneath the notch. */
  .shell[data-pinned="true"][data-dynamic-island="true"] .copy {
    flex: 0 1 34%;
    max-width: 34%;
    margin-left: auto;
  }

  .shell[data-pinned="true"][data-dynamic-island="true"][data-has-action="true"] .copy {
    flex-basis: 34%;
  }

  .shell[data-pinned="true"][data-dynamic-island="true"] .line {
    justify-content: flex-end;
  }

  .shell[data-pinned="true"][data-dynamic-island="true"] .summary {
    text-align: right;
  }

  .copy-swap {
    min-width: 0;
    overflow: hidden;
    animation: sc-copy-swap 180ms var(--sc-ease) both;
  }

  .drawer-slot {
    min-height: 0;
    flex: 1;
    display: flex;
  }

  .drawer-slot :global(.drawer) {
    flex: 1;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  .source {
    flex-shrink: 0;
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--sc-muted);
  }

  /* Keep the provider label beside the orb's inner edge. When the orb travels
     to the right end, move the label after the flexible copy and any action;
     the spacer remains last so the label sits immediately left of the orb. */
  .shell[data-ball="end"] .source {
    order: 1;
  }

  .shell[data-ball="end"] .ball-spacer {
    order: 2;
  }

  .headline {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 650;
  }

  .summary {
    margin: 3px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--sc-muted);
  }

  .unread {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--sc-completed);
    flex-shrink: 0;
  }

  .action {
    flex-shrink: 0;
    border: 0;
    border-radius: 999px;
    padding: 6px 10px;
    background: var(--sc-fill);
    color: var(--sc-text);
    font: inherit;
    cursor: pointer;
  }

  .action:hover {
    background: var(--sc-fill-strong);
  }

  @keyframes sc-copy-swap {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .card,
    .ball-slot,
    .copy,
    .source,
    .action,
    .drawer-slot {
      transition: none;
    }

    .copy-swap,
    .card::after {
      animation: none !important;
    }
  }
</style>
