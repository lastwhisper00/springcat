<script lang="ts">
  import type { SurfaceState, TaskSource } from "$domain";
  import { dockMotion } from "./dock-sources";
  import ToolLogo from "./ToolLogo.svelte";

  let {
    surface,
    source = null,
    swapKey,
    size = 44,
    drag = false,
  }: {
    surface: SurfaceState;
    source?: TaskSource | null;
    swapKey?: string;
    size?: number;
    drag?: boolean;
  } = $props();

  const motion = $derived(dockMotion(surface));
  const activeKey = $derived(swapKey ?? source ?? "springcat");
</script>

<span
  class="orb"
  class:drag
  data-motion={motion}
  data-source={source ?? "springcat"}
  data-drag-afford={drag ? "true" : undefined}
  title={drag ? "拖到顶 / 左 / 右吸附" : undefined}
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
>
  <span class="ring"></span>
  <span class="runner"><i></i></span>
  <span class="face">
    {#key activeKey}
      <span class="logo-swap"><ToolLogo {source} /></span>
    {/key}
  </span>
  {#if motion === "done"}
    <span class="unread-dot"></span>
  {/if}
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
      transform 180ms var(--sc-ease, cubic-bezier(0.22, 1, 0.36, 1));
    will-change: width, height, transform;
  }

  .orb.drag {
    cursor: grab;
  }

  .ring,
  .runner {
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

  .unread-dot {
    position: absolute;
    z-index: 2;
    top: -2px;
    right: -2px;
    width: 9px;
    height: 9px;
    border: 2px solid #15191f;
    border-radius: 50%;
    background: var(--sc-completed);
    box-shadow: 0 0 7px color-mix(in srgb, var(--sc-completed) 72%, transparent);
    pointer-events: none;
  }

  @keyframes sc-orbit {
    to {
      transform: rotate(360deg);
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
    .logo-swap {
      animation: none !important;
    }

    .orb[data-motion="idle"] .ring {
      opacity: 0.58;
    }
  }
</style>
