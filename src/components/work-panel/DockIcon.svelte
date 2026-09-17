<script lang="ts">
  import type { SurfaceState, TaskSource } from "$domain";
  import ToolLogo from "./ToolLogo.svelte";

  let {
    surface,
    source = null,
    executing,
    additionalSources = 0,
    size = 44,
    drag = false,
  }: {
    surface: SurfaceState;
    source?: TaskSource | null;
    executing?: boolean;
    additionalSources?: number;
    size?: number;
    drag?: boolean;
  } = $props();

  const isExecuting = $derived(executing ?? surface.kind === "working");
</script>

<span
  class="orb"
  class:drag
  data-executing={isExecuting}
  data-indicator={isExecuting ? "active" : "idle"}
  data-source={source ?? "springcat"}
  data-drag-afford={drag ? "true" : undefined}
  title={drag ? "拖到顶 / 左 / 右吸附" : undefined}
  style:width="{size}px"
  style:height="{size}px"
  aria-hidden="true"
>
  <span class="ring"></span>
  <span class="orbit"><i></i></span>
  <span class="face"><ToolLogo {source} /></span>
  {#if additionalSources > 0}
    <span class="source-count">+{additionalSources}</span>
  {/if}
</span>

<style>
  .orb {
    position: relative;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    border-radius: 50%;
    color: var(--sc-orb-text, #f1f2f4);
    background: var(--sc-orb-bg, #23262b);
    border: 1px solid var(--sc-orb-line, #555e6a);
    overflow: visible;
    flex-shrink: 0;
    transition:
      width var(--sc-step-strip, 300ms) var(--sc-ease-spatial, cubic-bezier(0.32, 0.72, 0, 1)),
      height var(--sc-step-strip, 300ms) var(--sc-ease-spatial, cubic-bezier(0.32, 0.72, 0, 1));
  }

  .orb.drag {
    cursor: grab;
  }

  .orb.drag:active {
    cursor: grabbing;
  }

  .ring,
  .orbit {
    position: absolute;
    inset: -2px;
    border-radius: 50%;
    pointer-events: none;
    transition: opacity 180ms ease;
  }

  .ring {
    border: 1px solid var(--sc-orb-working, #97b7dd);
    opacity: 0.75;
    box-shadow: 0 0 4px color-mix(in srgb, var(--sc-orb-working, #97b7dd) 24%, transparent);
  }

  .orb[data-indicator="idle"] .ring {
    animation: sc-idle-glow 3.2s ease-in-out infinite;
  }

  .orbit {
    opacity: 0;
  }

  .orbit i {
    position: absolute;
    top: -1px;
    left: 50%;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--sc-orb-working, #97b7dd) 35%, white);
    box-shadow: 0 0 4px color-mix(in srgb, var(--sc-orb-working, #97b7dd) 50%, transparent);
    transform: translateX(-50%);
  }

  .orb[data-indicator="active"] .orbit {
    opacity: 1;
    animation: sc-active-orbit 1.8s linear infinite;
  }

  @keyframes sc-idle-glow {
    0%, 100% { opacity: 0.3; }
    50% { opacity: 0.9; }
  }

  @keyframes sc-active-orbit {
    to { transform: rotate(360deg); }
  }

  .face {
    width: 58%;
    height: 58%;
    display: grid;
    place-items: center;
  }

  .source-count {
    position: absolute;
    right: -5px;
    bottom: -2px;
    display: grid;
    place-items: center;
    min-width: 17px;
    height: 16px;
    padding: 0 3px;
    border-radius: 8px;
    border: 1px solid var(--sc-orb-line, #555e6a);
    color: var(--sc-orb-text, #f1f2f4);
    background: var(--sc-orb-bg, #23262b);
    font-size: 10px;
    font-weight: 500;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  @media (prefers-reduced-motion: reduce) {
    .orb,
    .ring,
    .orbit {
      transition: none;
      animation: none !important;
    }

    .orb[data-indicator="idle"] .ring { opacity: 0.65; }
    .orb[data-indicator="active"] .ring { opacity: 0.85; }
  }
</style>
