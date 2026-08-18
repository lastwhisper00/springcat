<script lang="ts">
  import { flushSync, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import WorkPanel from "$components/work-panel/WorkPanel.svelte";
  import { shellSize } from "$components/work-panel/copy";
  import {
    closePlan,
    idleFrame,
    MOTION,
    openPlan,
    runMotionPlan,
    type MotionBeat,
    type MotionFlow,
    type MotionFrame,
  } from "$components/work-panel/panel-motion";
  import {
    decideNotification,
    didFinishLastRunning,
    isMuted,
    notificationRestingLayout,
    shouldPinPanel,
    type PanelLayout,
  } from "$domain";
  import type { DockSide, TaskItem } from "$domain";
  import type { ClientSettings } from "$domain/settings";
  import { taskStore } from "./stores/tasks.svelte";
  import { settingsStore } from "./stores/settings.svelte";
  import {
    animateSynchronizedResize,
    applySynchronizedResizeStep,
  } from "./synchronized-resize";
  import {
    applyPanelLayout,
    dockAfterDrag,
    getSettings,
    listTasks,
    movePanel,
    muteHour,
    openLatest,
    openSettings,
    openTask,
    popupPanelMenu,
    preparePanelLayout,
    previewDock,
    resizePanelFrame,
    resizePinnedPanel,
    setPanelPinned,
    topPinTarget,
    updateSettings,
    type DockChanged,
  } from "$services/tauri";
  import { blockMiddleButtonDefault } from "./pointer-guards";
  import {
    drawerIdleTarget,
    orbTargetLayout,
    pillTargetLayout,
    suppressUserCollapsedAutoOpen,
    taskPolicyKey,
  } from "./orb-interaction";

  const DRAG_PX = 6;
  const DRAWER_IDLE_MS = 2_000;

  interface PinnedPointerDrag {
    pointerId: number;
    element: HTMLElement;
    screenX: number;
    screenY: number;
    windowStart: Promise<{ x: number; y: number }>;
  }

  let layout = $state<PanelLayout>("collapsed");
  let dockSide = $state<DockSide>("top");
  let surfaceAnchorSide = $state<DockSide>("top");
  let snapPreview = $state(false);
  let synchronizedPanelWidth = $state<number | undefined>(undefined);
  let synchronizedNativeResize = $state(false);
  let userExpanded = $state(false);
  let userPeeked = $state(false);
  let userCollapsedPill = $state(false);
  let pinned = $state(false);
  let flow = $state<MotionFlow>("idle");
  let motionFrame = $state<MotionFrame>(idleFrame("collapsed"));
  let busy = false;
  let dynamicIslandResizeBusy = false;
  let pinBusy = false;
  let reanchorBusy = false;
  let policyPending = false;
  let dragStarted = $state(false);
  let canToggle = false;
  let pointerToggleTarget: "orb" | "pill" | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let drawerIdleTimer: ReturnType<typeof setTimeout> | undefined;
  let hideKey: string | null = null;
  let hideDeadline = 0;
  let origin: { x: number; y: number } | null = null;
  let lastPhysical: { x: number; y: number } | null = null;
  let pinQueue = Promise.resolve();
  let settingsQueue = Promise.resolve();
  let pinnedPointerDrag: PinnedPointerDrag | null = null;
  let nextPinnedDragPosition: { x: number; y: number } | null = null;
  let pinnedDragPump: Promise<void> | null = null;
  let windowFocused = true;

  const muted = $derived(isMuted(settingsStore.value));
  // Pinning is a physical top-center mode. Keep the visual choreography tied
  // to that invariant even while settings/HMR events are reconciling the last
  // unpinned dock side in the background.
  const visualDockSide = $derived<DockSide>(pinned ? "top" : dockSide);
  const decision = $derived(
    decideNotification(taskStore.surface, {
      muted,
      focusMode: settingsStore.value.focusMode,
    }),
  );

  function motionMs(ms: number) {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms;
  }

  function sleep(ms: number) {
    return new Promise<void>((resolve) => setTimeout(resolve, ms));
  }

  function nextPaint() {
    return new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }

  async function waitMotionBeat(ms: number) {
    // Start each duration only after its frame has reached the compositor, and
    // give the completed CSS transition one final paint before native resize.
    await nextPaint();
    const wait = motionMs(ms);
    if (wait) await sleep(wait);
    await nextPaint();
  }

  async function playMotion(nextFlow: Exclude<MotionFlow, "idle">, plan: MotionBeat[]) {
    flow = nextFlow;
    await runMotionPlan(
      plan,
      (frame) => {
        flushSync(() => {
          motionFrame = frame;
        });
      },
      waitMotionBeat,
    );
  }

  function settleLayout(next: PanelLayout) {
    flushSync(() => {
      layout = next;
      flow = "idle";
      motionFrame = idleFrame(next);
    });
  }

  function applyNativeLayout(next: PanelLayout) {
    return applyPanelLayout(
      next,
      lastPhysical,
      pinned,
      pinned ? settingsStore.value.dynamicIslandCompatible : false,
    );
  }

  function prepareNativeLayout(next: PanelLayout) {
    return preparePanelLayout(
      next,
      lastPhysical,
      pinned,
      pinned ? settingsStore.value.dynamicIslandCompatible : false,
    );
  }

  async function animateDrawerWindow(expanding: boolean) {
    const fromLayout: PanelLayout = expanding ? "peek" : "expanded";
    const toLayout: PanelLayout = expanding ? "expanded" : "peek";
    const from = shellSize(
      visualDockSide,
      fromLayout,
      "strip",
      pinned,
      pinned && settingsStore.value.dynamicIslandCompatible,
    );
    const to = shellSize(
      visualDockSide,
      toLayout,
      "strip",
      pinned,
      pinned && settingsStore.value.dynamicIslandCompatible,
    );

    // Mount the drawer behind the existing pill before its first visible
    // frame. During folding, keep the panel stage active so its height follows
    // the shrinking native viewport instead of snapping directly to 48 px.
    flushSync(() => {
      synchronizedNativeResize = true;
      flow = expanding ? "unfolding" : "folding";
      motionFrame = {
        stage: expanding ? "strip" : "panel",
        ball: "inner",
      };
    });
    await nextPaint();
    if (expanding) {
      flushSync(() => {
        motionFrame = { stage: "panel", ball: "inner" };
      });
    }
    await nextPaint();

    try {
      await animateSynchronizedResize({
        from,
        to,
        duration: motionMs(MOTION.panel),
        resize: ({ width, height }) => resizePanelFrame(width, height, pinned),
      });
      await applyNativeLayout(toLayout);
      settleLayout(toLayout);
      const final = await getCurrentWindow().outerPosition();
      lastPhysical = { x: final.x, y: final.y };
      await nextPaint();
    } finally {
      flushSync(() => {
        synchronizedNativeResize = false;
      });
    }
  }

  async function waitForPanelIdle() {
    while (busy || dynamicIslandResizeBusy || reanchorBusy || flow !== "idle") await sleep(16);
  }

  async function applySynchronizedPanelWidth(
    width: number,
    height: number,
    expanding: boolean,
  ) {
    await applySynchronizedResizeStep({
      width,
      height,
      expanding,
      resizeNative: resizePinnedPanel,
      renderWidth: (nextWidth) => {
        flushSync(() => {
          synchronizedPanelWidth = nextWidth;
        });
      },
    });
  }

  async function animateDynamicIslandWidth(
    previousCompatible: boolean,
    nextCompatible: boolean,
    nextSettings: ClientSettings,
  ) {
    await waitForPanelIdle();
    if (!pinned || layout === "collapsed") return;

    const animatedLayout = layout;
    const from = shellSize("top", animatedLayout, "strip", true, previousCompatible);
    const to = shellSize("top", animatedLayout, "strip", true, nextCompatible);
    if (from.width === to.width) return;

    const expanding = to.width > from.width;
    const duration = motionMs(MOTION.dock);
    dynamicIslandResizeBusy = true;
    synchronizedNativeResize = true;
    synchronizedPanelWidth = from.width;
    await nextPaint();
    settingsStore.value = nextSettings;
    await nextPaint();

    try {
      if (duration === 0) {
        await applySynchronizedPanelWidth(to.width, to.height, expanding);
      } else {
        await animateSynchronizedResize({
          from,
          to,
          duration,
          resize: ({ width, height }) =>
            applySynchronizedPanelWidth(width, height, expanding),
        });
      }

      await applyPanelLayout(
        animatedLayout,
        lastPhysical,
        true,
        nextCompatible,
      );
      const final = await getCurrentWindow().outerPosition();
      lastPhysical = { x: final.x, y: final.y };
    } finally {
      flushSync(() => {
        synchronizedPanelWidth = undefined;
        synchronizedNativeResize = false;
      });
      dynamicIslandResizeBusy = false;
      if (policyPending) {
        policyPending = false;
        queueMicrotask(() => applyPolicy(true));
      }
    }
  }

  async function collapseExpandedAfterBlur() {
    await waitForPanelIdle();
    if (windowFocused || layout !== "expanded") return;
    await foldDrawerToPill();
  }

  async function slideWindowTo(x: number, y: number, ms: number) {
    const win = getCurrentWindow();
    if (ms <= 0) {
      await movePanel(x, y);
      const final = await win.outerPosition();
      lastPhysical = { x: final.x, y: final.y };
      return;
    }
    const scale = await win.scaleFactor();
    const startPhys = lastPhysical ?? (await win.outerPosition());
    const start = { x: startPhys.x / scale, y: startPhys.y / scale };
    if (Math.hypot(start.x - x, start.y - y) < 1) {
      await movePanel(x, y);
      const final = await win.outerPosition();
      lastPhysical = { x: final.x, y: final.y };
      return;
    }
    const t0 = performance.now();
    await new Promise<void>((resolve, reject) => {
      const frame = async (now: number) => {
        const p = Math.min(1, (now - t0) / ms);
        const eased = 1 - (1 - p) ** 3;
        try {
          await movePanel(
            start.x + (x - start.x) * eased,
            start.y + (y - start.y) * eased,
          );
          if (p < 1) requestAnimationFrame(frame);
          else resolve();
        } catch (error) {
          reject(error);
        }
      };
      requestAnimationFrame(frame);
    });
    await movePanel(x, y);
    const final = await win.outerPosition();
    lastPhysical = { x: final.x, y: final.y };
  }

  async function setLayout(next: PanelLayout, force = false) {
    if (dynamicIslandResizeBusy) {
      policyPending = true;
      return;
    }
    if (busy) return;
    if (!force && next === layout && flow === "idle") return;
    busy = true;
    try {
      if (next === "peek" && layout === "expanded") {
        await animateDrawerWindow(false);
        return;
      }
      if (next === "expanded" && layout === "peek") {
        await animateDrawerWindow(true);
        return;
      }
      if (next === "collapsed" && layout !== "collapsed") {
        await playMotion("closing", closePlan(layout));
        surfaceAnchorSide = visualDockSide;
        settleLayout("collapsed");
        userExpanded = false;
        userPeeked = false;
        // Paint the resting orb in a surface whose width already matches the
        // final 48 px native clip. Keeping the old wide surface here makes the
        // collapsed CSS resolve against that stale viewport on WebView2, so a
        // side-docked window can clip a blank slice after contraction.
        await nextPaint();
        await prepareNativeLayout("collapsed");
        await nextPaint();
        await nextPaint();
        await applyNativeLayout("collapsed");
        return;
      }
      if (layout === "collapsed" && next !== "collapsed") {
        const plan = openPlan(next);
        // Commit the opening seed inside the still-collapsed native bounds
        // before resizing the WebView. Without this painted seed, Windows can
        // enlarge the transparent window one frame before the absolute ball
        // and clipped capsule styles are ready, producing a visible flash.
        flushSync(() => {
          flow = "opening";
          layout = next;
          motionFrame = plan[0].frame;
        });
        await nextPaint();
        await prepareNativeLayout(next);
        // A drag can change the dock side while the preserved wide surface is
        // still anchored for the old side. Switch the orb anchor only after
        // the new surface has been placed under the collapsed native clip.
        flushSync(() => {
          surfaceAnchorSide = visualDockSide;
        });
        // The expanded WebView is aligned under the still-collapsed native
        // clip. Wait until that full surface has actually painted before the
        // parent HWND exposes it.
        await nextPaint();
        await nextPaint();
        await applyNativeLayout(next);
        await playMotion("opening", plan);
        settleLayout(next);
        return;
      }
      settleLayout(next);
      await applyNativeLayout(next);
    } finally {
      busy = false;
      if (policyPending) {
        policyPending = false;
        queueMicrotask(() => applyPolicy(true));
      }
    }
  }

  function clearTimers() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
    }
  }

  function clearDrawerIdleTimer() {
    if (!drawerIdleTimer) return;
    clearTimeout(drawerIdleTimer);
    drawerIdleTimer = undefined;
  }

  function armDrawerIdleTimer() {
    clearDrawerIdleTimer();
    if (layout !== "expanded" || flow !== "idle") return;
    drawerIdleTimer = setTimeout(() => {
      drawerIdleTimer = undefined;
      if (layout === "expanded" && flow === "idle") void foldDrawerToPill();
    }, DRAWER_IDLE_MS);
  }

  async function foldDrawerToPill() {
    if (layout !== "expanded" || busy || flow !== "idle") return;
    clearDrawerIdleTimer();
    userExpanded = false;
    userPeeked = true;
    await setLayout(drawerIdleTarget(layout));
  }

  function noteDrawerActivity(event: Event) {
    if (layout !== "expanded") return;
    const target = event.target as HTMLElement | null;
    if (target?.closest(".drawer")) armDrawerIdleTimer();
  }

  $effect(() => {
    const drawerOpen = layout === "expanded" && flow === "idle";
    if (drawerOpen) armDrawerIdleTimer();
    else clearDrawerIdleTimer();
  });

  function autoHideWindow(): { shouldPeek: boolean; remainingMs: number | null } {
    if (pinned && taskStore.surface.kind !== "idle") {
      hideKey = null;
      hideDeadline = 0;
      return { shouldPeek: true, remainingMs: null };
    }
    if (!decision.peek || decision.autoHideMs === null || taskStore.surface.kind !== "completed") {
      hideKey = null;
      hideDeadline = 0;
      return { shouldPeek: decision.peek, remainingMs: null };
    }

    const state = taskStore.surface;
    const key = `${state.task.id}\u0000${state.task.updatedAt}`;
    const now = Date.now();
    if (hideKey !== key) {
      hideKey = key;
      hideDeadline = now + decision.autoHideMs;
    }
    const remainingMs = Math.max(0, hideDeadline - now);
    return { shouldPeek: remainingMs > 0, remainingMs };
  }

  function applyPolicy(fromEvent = false) {
    clearTimers();
    if (busy || dynamicIslandResizeBusy || pinBusy || reanchorBusy || flow !== "idle") {
      policyPending = true;
      return;
    }
    policyPending = false;
    if (userExpanded && layout === "expanded") return;
    if (userPeeked && layout === "peek") return;
    if (suppressUserCollapsedAutoOpen(layout, userCollapsedPill)) return;
    if (!fromEvent && layout === "expanded") return;
    const { shouldPeek, remainingMs } = autoHideWindow();
    if (layout === "expanded") {
      void setLayout(shouldPeek ? "peek" : "collapsed");
    } else if (shouldPeek && layout === "collapsed") {
      void setLayout("peek");
    }
    if (!shouldPeek && layout === "peek") {
      void setLayout("collapsed");
    }
    if (remainingMs !== null && remainingMs > 0) {
      hideTimer = setTimeout(() => {
        hideTimer = undefined;
        if (!userExpanded && !userPeeked && layout === "peek") {
          void setLayout("collapsed");
        }
      }, remainingMs);
    }
  }

  function restingLayout(): PanelLayout {
    return pinned && taskStore.surface.kind !== "idle"
      ? "peek"
      : notificationRestingLayout(decision);
  }

  async function transitionPinned(next: boolean) {
    // The native top guard is an effective runtime state, not just the saved
    // manual preference. Re-sync even when the visual state already matches:
    // changing the manual preference while an auto-pin is active can otherwise
    // disable the guard underneath the still-pinned task pill.
    if (next === pinned) {
      await setPanelPinned(next);
      if (next) dockSide = "top";
      return;
    }
    await waitForPanelIdle();
    pinBusy = true;
    clearTimers();
    try {
      if (next) {
        // A new pin period may reveal the pill once. If the user closes it from
        // the orb afterwards, policy updates must respect that decision.
        userCollapsedPill = false;
        // Enable this before the slide. Windows may normalize a top-edge window
        // to the taskbar work area during any intermediate move.
        await setPanelPinned(true);
        if (layout === "expanded") await setLayout("peek");

        const targetLayout: PanelLayout = "peek";
        const target = await topPinTarget(
          targetLayout,
          settingsStore.value.dynamicIslandCompatible,
        );
        await slideWindowTo(target.x, target.y, motionMs(MOTION.dock));
        dockSide = "top";

        if (layout === "peek") {
          // Grow the transparent native window first, then animate the visible
          // pill into the newly available space instead of jumping its width.
          await applyPanelLayout(
            "peek",
            lastPhysical,
            true,
            settingsStore.value.dynamicIslandCompatible,
          );
          await nextPaint();
          pinned = true;
          userPeeked = true;
          const wait = motionMs(MOTION.strip);
          if (wait) await sleep(wait);
        } else {
          await applyPanelLayout(
            "collapsed",
            lastPhysical,
            true,
            settingsStore.value.dynamicIslandCompatible,
          );
          pinned = true;
          userPeeked = true;
          await setLayout("peek");
        }
      } else if (layout === "expanded") {
        userCollapsedPill = false;
        await setLayout("collapsed");
        pinned = false;
        await applyPanelLayout("collapsed", lastPhysical, false, false);
      } else if (layout === "peek") {
        userCollapsedPill = false;
        // Shrink the visible card before trimming the native transparent bounds.
        pinned = false;
        await nextPaint();
        const wait = motionMs(MOTION.strip);
        if (wait) await sleep(wait);
        if (decision.peek) await applyPanelLayout("peek", lastPhysical, false, false);
        else await setLayout("collapsed");
      } else {
        userCollapsedPill = false;
        pinned = false;
        // A collapsed pinned orb still sits at the physical monitor top. Merely
        // disabling the guard leaves it underneath a reserved top app bar, so
        // explicitly dock it back inside the regular work area.
        await applyPanelLayout("collapsed", lastPhysical, false, false);
      }
    } finally {
      // Keep the guard active throughout the unpin animation, then release it
      // only after the native window has reached its unpinned layout.
      try {
        if (!next) await setPanelPinned(false);
      } finally {
        pinBusy = false;
        applyPolicy(true);
      }
    }
  }

  function queuePinned(next: boolean) {
    pinQueue = pinQueue
      .then(() => transitionPinned(next))
      .catch((error) => {
        console.error("Unable to update pinned panel", error);
        pinned = next;
        pinBusy = false;
        applyPolicy(true);
      });
    return pinQueue;
  }

  async function reanchorPinned(target: DockChanged) {
    if (!pinned || reanchorBusy) return;
    reanchorBusy = true;
    clearTimers();
    try {
      const current = await getCurrentWindow().outerPosition();
      lastPhysical = { x: current.x, y: current.y };
      const targetLayout = layout === "expanded" ? restingLayout() : layout;
      dockSide = target.side;
      await slideWindowTo(target.x, target.y, motionMs(MOTION.dock));
      await applyPanelLayout(
        targetLayout,
        lastPhysical,
        true,
        settingsStore.value.dynamicIslandCompatible,
      );
      settleLayout(targetLayout);
      userExpanded = targetLayout === "expanded";
      const final = await getCurrentWindow().outerPosition();
      lastPhysical = { x: final.x, y: final.y };
    } finally {
      reanchorBusy = false;
      applyPolicy(true);
    }
  }

  function queuePinnedDragPosition(x: number, y: number) {
    nextPinnedDragPosition = { x, y };
    if (pinnedDragPump) return;

    pinnedDragPump = (async () => {
      while (nextPinnedDragPosition) {
        const next = nextPinnedDragPosition;
        nextPinnedDragPosition = null;
        await movePanel(next.x, next.y);
      }
    })()
      .catch((error) => console.error("Unable to move pinned panel", error))
      .finally(() => {
        pinnedDragPump = null;
        if (nextPinnedDragPosition) {
          queuePinnedDragPosition(nextPinnedDragPosition.x, nextPinnedDragPosition.y);
        }
      });
  }

  function movePinnedWithPointer(event: PointerEvent, drag: PinnedPointerDrag) {
    const screenX = event.screenX;
    const screenY = event.screenY;
    void drag.windowStart.then((start) => {
      if (pinnedPointerDrag !== drag || !dragStarted) return;
      queuePinnedDragPosition(
        start.x + screenX - drag.screenX,
        start.y + screenY - drag.screenY,
      );
    });
  }

  async function releasePinnedPointer() {
    const drag = pinnedPointerDrag;
    if (drag?.element.hasPointerCapture(drag.pointerId)) {
      drag.element.releasePointerCapture(drag.pointerId);
    }
    pinnedPointerDrag = null;
    const pump = pinnedDragPump;
    if (pump) await pump;
  }

  function toggleFromOrb() {
    if (busy || dynamicIslandResizeBusy) return;
    clearTimers();
    const next = orbTargetLayout(layout);
    userCollapsedPill = next === "collapsed";
    userExpanded = next === "expanded";
    userPeeked = false;
    void setLayout(next);
  }

  function toggleDrawerFromPill() {
    if (busy || dynamicIslandResizeBusy || layout === "collapsed") return;
    clearTimers();
    clearDrawerIdleTimer();
    const next = pillTargetLayout(layout);
    userCollapsedPill = false;
    if (next === "expanded") {
      userExpanded = true;
      userPeeked = false;
      void setLayout(next);
      return;
    }
    void foldDrawerToPill();
  }

  function onKey(event: KeyboardEvent) {
    noteDrawerActivity(event);
    if (event.key === "Escape") {
      if (layout === "expanded") void foldDrawerToPill();
    }
    if (event.key === "Enter" || event.key === " ") {
      const target = event.target as HTMLElement | null;
      if (target?.closest("button, .drawer")) return;
      event.preventDefault();
      if (target?.closest("[data-pill-control]")) toggleDrawerFromPill();
      else toggleFromOrb();
    }
  }

  function onPointerDown(event: PointerEvent) {
    noteDrawerActivity(event);
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button.action, .drawer, .hit")) {
      canToggle = false;
      pointerToggleTarget = null;
      origin = null;
      return;
    }
    const onDragAfford = Boolean(target?.closest("[data-drag-afford]"));
    const onOrb = Boolean(target?.closest("[data-orb-control]"));
    const onPill = Boolean(target?.closest("[data-pill-control]"));
    dragStarted = false;
    pointerToggleTarget = onOrb ? "orb" : onPill ? "pill" : null;
    canToggle = pointerToggleTarget !== null;
    const canDrag = layout === "collapsed" || onDragAfford;
    origin = canDrag ? { x: event.clientX, y: event.clientY } : null;
    if (pinned && canDrag) {
      const element = event.currentTarget as HTMLElement;
      element.setPointerCapture(event.pointerId);
      const win = getCurrentWindow();
      pinnedPointerDrag = {
        pointerId: event.pointerId,
        element,
        screenX: event.screenX,
        screenY: event.screenY,
        windowStart: Promise.all([win.outerPosition(), win.scaleFactor()]).then(
          ([position, scale]) => ({ x: position.x / scale, y: position.y / scale }),
        ),
      };
    }
  }

  async function onPointerMove(event: PointerEvent) {
    noteDrawerActivity(event);
    if (pinnedPointerDrag && dragStarted) {
      movePinnedWithPointer(event, pinnedPointerDrag);
      return;
    }
    if (!origin || dragStarted) return;
    const dx = event.clientX - origin.x;
    const dy = event.clientY - origin.y;
    if (dx * dx + dy * dy < DRAG_PX * DRAG_PX) return;
    dragStarted = true;
    canToggle = false;
    pointerToggleTarget = null;
    snapPreview = true;
    if (pinnedPointerDrag) {
      movePinnedWithPointer(event, pinnedPointerDrag);
      return;
    }
    try {
      await getCurrentWindow().startDragging();
    } catch {
      dragStarted = false;
      canToggle = false;
      pointerToggleTarget = null;
      snapPreview = false;
    }
  }

  async function onPointerUp() {
    origin = null;
    await releasePinnedPointer();
    if (busy) {
      canToggle = false;
      pointerToggleTarget = null;
      return;
    }
    if (dragStarted) {
      dragStarted = false;
      canToggle = false;
      pointerToggleTarget = null;
      try {
        if (pinned) {
          const targetLayout = layout === "expanded" ? restingLayout() : layout;
          const result = await topPinTarget(
            targetLayout,
            settingsStore.value.dynamicIslandCompatible,
          );
          await reanchorPinned(result);
          return;
        }

        settleLayout("collapsed");
        userExpanded = false;
        userPeeked = false;
        const result = await dockAfterDrag(lastPhysical);
        dockSide = result.side;
        await slideWindowTo(result.x, result.y, motionMs(MOTION.dock));
        applyPolicy(true);
      } finally {
        snapPreview = false;
      }
      return;
    }
    if (!canToggle) return;
    canToggle = false;
    const target = pointerToggleTarget;
    pointerToggleTarget = null;
    if (target === "pill") toggleDrawerFromPill();
    else if (target === "orb") toggleFromOrb();
  }

  async function onAction() {
    const task = taskStore.surface.kind === "idle" ? undefined : taskStore.surface.task;
    if (task) await openTask(task.id);
  }

  async function onTaskOpen(task: TaskItem) {
    await openTask(task.id);
  }

  async function onDoubleClick() {
    if (settingsStore.value.doubleClickAction === "none") return;
    await openLatest();
  }

  async function onContextMenu(event: MouseEvent) {
    event.preventDefault();
    await popupPanelMenu();
  }

  async function syncSettingsFromBackend() {
    const next = await getSettings();
    const previous = settingsStore.value;
    const nextPinned = shouldPinPanel(next, taskStore.items);
    const dynamicIslandChanged =
      previous.dynamicIslandCompatible !== next.dynamicIslandCompatible;
    const animateDynamicIslandResize =
      dynamicIslandChanged &&
      pinned &&
      nextPinned &&
      (layout === "peek" || layout === "expanded");

    if (animateDynamicIslandResize) {
      await animateDynamicIslandWidth(
        previous.dynamicIslandCompatible,
        next.dynamicIslandCompatible,
        next,
      );
    }

    // The animation commits this snapshot after installing its width override.
    // Assigning it again is intentional: collapsed and interrupted paths still
    // converge on the latest persisted settings.
    settingsStore.value = next;
    if (!pinned && !nextPinned) dockSide = next.dockSide;
    await queuePinned(nextPinned);
  }

  function queueSettingsRefresh() {
    settingsQueue = settingsQueue
      .then(syncSettingsFromBackend)
      .catch((error) => {
        console.error("Unable to synchronize settings", error);
        synchronizedPanelWidth = undefined;
        synchronizedNativeResize = false;
        dynamicIslandResizeBusy = false;
        applyPolicy(true);
      });
    return settingsQueue;
  }

  onMount(() => {
    document.documentElement.classList.add("overlay");
    // The Windows WebView also disables Blink's native autoscroll at startup.
    // Keep this platform-neutral guard for WKWebView and auxiliary-link actions.
    window.addEventListener("pointerdown", blockMiddleButtonDefault, true);
    window.addEventListener("mousedown", blockMiddleButtonDefault, true);
    window.addEventListener("auxclick", blockMiddleButtonDefault, true);

    const unlistenTasks = listen<TaskItem[]>("tasks-updated", (event) => {
      const finishedLastRunning = didFinishLastRunning(taskStore.items, event.payload);
      const policyChanged = taskPolicyKey(taskStore.items) !== taskPolicyKey(event.payload);
      taskStore.items = event.payload;
      if (policyChanged) userCollapsedPill = false;
      if (finishedLastRunning) userExpanded = false;
      void queuePinned(shouldPinPanel(settingsStore.value, event.payload)).then(() => {
        applyPolicy(true);
      });
    });
    const unlistenLayout = listen<string>("panel-layout", (event) => {
      if (event.payload === "expanded" || event.payload === "peek" || event.payload === "collapsed") {
        if (event.payload === "collapsed") surfaceAnchorSide = visualDockSide;
        settleLayout(event.payload);
        userExpanded = event.payload === "expanded";
        userPeeked = false;
      }
    });
    const unlistenDock = listen<{ side: DockSide }>("dock-changed", (event) => {
      dockSide = event.payload.side;
      snapPreview = false;
    });
    const unlistenSettings = listen("settings-changed", () => {
      void queueSettingsRefresh();
    });
    const unlistenTray = listen<string>("tray-action", async (event) => {
      if (event.payload === "settings") await openSettings();
      if (event.payload === "mute") settingsStore.value = await muteHour();
      if (event.payload === "focus") {
        settingsStore.value = await updateSettings({ focusMode: !settingsStore.value.focusMode });
      }
    });
    const unlistenMoved = getCurrentWindow().onMoved((event) => {
      lastPhysical = { x: event.payload.x, y: event.payload.y };
      if (!dragStarted || pinnedPointerDrag) return;
      void previewDock().then((side) => {
        snapPreview = side !== null;
      });
    });
    const unlistenFocus = getCurrentWindow().onFocusChanged((event) => {
      windowFocused = event.payload;
      if (!windowFocused && layout === "expanded") void collapseExpandedAfterBlur();
    });
    const unlistenPinnedReanchor = listen<DockChanged>("pinned-reanchor", (event) => {
      origin = null;
      canToggle = false;
      pointerToggleTarget = null;
      dragStarted = false;
      snapPreview = false;
      void reanchorPinned(event.payload);
    });

    window.addEventListener("keydown", onKey);

    void (async () => {
      const initialSettings = await getSettings();
      settingsStore.value = initialSettings;
      dockSide = initialSettings.dockSide;
      surfaceAnchorSide = initialSettings.dockSide;
      taskStore.items = await listTasks();
      await setLayout("collapsed", true);
      await queuePinned(shouldPinPanel(initialSettings, taskStore.items));
      applyPolicy(true);
    })();

    return () => {
      document.documentElement.classList.remove("overlay");
      clearTimers();
      clearDrawerIdleTimer();
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", blockMiddleButtonDefault, true);
      window.removeEventListener("mousedown", blockMiddleButtonDefault, true);
      window.removeEventListener("auxclick", blockMiddleButtonDefault, true);
      void unlistenTasks.then((fn) => fn());
      void unlistenLayout.then((fn) => fn());
      void unlistenDock.then((fn) => fn());
      void unlistenSettings.then((fn) => fn());
      void unlistenTray.then((fn) => fn());
      void unlistenMoved.then((fn) => fn());
      void unlistenFocus.then((fn) => fn());
      void unlistenPinnedReanchor.then((fn) => fn());
    };
  });
</script>

<main
  class="overlay-root"
  onpointerdowncapture={noteDrawerActivity}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={() => {
    origin = null;
    canToggle = false;
    pointerToggleTarget = null;
    dragStarted = false;
    void releasePinnedPointer();
  }}
  onwheel={noteDrawerActivity}
>
  <WorkPanel
    surface={taskStore.surface}
    tasks={taskStore.items}
    dockSide={visualDockSide}
    orbAnchorSide={pinned ? "top" : surfaceAnchorSide}
    {layout}
    {pinned}
    dynamicIslandCompatible={settingsStore.value.dynamicIslandCompatible}
    widthOverride={synchronizedPanelWidth}
    {synchronizedNativeResize}
    {snapPreview}
    {flow}
    {motionFrame}
    sideVariant="strip"
    fillWindow
    ondblclick={onDoubleClick}
    oncontextmenu={onContextMenu}
    onaction={onAction}
    ontaskopen={onTaskOpen}
  />
</main>

<style>
  .overlay-root {
    width: 100%;
    height: 100%;
    background: transparent;
  }

  .overlay-root :global(.shell) {
    width: 100%;
    height: 100%;
  }
</style>
