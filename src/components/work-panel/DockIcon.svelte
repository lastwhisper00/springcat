<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { SurfaceState, TaskSource } from "$domain";
  import { dockMotion } from "./dock-sources";
  import ToolLogo from "./ToolLogo.svelte";

  let {
    surface,
    source = null,
    swapKey,
    flashKey,
    size = 44,
    drag = false,
  }: {
    surface: SurfaceState;
    source?: TaskSource | null;
    swapKey?: string;
    /** Identity token of the currently displayed task; changes trigger the attention flash. */
    flashKey?: string;
    size?: number;
    drag?: boolean;
  } = $props();

  const motion = $derived(dockMotion(surface));
  const activeKey = $derived(swapKey ?? source ?? "springcat");

  // Attention pulse when the displayed task identity changes (new task, or a
  // status transition like running → waiting). Never on the initial mount.
  let flash = $state(false);
  let prevFlashKey: string | null = null;
  let flashTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const key = flashKey;
    if (key !== undefined && key !== "idle" && prevFlashKey !== null && key !== prevFlashKey) {
      flash = true;
      if (flashTimer) clearTimeout(flashTimer);
      flashTimer = setTimeout(() => {
        flash = false;
        flashTimer = undefined;
      }, 900);
    }
    prevFlashKey = key ?? null;
  });

  // First-run drag guide: wiggle the orb once so new users discover that the
  // icon can be dragged to dock at another edge. Shown once per device.
  let dragHint = $state(false);
  let dragHintTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    try {
      if (localStorage.getItem("springcat-drag-hint-seen")) return;
      localStorage.setItem("springcat-drag-hint-seen", "1");
      dragHint = true;
      dragHintTimer = setTimeout(() => {
        dragHint = false;
        dragHintTimer = undefined;
      }, 2_600);
    } catch {
      // Private mode / storage unavailable: skip the hint.
    }
  });

  onDestroy(() => {
    if (flashTimer) clearTimeout(flashTimer);
    if (dragHintTimer) clearTimeout(dragHintTimer);
  });
</script>

