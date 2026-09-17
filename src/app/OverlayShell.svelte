<script lang="ts">
  import { flushSync, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import WorkPanel from "$components/work-panel/WorkPanel.svelte";
  import { shellSize } from "$components/work-panel/copy";
  import type { PanelPage } from "$components/work-panel/panel-layout";
  import {
    closeCapsulePlan,
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
    synchronizedResizeEase,
  } from "./synchronized-resize";
  import {
    applyPanelLayout,
    dockAfterDrag,
    getSettings,
    getAppMeta,
    listTasks,
    markAllRead,
    movePanel,
    muteHour,
    openLatest,
    openSettings,
    openTask,
    popupPanelMenu,
    preparePanelLayout,
    previewDock,
    resizePinnedPanel,
    resizePanelFrame,
    setPanelPinned,
    topPinTarget,
    uiLog,
    updateSettings,
    type DockChanged,
  } from "$services/tauri";
  import { blockMiddleButtonDefault } from "./pointer-guards";
  import { resolveWindowKind } from "./window-kind";
  import {
    drawerIdleTarget,
    orbTargetLayout,
    pillTargetLayout,
    suppressUserCollapsedAutoOpen,
    taskPolicyKey,
  } from "./orb-interaction";

  const DRAG_PX = 6;
  // A drawer the user opened deliberately stays open until they dismiss it
  // (pill click, Escape, or window blur). Only policy-opened drawers fold on
  // inactivity — collapsing a list out from under a reader is the most jarring
  // kind of motion there is.
  const DRAWER_IDLE_MS = 6_000;
  // Hover-peek debounce: opening needs a short dwell so a fast mouse sweep
  // across the orb does not flap the native window open; closing is immediate
  // once the pointer actually leaves the window.
  const HOVER_OPEN_MS = 140;
  const isOverlayWindow = resolveWindowKind() === "overlay";
  let fixedNativeSurface = $state(false);
  let widgetsSupported = $state(false);
  let panelPage = $state<PanelPage>("tasks");
  let headerActionBusy = $state(false);
  let suppressAutoPin = false;
  let manualPinTarget: boolean | null = null;

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
  let busy = $state(false);
  let dynamicIslandResizeBusy = $state(false);
  let pinBusy = $state(false);
  let reanchorBusy = $state(false);
  let policyPending = false;
  let dragStarted = $state(false);
  let canToggle = false;
  let pointerToggleTarget: "orb" | "pill" | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let drawerIdleTimer: ReturnType<typeof setTimeout> | undefined;
  let hoverOpenTimer: ReturnType<typeof setTimeout> | undefined;
  // After a capsule collapse, the hover-peek stays suppressed briefly. A
  // pointer sweeping near the freshly collapsed orb would otherwise re-run
  // the whole open/close native surface dance every pass — the flapping that
  // reads as constant flicker.
  let hoverPeekSuppressedUntil = 0;
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
  // True while the collapsed orb is showing the peek pill only because the
  // pointer is resting on it. While hover-peeking, the panel owes its shape to
  // the hover, so clicks are interpreted from the collapsed state and the
  // panel collapses again as soon as the pointer leaves.
  let hoverPeeking = $state(false);
  let pointerInsidePanel = $state(false);
  // True while the pointer is inside the expanded drawer. The drawer never
  // folds itself under an active cursor; it only folds after the pointer has
  // left (or after keyboard/pointer activity elsewhere) for DRAWER_IDLE_MS.
  let drawerHovered = $state(false);

  const muted = $derived(isMuted(settingsStore.value));
  // Pinning is a physical top-center mode. Keep the visual choreography tied
  // to that invariant even while settings/HMR events are reconciling the last
  // unpinned dock side in the background.
  const visualDockSide = $derived<DockSide>(pinned || panelPage === "widgets" ? "top" : dockSide);
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
      panelPage,
    );
  }

  function prepareNativeLayout(next: PanelLayout) {
    return preparePanelLayout(
      next,
      lastPhysical,
      pinned,
      pinned ? settingsStore.value.dynamicIslandCompatible : false,
      panelPage,
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
      panelPage,
    );
    const to = shellSize(
      visualDockSide,
      toLayout,
      "strip",
      pinned,
      pinned && settingsStore.value.dynamicIslandCompatible,
      panelPage,
    );

    // Keep the native window out of the animation loop. Resizing an HWND every
    // frame forces WebView2 to recreate its composition surface and is the
    // reason the desktop build feels rough even when the CSS demo is smooth.
    // The window is prepared and resized once; the card itself then grows or
    // folds on the compositor at 60fps.
    flushSync(() => {
      flow = expanding ? "unfolding" : "folding";
      layout = "expanded";
      synchronizedPanelWidth = from.width;
      motionFrame = {
        stage: expanding ? "strip" : "panel",
        ball: "inner",
      };
    });
    await nextPaint();
    await nextPaint();
    if (expanding) {
      if (from.width > to.width) {
        // Notch-compatible capsules can be wider than the list. Keep both
        // endpoints inside the canvas until the visible card has contracted.
        await resizePanelFrame(from.width, to.height, pinned);
      } else {
        await prepareNativeLayout(toLayout);
        await applyNativeLayout(toLayout);
      }
      await nextPaint();
      await nextPaint();
      flushSync(() => {
        synchronizedPanelWidth = to.width;
        motionFrame = { stage: "panel", ball: "inner" };
      });
    } else {
      if (to.width > from.width) {
        // The compatibility capsule may be wider than the list. Reserve its
        // full width before animating, and only trim the height at the end.
        await resizePanelFrame(to.width, from.height, pinned);
        await nextPaint();
        await nextPaint();
      }
      flushSync(() => {
        synchronizedPanelWidth = to.width;
        motionFrame = { stage: "strip", ball: "inner" };
      });
    }

    try {
      await waitMotionBeat(motionMs(expanding ? MOTION.panel : MOTION.fold));
      settleLayout(toLayout);
      await applyNativeLayout(toLayout);
      const final = await getCurrentWindow().outerPosition();
      lastPhysical = { x: final.x, y: final.y };
      await nextPaint();
    } finally {
      flushSync(() => {
        synchronizedPanelWidth = undefined;
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
    const from = shellSize("top", animatedLayout, "strip", true, previousCompatible, panelPage);
    const to = shellSize("top", animatedLayout, "strip", true, nextCompatible, panelPage);
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
        panelPage,
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
    if (pinned) return;
    if (dragStarted || pinnedPointerDrag) return;
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
        // Window slides ride the same spatial curve as every other surface
        // motion, so docking reads as one continuous gesture with the panel.
        const eased = synchronizedResizeEase(p);
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

  /** The only peek -> collapsed implementation. Drawer closing never enters here. */
  async function collapsePeekCapsule() {
    if (layout !== "peek") {
      throw new Error(`capsule collapse requires peek layout, received ${layout}`);
    }

    await playMotion("closing", closeCapsulePlan());
    surfaceAnchorSide = visualDockSide;
    settleLayout("collapsed");
    userExpanded = false;
    userPeeked = false;
    // Paint the resting orb in a surface whose width already matches the
    // final 48 px native clip. Keeping the old wide surface here makes the
    // collapsed CSS resolve against that stale viewport on WebView2, so a
    // side-docked window can clip a blank slice after contraction.
    await nextPaint();
    try {
      await prepareNativeLayout("collapsed");
    } catch (error) {
      uiLog(`collapse prepare FAILED: ${String(error)}`);
    }
    // The WebView must not only lay out at the shrunken width but have its
    // frame PRESENTED before the parent HWND exposes it. Two rAFs were not
    // always enough under GPU load, which is what made the orb vanish after
    // a collapse; the extra settle beat closes that window.
    await nextPaint();
    await sleep(40);
    await nextPaint();
    hoverPeekSuppressedUntil = performance.now() + 400;
    try {
      await applyNativeLayout("collapsed");
    } catch (error) {
      uiLog(`collapse apply FAILED: ${String(error)}`);
    }
  }

  async function setLayout(next: PanelLayout, force = false) {
    if (dynamicIslandResizeBusy) {
      policyPending = true;
      uiLog(`setLayout ${next} -> blocked: island-resize`);
      return;
    }
    if (busy) {
      uiLog(`setLayout ${next} -> blocked: busy (layout=${layout} flow=${flow})`);
      return;
    }
    if (!force && next === layout && flow === "idle") return;
    busy = true;
    uiLog(`setLayout ${next} start (from layout=${layout})`);
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
        // Every full close converges through the same two state transitions:
        // expanded -> peek folds/resizes the drawer first; only a real peek
        // window is allowed to run the capsule -> orb animation afterward.
        if (layout === "expanded") {
          await animateDrawerWindow(false);
          // Give the real peek surface one committed frame before the capsule
          // timeline starts, so the two phases cannot visually merge.
          await nextPaint();
        }
        await collapsePeekCapsule();
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
      uiLog(`setLayout ${next} done (layout=${layout} flow=${flow})`);
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
    if (pinned) return;
    if (userExpanded) return;
    if (drawerHovered) return;
    if (layout !== "expanded" || flow !== "idle") return;
    drawerIdleTimer = setTimeout(() => {
      drawerIdleTimer = undefined;
      if (layout === "expanded" && flow === "idle" && !drawerHovered && !userExpanded) {
        void foldDrawerToPill();
      }
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
    // Pointer/keyboard activity inside the drawer counts as engagement: it
    // keeps the drawer open under the cursor and restarts the fold timer.
    drawerHovered = target?.closest(".drawer") ? true : false;
    armDrawerIdleTimer();
  }

  function onDrawerHoverChange(hovered: boolean) {
    drawerHovered = hovered;
    if (hovered) clearDrawerIdleTimer();
    else armDrawerIdleTimer();
  }

  $effect(() => {
    const drawerOpen = layout === "expanded" && flow === "idle";
    if (drawerOpen) armDrawerIdleTimer();
    else {
      drawerHovered = false;
      clearDrawerIdleTimer();
    }
  });

  function clearHoverTimers() {
    if (hoverOpenTimer) {
      clearTimeout(hoverOpenTimer);
      hoverOpenTimer = undefined;
    }
  }

  function cancelHoverPeek() {
    clearHoverTimers();
    hoverPeeking = false;
  }

  function onPanelPointerEnter(event: PointerEvent) {
    pointerInsidePanel = true;
    if (event.pointerType !== "mouse") return;
    if (hoverPeeking || busy || dynamicIslandResizeBusy || pinBusy || reanchorBusy || dragStarted) return;
    if (layout !== "collapsed" || flow !== "idle") return;
    if (taskStore.surface.kind === "idle") return;
    if (performance.now() < hoverPeekSuppressedUntil) return;
    clearHoverTimers();
    hoverOpenTimer = setTimeout(() => {
      hoverOpenTimer = undefined;
      if (busy || dynamicIslandResizeBusy || pinBusy || reanchorBusy || dragStarted) return;
      if (layout !== "collapsed" || flow !== "idle") return;
      if (taskStore.surface.kind === "idle") return;
      if (performance.now() < hoverPeekSuppressedUntil) return;
      uiLog("hover-peek opening");
      hoverPeeking = true;
      void setLayout("peek");
    }, HOVER_OPEN_MS);
  }

  function onPanelPointerLeave() {
    pointerInsidePanel = false;
  }

  // Collapse the hover-peek as soon as the pointer is out of the window and
  // the panel has settled — including when the pointer left mid-motion.
  $effect(() => {
    if (hoverPeeking && layout !== "peek") {
      hoverPeeking = false;
      return;
    }
    if (flow !== "idle" || !hoverPeeking || pointerInsidePanel) return;
    if (busy || dynamicIslandResizeBusy || pinBusy || reanchorBusy || dragStarted) return;
    if (userExpanded || userPeeked) return;
    hoverPeeking = false;
    if (layout === "peek") void setLayout("collapsed");
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
    uiLog(
      `applyPolicy fromEvent=${fromEvent} layout=${layout} userExpanded=${userExpanded} userPeeked=${userPeeked} hoverPeeking=${hoverPeeking} collapsedPill=${userCollapsedPill}`,
    );
    // A hover-peek is a user-initiated, transient look: let it stay open for
    // as long as the pointer is on it instead of auto-hiding underneath it.
    if (hoverPeeking && layout === "peek") return;
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
      uiLog(`autoHide armed ${remainingMs}ms`);
      hideTimer = setTimeout(() => {
        hideTimer = undefined;
        if (!userExpanded && !userPeeked && !hoverPeeking && layout === "peek") {
          uiLog("autoHide firing");
          void setLayout("collapsed");
        }
      }, remainingMs);
    }
  }

  async function transitionPinned() {
    await waitForPanelIdle();
    const next = desiredPin();
    uiLog(`transitionPinned ${next} (layout=${layout})`);
    // The native top guard is an effective runtime state, not just the saved
    // manual preference. Re-sync even when the visual state already matches:
    // changing the manual preference while an auto-pin is active can otherwise
    // disable the guard underneath the still-pinned task pill.
    if (next === pinned) {
      await setPanelPinned(next);
      if (next) dockSide = "top";
      return;
    }
    const restoreExpanded = layout === "expanded";
    pinBusy = true;
    clearTimers();
    try {
      // Header pinning keeps the current page open. When already at the top,
      // only the native pin guard/placement changes; content does not fold.
      if (restoreExpanded && visualDockSide === "top") {
        await setPanelPinned(next);
        pinned = next;
        dockSide = "top";
        userExpanded = true;
        userPeeked = false;
        await applyNativeLayout("expanded");
        return;
      }
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
          panelPage,
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
      if (restoreExpanded && layout !== "expanded") {
        userExpanded = true;
        await setLayout("expanded");
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

  function desiredPin(settings = settingsStore.value) {
    return manualPinTarget ?? shouldPinPanel(settings, taskStore.items, suppressAutoPin);
  }

  function queuePinned() {
    pinQueue = pinQueue
      .then(() => transitionPinned())
      .catch((error) => {
        console.error("Unable to update pinned panel", error);
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
      const targetLayout = layout;
      dockSide = target.side;
      await slideWindowTo(target.x, target.y, motionMs(MOTION.dock));
      await applyPanelLayout(
        targetLayout,
        lastPhysical,
        true,
        settingsStore.value.dynamicIslandCompatible,
        panelPage,
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
    if (busy || dynamicIslandResizeBusy || headerActionBusy) {
      uiLog(`orb toggle dropped (busy=${busy} island=${dynamicIslandResizeBusy} layout=${layout})`);
      return;
    }
    clearTimers();
    clearHoverTimers();
    // The orb is the master toggle. A click while hover-peeking must expand
    // (the user's baseline was "collapsed"), never collapse the peek again.
    const next = orbTargetLayout(hoverPeeking ? "collapsed" : layout);
    uiLog(`orb toggle -> ${next} (layout=${layout} hoverPeeking=${hoverPeeking})`);
    hoverPeeking = false;
    userCollapsedPill = next === "collapsed";
    userExpanded = next === "expanded";
    userPeeked = false;
    void setLayout(next);
  }

  function toggleDrawerFromPill() {
    if (busy || dynamicIslandResizeBusy || headerActionBusy || layout === "collapsed") {
      uiLog(`pill toggle dropped (busy=${busy} layout=${layout})`);
      return;
    }
    clearTimers();
    clearDrawerIdleTimer();
    const next = pillTargetLayout(layout);
    uiLog(`pill toggle -> ${next} (layout=${layout})`);
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
    if (headerActionBusy) return;
    if (event.key === "Escape") {
      if (layout === "expanded" && panelPage === "widgets") {
        void togglePanelPage();
        return;
      }
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
    clearHoverTimers();
    if (event.button !== 0) return;
    if (busy || dynamicIslandResizeBusy || headerActionBusy || flow !== "idle") return;
    const target = event.target as HTMLElement | null;
    uiLog(`pointerdown tag=${target?.tagName} pill=${Boolean(target?.closest("[data-pill-control]"))} orb=${Boolean(target?.closest("[data-orb-control]"))} drag=${Boolean(target?.closest("[data-drag-afford]"))} layout=${layout} flow=${flow} busy=${busy}`);
    if (target?.closest("button, a, input, select, .drawer, .hit")) {
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
    if (layout === "expanded" && canDrag) {
      userExpanded = true;
      clearDrawerIdleTimer();
    }
    // Capture list-header dragging ourselves. Native startDragging can eat
    // pointerup on Windows and used to leave an expanded list stuck in drag.
    if ((pinned || layout === "expanded") && canDrag) {
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
    uiLog(`drag start pinned=${pinned} layout=${layout}`);
    cancelHoverPeek();
    if (pinnedPointerDrag) {
      movePinnedWithPointer(event, pinnedPointerDrag);
      return;
    }
    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      uiLog(`native drag FAILED: ${String(error)}`);
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
          const targetLayout = layout;
          const result = await topPinTarget(
            targetLayout,
            settingsStore.value.dynamicIslandCompatible,
            panelPage,
          );
          await reanchorPinned(result);
          return;
        }

        if (layout === "expanded") {
          const win = getCurrentWindow();
          const position = await win.outerPosition();
          lastPhysical = { x: position.x, y: position.y };
          userExpanded = true;
          await applyNativeLayout("expanded");
          return;
        }

        settleLayout("collapsed");
        userExpanded = false;
        userPeeked = false;
        const result = await dockAfterDrag(lastPhysical);
        // Side-docked compact forms reopen the task page; the wider widgets
        // are top-only and must not retain a top anchor on a side native clip.
        if (result.side !== "top") panelPage = "tasks";
        dockSide = result.side;
        await slideWindowTo(result.x, result.y, motionMs(MOTION.dock));
        applyPolicy(true);
      } finally {
        snapPreview = false;
      }
      return;
    }
    if (!canToggle) {
      uiLog(`pointerup: no toggle target (busy=${busy} layout=${layout})`);
      return;
    }
    canToggle = false;
    const target = pointerToggleTarget;
    pointerToggleTarget = null;
    uiLog(`pointerup -> toggle ${target}`);
    if (target === "pill") toggleDrawerFromPill();
    else if (target === "orb") toggleFromOrb();
  }

  let operationNotice = $state("");

  async function toggleHeaderPin() {
    if (headerActionBusy || busy || pinBusy || dynamicIslandResizeBusy || flow !== "idle") return;
    const next = !pinned;
    const previousSuppression = suppressAutoPin;
    headerActionBusy = true;
    manualPinTarget = next;
    suppressAutoPin = !next && taskStore.items.some(task => task.status === "running");
    userExpanded = true;
    operationNotice = "";
    try {
      settingsStore.value = await updateSettings({ alwaysOnTop: next });
      await queuePinned();
      if (pinned !== next) operationNotice = "置顶状态未能更新，请重试";
    } catch {
      suppressAutoPin = previousSuppression;
      operationNotice = "未能保存置顶状态，请重试";
    } finally {
      manualPinTarget = null;
      headerActionBusy = false;
    }
  }

  async function togglePanelPage() {
    if (!widgetsSupported || headerActionBusy || busy || pinBusy || dynamicIslandResizeBusy || flow !== "idle" || layout !== "expanded") return;
    const next: PanelPage = panelPage === "tasks" ? "widgets" : "tasks";
    const previousPage = panelPage;
    headerActionBusy = true;
    clearTimers();
    clearDrawerIdleTimer();
    userExpanded = true;
    operationNotice = "";
    try {
      if (next === "widgets" && visualDockSide !== "top") {
        reanchorBusy = true;
        // Reposition the small orb first so a side-docked canvas never needs
        // to paint the wider widget page outside its native clipping bounds.
        await setLayout("collapsed");
        const target = await topPinTarget("collapsed", settingsStore.value.dynamicIslandCompatible);
        await slideWindowTo(target.x, target.y, motionMs(MOTION.dock));
        dockSide = "top";
        surfaceAnchorSide = "top";
        panelPage = next;
        userExpanded = true;
        await setLayout("expanded");
        return;
      }
      const from = shellSize("top", "expanded", "strip", pinned, settingsStore.value.dynamicIslandCompatible, panelPage);
      const to = shellSize("top", "expanded", "strip", pinned, settingsStore.value.dynamicIslandCompatible, next);
      busy = true;
      synchronizedPanelWidth = from.width;
      flow = "unfolding";
      await nextPaint();
      await nextPaint();
      if (to.width > from.width || to.height > from.height) {
        // Pages trade width for height. Reserve both endpoints before the CSS
        // transition, then trim the outer clip only after both dimensions settle.
        await resizePanelFrame(Math.max(from.width, to.width), Math.max(from.height, to.height), pinned);
        const position = await getCurrentWindow().outerPosition();
        lastPhysical = { x: position.x, y: position.y };
        await nextPaint();
        await nextPaint();
      }
      flushSync(() => {
        dockSide = "top";
        panelPage = next;
        synchronizedPanelWidth = to.width;
      });
      await waitMotionBeat(MOTION.panel);
      await applyNativeLayout("expanded");
      const position = await getCurrentWindow().outerPosition();
      lastPhysical = { x: position.x, y: position.y };
    } catch {
      panelPage = previousPage;
      operationNotice = "暂时无法切换页面，请重试";
      await applyNativeLayout(layout).catch(() => undefined);
    } finally {
      synchronizedPanelWidth = undefined;
      settleLayout(layout);
      busy = false;
      headerActionBusy = false;
      reanchorBusy = false;
      if (policyPending) applyPolicy(true);
    }
  }
  async function onTaskOpen(task: TaskItem) {
    operationNotice = "";
    try {
      await openTask(task.id);
    } catch {
      operationNotice = "暂时无法打开来源，请重试或在原工具中查找";
    }
  }

  async function onMarkAllRead() {
    operationNotice = "";
    try {
      await markAllRead();
    } catch {
      operationNotice = "未能标记已读，请稍后重试";
    }
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
    const nextPinned = desiredPin(next);
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
    await queuePinned();
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
    if (!isOverlayWindow) return;
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
      if (finishedLastRunning) {
        suppressAutoPin = false;
        if (!pinned && panelPage !== "widgets") userExpanded = false;
      }
      void queuePinned().then(() => {
        applyPolicy(true);
      });
    });
    const unlistenLayout = listen<string>("panel-layout", (event) => {
      uiLog(`panel-layout event: ${event.payload}`);
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
      uiLog(`focus ${event.payload} (layout=${layout})`);
      windowFocused = event.payload;
      if (!windowFocused && layout === "expanded") void collapseExpandedAfterBlur();
    });
    const unlistenPinnedReanchor = listen<DockChanged>("pinned-reanchor", (event) => {
      // A paused pointer is still dragging. The native move watchdog must
      // not steal a captured header drag before the user releases it.
      if (pinnedPointerDrag) return;
      origin = null;
      canToggle = false;
      pointerToggleTarget = null;
      dragStarted = false;
      snapPreview = false;
      void reanchorPinned(event.payload);
    });

    window.addEventListener("keydown", onKey);

    void (async () => {
      const [initialSettings, meta] = await Promise.all([getSettings(), getAppMeta()]);
      fixedNativeSurface = meta.fixedOverlaySurface === true;
      widgetsSupported = meta.widgetPanelSupported === true;
      settingsStore.value = initialSettings;
      dockSide = initialSettings.dockSide;
      surfaceAnchorSide = initialSettings.dockSide;
      taskStore.items = await listTasks();
      await setLayout("collapsed", true);
      await queuePinned();
      applyPolicy(true);
    })();

    return () => {
      document.documentElement.classList.remove("overlay");
      clearTimers();
      clearDrawerIdleTimer();
      clearHoverTimers();
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

{#if isOverlayWindow}
<main
  class="overlay-root"
  onpointerdowncapture={noteDrawerActivity}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointerenter={onPanelPointerEnter}
  onpointerleave={onPanelPointerLeave}
  onpointercancel={() => {
    origin = null;
    canToggle = false;
    pointerToggleTarget = null;
    dragStarted = false;
    cancelHoverPeek();
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
    {panelPage}
    {widgetsSupported}
    controlsBusy={headerActionBusy || busy || pinBusy || dynamicIslandResizeBusy || flow !== "idle"}
    dynamicIslandCompatible={settingsStore.value.dynamicIslandCompatible}
    widthOverride={synchronizedPanelWidth}
    {synchronizedNativeResize}
    {snapPreview}
    {flow}
    {motionFrame}
    sideVariant="strip"
    fillWindow
    {fixedNativeSurface}
    onhoverchange={onDrawerHoverChange}
    ondblclick={onDoubleClick}
    oncontextmenu={onContextMenu}
    notice={operationNotice}
    onpintoggle={toggleHeaderPin}
    onpagetoggle={togglePanelPage}
    onmarkallread={onMarkAllRead}
    ontaskopen={onTaskOpen}
  />
</main>
{/if}

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
