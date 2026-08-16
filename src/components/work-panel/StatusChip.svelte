<script lang="ts">
  import type { SurfaceState } from "$domain";

  let {
    kind,
    breathe = false,
  }: {
    kind: SurfaceState["kind"];
    breathe?: boolean;
  } = $props();
</script>

<span class="chip" data-kind={kind} data-breathe={breathe ? "yes" : "no"} aria-hidden="true">
  <span class="bar"></span>
  <span class="dot"></span>
</span>

<style>
  .chip {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .bar {
    width: 5px;
    align-self: stretch;
    min-height: 28px;
    border-radius: 999px;
    background: var(--sc-idle);
  }

  .dot {
    width: 8px;
    height: 8px;
    margin-left: 8px;
    border-radius: 50%;
    background: var(--sc-idle);
  }

  .chip[data-kind="working"] .bar,
  .chip[data-kind="working"] .dot {
    background: var(--sc-working);
  }
  .chip[data-kind="waiting"] .bar,
  .chip[data-kind="waiting"] .dot {
    background: var(--sc-waiting);
  }
  .chip[data-kind="completed"] .bar,
  .chip[data-kind="completed"] .dot {
    background: var(--sc-completed);
  }
  .chip[data-kind="failed"] .bar,
  .chip[data-kind="failed"] .dot {
    background: var(--sc-failed);
  }

  .chip[data-breathe="yes"][data-kind="working"] .dot {
    animation: sc-breathe 2.8s ease-in-out infinite;
  }

  @keyframes sc-breathe {
    0%,
    100% {
      opacity: 0.45;
      transform: scale(0.92);
    }
    50% {
      opacity: 1;
      transform: scale(1.08);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .chip[data-breathe="yes"] .dot {
      animation: none;
    }
  }
</style>