<span
  class="orb"
  class:drag
  data-motion={motion}
  data-source={source ?? "springcat"}
  data-drag-afford={drag ? "true" : undefined}
  data-flash={flash ? "true" : undefined}
  data-hint={dragHint ? "true" : undefined}
  title={drag ? "拖到顶 / 左 / 右吸附" : undefined}
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
>
  <span class="ring"></span>
  <span class="runner"><i></i></span>
  <span class="flash-ring"></span>
  <span class="face">
    {#key activeKey}
      <span class="logo-swap"><ToolLogo {source} /></span>
    {/key}
  </span>
</span>

<style>
  .orb {
    --sc-ring-color: #fff;
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: #fff;
    background:
      radial-gradient(circle at 34% 27%, rgba(255, 255, 255, 0.16), transparent 34%),
      radial-gradient(circle at 50% 58%, #252a31 0 42%, #15191f 70%, #090b0e 100%);
    border: 1px solid rgba(255, 255, 255, 0.16);
    box-shadow:
      inset 0 1px 1px rgba(255, 255, 255, 0.16),
      inset 0 -5px 10px rgba(0, 0, 0, 0.28);
    overflow: visible;
    flex-shrink: 0;
    transition:
      width var(--sc-step-strip, 280ms) cubic-bezier(0.4, 0, 0.2, 1),
      height var(--sc-step-strip, 280ms) cubic-bezier(0.4, 0, 0.2, 1),
      transform 180ms var(--sc-ease, cubic-bezier(0.22, 1, 0.36, 1)),
      filter 220ms ease;
    will-change: width, height, transform, filter;
  }

  /* Completed-but-unread: a gentle lift of the whole ball instead of a badge. */
  .orb[data-motion="done"] {
    filter: brightness(1.24) saturate(1.05);
  }

  .orb.drag {
    cursor: grab;
  }

  .ring,
  .runner,
  .flash-ring {
    position: absolute;
    inset: -2px;
    border-radius: 50%;
    pointer-events: none;
  }

  .ring {
    border: 2px solid var(--sc-ring-color);
    opacity: 0.38;
    box-shadow: inset 0 0 2px rgba(255, 255, 255, 0.72);
    animation: sc-heartbeat 5.6s ease-in-out infinite;
  }

  /* One-shot attention pulse when a new task identity takes over the orb. */
  .flash-ring {
    border: 2px solid var(--sc-ring-color);
    opacity: 0;
    box-shadow: inset 0 0 3px rgba(255, 255, 255, 0.9);
  }

  .orb[data-flash="true"] .flash-ring {
    animation: sc-flash 900ms var(--sc-ease) both;
  }

  /* First-run nudge: two gentle wiggles tell the user the orb can be dragged
     to dock elsewhere. The ring pulses in sync. */
  .orb[data-hint="true"] {
    animation: sc-nudge-orb 2.6s ease both;
  }

  .orb[data-hint="true"] .ring {
    animation: sc-nudge-ring 2.6s ease both;
  }

  .runner {
    opacity: 0;
  }

  .runner i {
    position: absolute;
    top: -1.5px;
    left: 50%;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--sc-ring-color) 58%, white);
    box-shadow:
      0 0 3px white,
      0 0 5px 1px rgba(255, 255, 255, 0.7);
    transform: translateX(-50%);
  }

  .face {
    width: 58%;
    height: 58%;
    display: grid;
    place-items: center;
    z-index: 1;
  }

  .logo-swap {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    animation: sc-logo-swap 180ms var(--sc-ease) both;
  }

  .orb[data-motion="run"] .ring,
  .orb[data-motion="wait"] .ring,
  .orb[data-motion="fail"] .ring {
    opacity: 1;
    animation: none;
    box-shadow: inset 0 0 3px rgba(255, 255, 255, 0.9);
  }

  .orb[data-motion="run"] .runner,
  .orb[data-motion="wait"] .runner,
  .orb[data-motion="fail"] .runner {
    opacity: 1;
    animation: sc-orbit 1.35s linear infinite;
  }

  @keyframes sc-orbit {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes sc-flash {
    0% {
      opacity: 0;
      transform: scale(1);
    }
    22% {
      opacity: 0.95;
      transform: scale(1.16);
    }
    50% {
      opacity: 0.3;
      transform: scale(1.04);
    }
    76% {
      opacity: 0.75;
      transform: scale(1.12);
    }
    100% {
      opacity: 0;
      transform: scale(1);
    }
  }

  @keyframes sc-nudge-orb {
    0%,
    100% {
      transform: translateX(0);
    }
    10% {
      transform: translateX(-4px);
    }
    22% {
      transform: translateX(3px);
    }
    34% {
      transform: translateX(-2px);
    }
    46%,
    100% {
      transform: translateX(0);
    }
  }

  @keyframes sc-nudge-ring {
    0%,
    46% {
      opacity: 0.38;
      transform: scale(1);
    }
    10% {
      opacity: 0.85;
      transform: scale(1.05);
    }
    20% {
      opacity: 0.5;
    }
    32% {
      opacity: 0.8;
    }
    46%,
    100% {
      opacity: 0.38;
      transform: scale(1);
    }
  }

  @keyframes sc-heartbeat {
    0%,
    100% {
      transform: scale(0.98);
      opacity: 0.32;
    }
    30% {
      transform: scale(1.025);
      opacity: 0.9;
    }
    42% {
      transform: scale(1);
      opacity: 0.42;
    }
    54% {
      transform: scale(1.012);
      opacity: 0.66;
    }
    70% {
      transform: scale(0.98);
      opacity: 0.32;
    }
  }

  @keyframes sc-logo-swap {
    from {
      opacity: 0;
      transform: scale(0.76);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ring,
    .runner,
    .flash-ring,
    .logo-swap {
      animation: none !important;
    }

    .orb[data-hint="true"] {
      animation: none !important;
    }

    .orb[data-motion="idle"] .ring {
      opacity: 0.58;
    }
  }
</style>
