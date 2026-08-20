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
    onhoverchange,
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
    onhoverchange?: (hovered: boolean) => void;
  } = $props();

  const size = $derived(
    shellSize(dockSide, layout, "strip", pinned, dynamicIslandCompatible),
  );
  const renderedWidth = $derived(widthOverride ?? size.width);
  const carouselTasks = $derived(dockCarouselTasks(tasks, surface));
  const carouselIdentity = $derived(carouselTasks.map((item) => item.id).join("\u0000"));
  let carouselIndex = $state(0);
  // A resting pointer means the user is reading the current task: freeze the
  // carousel instead of swapping the headline out from under the cursor.
  let hovering = $state(false);

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
    if (hovering) return;

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
  style:--sc-step-capsule={`${MOTION.capsule}ms`}
  style:--sc-step-travel={`${MOTION.travel}ms`}
  style:--sc-step-panel={`${MOTION.panel}ms`}
  role="button"
  tabindex="0"
  {onclick}
  {ondblclick}
  {oncontextmenu}
  onpointerenter={() => (hovering = true)}
  onpointerleave={() => (hovering = false)}
  onkeydown={(event) => {
    const target = event.target as HTMLElement | null;
    // Inner buttons (action, source cycle) handle their own keys.
    if (target?.closest("button")) return;
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
      flashKey={task ? `${task.id}\u0000${task.status}` : "idle"}
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
          {#if carouselTasks.length > 1}
            <!-- With several running tasks the source label doubles as a
                 manual carousel control; it never toggles the panel. -->
            <button
              class="source cycle"
              class:ready={showCopy}
              type="button"
              title={`${carouselIndex + 1}/${carouselTasks.length} · 点击切换`}
              aria-label={`来源 ${task.source}，${carouselIndex + 1}/${carouselTasks.length}，点击切换`}
              onpointerdown={(event) => event.stopPropagation()}
              onclick={(event) => {
                event.stopPropagation();
                carouselIndex = (carouselIndex + 1) % carouselTasks.length;
              }}
            >
              {SOURCE_LABEL[task.source]}<i class="count">{carouselIndex + 1}/{carouselTasks.length}</i>
            </button>
          {:else}
            <span class="source" class:ready={showCopy}>{SOURCE_LABEL[task.source]}</span>
          {/if}
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
          <TaskDrawer {tasks} {ontaskopen} {onhoverchange} />
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
    --sc-step-capsule: 420ms;
    --sc-step-travel: var(--sc-motion-travel);
    --sc-step-panel: var(--sc-motion-panel);
    --sc-width-motion: var(--sc-step-strip);
    --sc-height-motion: var(--sc-step-panel);
    --sc-pill-height: 42px;
    --sc-pill-radius: 21px;
    --sc-pill-inset: 5px;
    /* The pill is intentionally clearer than the drawer. The slightly denser
       drawer keeps long task titles readable over busy desktop content. */
    --sc-glass-pill: color-mix(in srgb, Canvas 58%, transparent);
    --sc-glass-panel: color-mix(in srgb, Canvas 68%, transparent);
    --sc-glass-pill-pinned: color-mix(in srgb, Canvas 63%, transparent);
    --sc-glass-panel-pinned: color-mix(in srgb, Canvas 72%, transparent);
  }

  /* Closing has its own, deliberately visible contraction beat. The ball
     finishes travelling before this duration starts. */
  .shell[data-flow="closing"] {
    --sc-width-motion: var(--sc-step-capsule);
    --sc-height-motion: var(--sc-step-capsule);
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
    will-change: top, left;
  }

  .shell[data-roll="clockwise"] .ball-slot :global(.orb) {
    animation: sc-orb-roll-clockwise var(--sc-step-travel) var(--sc-ease) both;
  }

  .shell[data-roll="counterclockwise"] .ball-slot :global(.orb) {
    animation: sc-orb-roll-counterclockwise var(--sc-step-travel) var(--sc-ease) both;
  }

  .shell.open .ball-slot {
    position: absolute;
    top: var(--sc-pill-inset);
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
    /* One continuous rounded contour for panel → pill → orb. The browser
       automatically clamps this radius to half the short side of the pill. */
    border-radius: var(--sc-radius);
    background-color: transparent;
    border: 1px solid transparent;
    box-shadow: none;
    isolation: isolate;
    will-change: top, left, width, height, opacity, transform, clip-path;
    --sc-surface: var(--sc-glass-pill);
    transition:
      height var(--sc-height-motion) var(--sc-ease),
      top var(--sc-height-motion) var(--sc-ease),
      width var(--sc-width-motion) cubic-bezier(0.16, 1, 0.3, 1),
      clip-path var(--sc-width-motion) cubic-bezier(0.4, 0, 0.2, 1),
      opacity var(--sc-width-motion) cubic-bezier(0.4, 0, 0.2, 1),
      transform var(--sc-width-motion) cubic-bezier(0.16, 1.12, 0.3, 1),
      background-color 140ms ease-out,
      border-color 180ms ease-out,
      box-shadow var(--sc-width-motion) ease-out,
      left var(--sc-width-motion) var(--sc-ease);
  }

  /* Specular sheen: a white bloom on the top-left and bottom-right corners. */
  .card::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 1;
    border-radius: inherit;
    pointer-events: none;
    opacity: 0;
    background:
      radial-gradient(ellipse 92% 86% at 4% -8%, rgb(255 255 255 / 52%), transparent 46%),
      radial-gradient(ellipse 78% 72% at 98% 108%, rgb(255 255 255 / 24%), transparent 48%);
    box-shadow:
      inset 1px 1px 0 rgb(255 255 255 / 42%),
      inset -1px -1px 0 rgb(255 255 255 / 16%);
    transition: opacity 180ms ease-out;
  }

  /* 1px glass rim, brightest at the same two corners. */
  .card::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 3;
    border-radius: inherit;
    pointer-events: none;
    opacity: 0;
    padding: 1px;
    background: linear-gradient(
      135deg,
      rgb(255 255 255 / 68%) 0%,
      rgb(255 255 255 / 0%) 28%,
      rgb(255 255 255 / 0%) 72%,
      rgb(255 255 255 / 34%) 100%
    );
    mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    mask-composite: exclude;
    -webkit-mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    -webkit-mask-composite: xor;
    transition: opacity 180ms ease-out;
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
    background-color: var(--sc-surface);
    background-image:
      radial-gradient(ellipse 110% 82% at 12% -20%, rgb(255 255 255 / 20%), transparent 52%),
      linear-gradient(
        165deg,
        rgb(255 255 255 / 10%) 0%,
        transparent 42%,
        transparent 66%,
        rgb(0 0 0 / 10%) 100%
      );
    border-color: color-mix(in srgb, white 26%, var(--sc-border));
    box-shadow:
      0 18px 46px rgb(0 0 0 / 30%),
      inset 0 1px 0 rgb(255 255 255 / 18%),
      inset 0 -1px 0 rgb(255 255 255 / 6%);
    -webkit-backdrop-filter: blur(28px) saturate(1.48) contrast(1.04);
    backdrop-filter: blur(28px) saturate(1.48) contrast(1.04);
    left: 0;
  }

  .shell[data-stage="strip"] .card {
    --sc-surface: var(--sc-glass-pill);
  }

  .shell[data-stage="panel"] .card {
    --sc-surface: var(--sc-glass-panel);
  }

  .shell[data-stage="strip"] .card::before,
  .shell[data-stage="panel"] .card::before,
  .shell[data-stage="strip"] .card::after,
  .shell[data-stage="panel"] .card::after {
    opacity: 1;
  }

  /* Pinned surfaces get a little more tint to resist Windows' live-move frame,
     but remain translucent so they retain the same glass material. */
  .shell[data-pinned="true"][data-stage="strip"] .card {
    --sc-surface: var(--sc-glass-pill-pinned);
  }

  .shell[data-pinned="true"][data-stage="panel"] .card {
    --sc-surface: var(--sc-glass-panel-pinned);
  }

  /* A top-docked pill grows symmetrically out of the centered orb. Its 32 px
     seed stays fully behind the ball, then both ends visibly expand from —
     and contract back into — that ball instead of flashing a 48 px halo. */
  .shell.open[data-dock="top"] .card {
    left: 0;
    width: var(--sc-card-width);
    opacity: 1;
    clip-path: inset(0 calc(50% - 16px) round 16px);
    border-radius: var(--sc-radius);
    background-color: var(--sc-surface);
    border-color: color-mix(in srgb, white 26%, var(--sc-border));
  }

  .shell.open[data-dock="top"][data-stage="icon"] .card {
    left: 0;
    transform: scaleY(0.82);
  }

  .shell.open[data-dock="top"][data-stage="strip"] .card {
    clip-path: inset(0 round var(--sc-radius));
    transform: scaleY(1);
  }

  .shell.open[data-dock="top"][data-stage="panel"] .card {
    clip-path: inset(0 round var(--sc-radius));
    transform: scaleY(1);
  }

  .shell.open[data-dock="top"][data-stage="strip"] .card::before,
  .shell.open[data-dock="top"][data-stage="panel"] .card::before,
  .shell.open[data-dock="top"][data-stage="strip"] .card::after,
  .shell.open[data-dock="top"][data-stage="panel"] .card::after {
    opacity: 1;
  }

  .shell[data-stage="strip"] .card {
    height: var(--sc-pill-height);
    /* The native peek clip is a few pixels taller than the visible pill so the
       orb ring is not shaved off. An outer shadow is still clipped by that
       rectangular window and shows up as four faint corners. */
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 22%),
      inset 0 -1px 0 rgb(255 255 255 / 7%);
  }

  .shell[data-layout="peek"] .card {
    cursor: grab;
  }

  .shell[data-layout="peek"] .card:active {
    cursor: grabbing;
  }

  .shell[data-stage="panel"] .card {
    height: 100%;
  }

  /* The transparent WebView backing surface can deliberately remain 448 px
     tall after the native peek HWND has contracted to its 48 px clip. Keep
     every capsule-close frame in that top 48 px band; centering against the
     backing surface moves the still-animating card to y≈203 and makes the
     desktop capsule look as though it vanished in one frame. */
  .shell[data-flow="closing"][data-stage="strip"] .card {
    top: 0;
    transform-origin: center center;
  }

  .shell[data-flow="closing"][data-stage="icon"] .card {
    top: 3px;
    transform-origin: center center;
    /* The glass surface must survive until it is completely hidden behind the
       orb. Dropping these stage-only properties caused the one-frame flash. */
    background-color: var(--sc-surface);
    background-image:
      radial-gradient(ellipse 110% 82% at 12% -20%, rgb(255 255 255 / 20%), transparent 52%),
      linear-gradient(
        165deg,
        rgb(255 255 255 / 10%) 0%,
        transparent 42%,
        transparent 66%,
        rgb(0 0 0 / 10%) 100%
      );
    border-color: color-mix(in srgb, white 26%, var(--sc-border));
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 22%),
      inset 0 -1px 0 rgb(255 255 255 / 7%);
    -webkit-backdrop-filter: blur(28px) saturate(1.48) contrast(1.04);
    backdrop-filter: blur(28px) saturate(1.48) contrast(1.04);
  }

  .shell[data-flow="closing"][data-stage="icon"] .card::before,
  .shell[data-flow="closing"][data-stage="icon"] .card::after {
    opacity: 1;
  }

  /* Top/pinned mode uses real centered width contraction instead of a large
     card plus clip-path. WebView2 can snap the calc() clip inset, while width
     + left interpolate reliably: both rounded ends now meet behind the orb. */
  .shell.open[data-dock="top"][data-flow="closing"][data-stage="icon"] .card {
    left: calc(50% - 16px);
    top: 8px;
    width: 32px;
    height: 32px;
    opacity: 1;
    transform: scale(1);
    clip-path: inset(0 round var(--sc-radius));
  }

  .shell[data-pinned="true"][data-flow="closing"][data-stage="icon"] .card {
    --sc-surface: var(--sc-glass-pill-pinned);
  }

  .shell[data-flow="closing"]:not([data-dock="top"])[data-stage="icon"] .card {
    left: calc(50% - 24px);
    top: 3px;
    height: var(--sc-pill-height);
    transform: scale(1);
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
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
    min-height: var(--sc-pill-height);
    padding: var(--sc-pill-inset) 10px;
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
    position: relative;
    z-index: 2;
    opacity: 0;
    transform: scaleY(0.965);
    transform-origin: center center;
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
    transform: scaleY(0.965);
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

  /* The source doubles as a carousel control while several tasks are running. */
  .source.cycle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 5px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font-family: inherit;
    cursor: pointer;
    transition:
      background-color 120ms ease,
      color 120ms ease;
  }

  .source.cycle:hover,
  .source.cycle:focus-visible {
    background: color-mix(in srgb, var(--sc-text) 10%, transparent);
    color: var(--sc-text);
  }

  .source.cycle:focus-visible {
    outline: 0;
  }

  .source.cycle .count {
    font-size: 8.5px;
    font-style: normal;
    font-weight: 600;
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
    color: color-mix(in srgb, var(--sc-muted) 75%, transparent);
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
    margin: 2px 0 0;
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
    padding: 5px 10px;
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
    .card::before,
    .card::after,
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
