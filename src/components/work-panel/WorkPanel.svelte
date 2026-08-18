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
    idleFrame,
    MOTION,
    orbRollDirection,
    orbSize,
    orbSurfaceSide,
    resolveAlign,
    type MotionFrame,
    type MotionFlow,
    type MotionStage,
  } from "./panel-motion";

  let {
    surface,
    tasks = [],
    dockSide = "top",
    orbAnchorSide = dockSide,
    layout = "collapsed",
    pinned = false,
    dynamicIslandCompatible = false,
    widthOverride,
    synchronizedNativeResize = false,
    fillWindow = false,
    snapPreview = false,
    flow = "idle",
    motionFrame,
    onclick,
    ondblclick,
    oncontextmenu,
    onaction,
    ontaskopen,
  }: {
    surface: SurfaceState;
    tasks?: TaskItem[];
    dockSide?: DockSide;
    orbAnchorSide?: DockSide;
    layout?: PanelLayout;
    pinned?: boolean;
    dynamicIslandCompatible?: boolean;
    widthOverride?: number;
    synchronizedNativeResize?: boolean;
    sideVariant?: "strip" | "card";
    fillWindow?: boolean;
    snapPreview?: boolean;
    flow?: MotionFlow;
    motionFrame?: MotionFrame;
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

  const frame = $derived(motionFrame ?? idleFrame(layout));
  const stage = $derived<MotionStage>(frame.stage);
  const ballLane = $derived(frame.ball);

  const ballSurfaceSide = $derived(
    orbSurfaceSide(dockSide, orbAnchorSide, layout, flow, stage),
  );
  const ballAlign = $derived(resolveAlign(ballSurfaceSide, ballLane));
  const rollDirection = $derived(orbRollDirection(dockSide, flow, stage, ballLane));
  const showCard = $derived(!(flow === "idle" && layout === "collapsed"));
  const isIcon = $derived(!showCard);
  const showCopy = $derived(stage !== "icon" && (stage === "panel" || ballLane === "inner"));

  $effect(() => {
    carouselIdentity;
    carouselIndex = 0;
  });

  $effect(() => {
    const identity = carouselIdentity;
    const motionIdle = flow === "idle";
    // A task/logo swap while the orb is settling reads as a flash at its final
    // position. Freeze the current carousel item for the whole motion and give
    // it a fresh interval only after the panel is fully idle again.
    if (!motionIdle) return;
    if (!identity) return;

    const count = identity.split("\u0000").length;
    if (count <= 1) return;

    const timer = setInterval(() => {
      carouselIndex = (carouselIndex + 1) % count;
    }, 2400);
    return () => clearInterval(timer);
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
  data-roll={rollDirection}
  style:width={fillWindow ? "100%" : `${renderedWidth}px`}
  style:height={fillWindow ? "100%" : `${size.height}px`}
  style:--sc-card-width={fillWindow ? "100%" : `${renderedWidth}px`}
  style:--sc-step-strip={`${MOTION.strip}ms`}
  style:--sc-step-travel={`${MOTION.travel}ms`}
  style:--sc-step-panel={`${MOTION.panel}ms`}
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
  <div class="ball-slot" data-orb-control="true">
    <DockIcon
      {surface}
      source={task?.source ?? null}
      swapKey={task?.id}
      size={orbSize(stage)}
      drag={layout !== "expanded"}
    />
  </div>

  {#if showCard}
    <div class="card" data-drag-afford={layout === "peek" ? "true" : undefined}>
      <header
        class="chrome"
        data-pill-control="true"
        role="button"
        tabindex="0"
        aria-label={layout === "expanded" ? "收起会话列表" : "展开会话列表"}
        aria-expanded={layout === "expanded"}
      >
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

      {#if layout === "expanded" || flow === "unfolding"}
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
    --sc-height-motion: var(--sc-step-panel);
  }

  .shell[data-synchronized-native-resize="true"] {
    --sc-width-motion: 0ms;
    --sc-height-motion: 0ms;
  }

  .shell.open {
    display: block;
  }

  .shell.icon {
    align-items: start;
    border-radius: 50%;
    background: transparent;
  }

  /* Give the final 48 px transparent WebView an explicit, compositor-stable
     orb position. `left: 50%` preserves the same screen-space center while
     the native window contracts; the 6 px top inset leaves room for the ring
     and hover scale instead of clipping them against the window edge. */
  .shell.icon .ball-slot {
    position: absolute;
    top: 6px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
  }

  .shell.icon[data-ball="center"] .ball-slot {
    left: 50%;
    transform: translateX(-50%);
  }

  .shell.icon[data-ball="start"] .ball-slot {
    left: 6px;
  }

  .shell.icon[data-ball="end"] .ball-slot {
    left: calc(100% - 42px);
  }

  .ball-slot {
    z-index: 3;
    cursor: pointer;
    will-change: left;
  }

  .shell[data-roll="clockwise"] .ball-slot :global(.orb) {
    animation: sc-orb-roll-clockwise var(--sc-step-travel) var(--sc-ease) both;
  }

  .shell[data-roll="counterclockwise"] .ball-slot :global(.orb) {
    animation: sc-orb-roll-counterclockwise var(--sc-step-travel) var(--sc-ease) both;
  }

  .shell.open .ball-slot {
    position: absolute;
    top: 8px;
    left: 8px;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    transition:
      top var(--sc-step-strip) var(--sc-ease),
      left var(--sc-step-travel) var(--sc-ease);
  }

  .shell.open[data-stage="icon"] .ball-slot {
    top: 8px;
  }

  .shell.open[data-ball="end"] .ball-slot {
    left: calc(100% - 40px);
  }

  .shell.open[data-ball="center"] .ball-slot {
    left: calc(50% - 16px);
  }

  .shell.open[data-stage="icon"][data-ball="start"] .ball-slot {
    left: 8px;
  }

  .shell.open[data-stage="icon"][data-ball="end"] .ball-slot {
    left: calc(100% - 40px);
  }

  .ball-spacer {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
  }

  /* Keep the hover scale attached to the orb and the visual icon stage. The
     `.icon` container class is removed at the opening seed and restored after
     closing; using it here made the scale reset/pop on both transitions. */
  .shell[data-stage="icon"] .ball-slot:hover :global(.orb) {
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
    height: 48px;
    width: 48px;
    opacity: 0;
    transform: scale(0.76, 0.86);
    transform-origin: 24px 24px;
    border-radius: 50%;
    background-color: transparent;
    border: 1px solid transparent;
    box-shadow: none;
    isolation: isolate;
    will-change: width, height, opacity, transform, clip-path;
    transition:
      height var(--sc-height-motion) var(--sc-ease),
      width var(--sc-width-motion) cubic-bezier(0.16, 1, 0.3, 1),
      clip-path var(--sc-step-strip) cubic-bezier(0.4, 0, 0.2, 1),
      opacity 150ms ease-out,
      transform var(--sc-step-strip) cubic-bezier(0.16, 1.12, 0.3, 1),
      background-color 140ms ease-out,
      border-color 180ms ease-out,
      box-shadow var(--sc-step-strip) ease-out,
      border-radius var(--sc-step-strip) var(--sc-ease),
      left var(--sc-step-strip) var(--sc-ease);
  }

  .shell[data-ball="end"] .card {
    transform-origin: calc(100% - 24px) 24px;
  }

  .shell[data-ball="center"] .card {
    transform-origin: 50% 24px;
  }

  .shell[data-ball="end"][data-stage="icon"] .card {
    left: calc(100% - 48px);
  }

  .shell[data-ball="center"][data-stage="icon"] .card {
    left: calc(50% - 24px);
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

  /* A top-docked pill grows symmetrically out of the centered orb. Its 32 px
     seed stays fully behind the ball, then both ends visibly expand from —
     and contract back into — that ball instead of flashing a 48 px halo. */
  .shell.open[data-dock="top"] .card {
    left: 0;
    width: var(--sc-card-width);
    opacity: 1;
    clip-path: inset(0 calc(50% - 16px) round 16px);
    border-radius: 24px;
    background-color: var(--sc-bg);
    border-color: var(--sc-border);
  }

  .shell.open[data-dock="top"][data-pinned="true"] .card {
    background-color: Canvas;
  }

  .shell.open[data-dock="top"][data-stage="icon"] .card {
    left: 0;
    transform: scaleY(0.82);
  }

  .shell.open[data-dock="top"][data-stage="strip"] .card,
  .shell.open[data-dock="top"][data-stage="panel"] .card {
    clip-path: inset(0 round 24px);
    transform: scaleY(1);
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

  @keyframes sc-orb-roll-clockwise {
    from {
      transform: rotate(0turn);
    }
    to {
      transform: rotate(1turn);
    }
  }

  @keyframes sc-orb-roll-counterclockwise {
    from {
      transform: rotate(0turn);
    }
    to {
      transform: rotate(-1turn);
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
  .action {
    opacity: 0;
    transition: opacity 180ms var(--sc-ease);
  }

  .copy.ready,
  .source.ready,
  .action.ready {
    opacity: 1;
  }

  .drawer-slot {
    opacity: 0;
    transform: translateY(-7px) scaleY(0.985);
    transform-origin: top center;
    transition:
      opacity 300ms ease,
      transform var(--sc-step-panel) cubic-bezier(0.4, 0, 0.2, 1);
  }

  .drawer-slot.ready {
    opacity: 1;
    transform: translateY(0) scaleY(1);
  }

  .shell[data-flow="folding"] .drawer-slot {
    opacity: 0;
    transform: translateY(-7px) scaleY(0.985);
  }

  .copy {
    min-width: 0;
    flex: 1;
    overflow: hidden;
  }

  /* The source keeps its own non-shrinking column. The conversation title is
     centered inside the remaining safe area and ellipsizes there, so it can
     never paint across the AI tool name. */
  .shell[data-pinned="true"][data-stage="strip"] .line {
    justify-content: center;
  }

  .shell[data-pinned="true"][data-stage="strip"] .summary {
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
    .ball-slot :global(.orb) {
      animation: none !important;
    }
  }
</style>
