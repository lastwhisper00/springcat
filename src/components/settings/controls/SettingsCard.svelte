<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    icon,
    title,
    description,
    children,
  }: {
    icon: "eye" | "activity" | "layers" | "plug";
    title: string;
    description?: string;
    children?: Snippet;
  } = $props();
</script>

<section class="settings-card">
  <div class="section-title">
    <span class="section-icon" aria-hidden="true">
      {#if icon === "eye"}
        <svg viewBox="0 0 20 20"><path d="M3.5 10s2.4-4 6.5-4 6.5 4 6.5 4-2.4 4-6.5 4-6.5-4-6.5-4Z" /><circle cx="10" cy="10" r="2" /></svg>
      {:else if icon === "activity"}
        <svg viewBox="0 0 20 20"><path d="M10 3v2m0 10v2M3 10h2m10 0h2M5 5l1.4 1.4M13.6 13.6 15 15m0-10-1.4 1.4M6.4 13.6 5 15" /><circle cx="10" cy="10" r="3.2" /></svg>
      {:else if icon === "layers"}
        <svg viewBox="0 0 20 20"><path d="M3.5 6.5 10 3l6.5 3.5L10 10 3.5 6.5Z" /><path d="m3.5 10 6.5 3.5 6.5-3.5M3.5 13.5 10 17l6.5-3.5" /></svg>
      {:else}
        <svg viewBox="0 0 20 20"><path d="M7 4v4m6-4v4M5 8h10v2a5 5 0 0 1-5 5 5 5 0 0 1-5-5V8Zm5 7v2" /></svg>
      {/if}
    </span>
    <div>
      <h2>{title}</h2>
      {#if description}<p>{description}</p>{/if}
    </div>
  </div>
  {@render children?.()}
</section>

<style>
  .settings-card {
    margin-bottom: 14px;
    padding: 17px;
    border: 1px solid var(--settings-border);
    border-radius: 16px;
    background: var(--settings-card);
    box-shadow: var(--settings-card-shadow);
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 11px;
    padding-bottom: 13px;
  }

  .section-icon {
    display: grid;
    width: 32px;
    height: 32px;
    flex: 0 0 32px;
    place-items: center;
    border-radius: 10px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
  }

  .section-icon svg {
    width: 17px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.6;
  }

  h2 {
    margin: 0;
    font-size: 12px;
  }

  .section-title p {
    margin: 3px 0 0;
    color: var(--sc-muted);
    font-size: 9px;
  }
</style>
