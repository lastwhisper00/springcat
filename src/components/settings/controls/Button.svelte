<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    variant = "secondary",
    danger = false,
    compact = false,
    disabled = false,
    onclick,
    children,
  }: {
    variant?: "primary" | "secondary" | "text";
    danger?: boolean;
    compact?: boolean;
    disabled?: boolean;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
  } = $props();
</script>

<button
  class="btn"
  class:primary={variant === "primary"}
  class:secondary={variant === "secondary"}
  class:text={variant === "text"}
  class:compact
  class:danger
  type="button"
  {disabled}
  {onclick}
>
  {@render children?.()}
</button>

<style>
  .btn {
    min-height: 34px;
    padding: 7px 11px;
    border-radius: 9px;
    font: inherit;
    cursor: pointer;
  }

  .primary {
    border: 1px solid var(--settings-accent);
    background: var(--settings-accent);
    color: var(--settings-accent-text);
    box-shadow: 0 5px 14px color-mix(in srgb, var(--settings-accent) 18%, transparent);
  }

  .secondary {
    border: 1px solid var(--settings-border);
    background: var(--settings-control);
    color: var(--sc-text);
  }

  .text {
    border: 0;
    background: transparent;
    color: var(--settings-accent);
  }

  .primary:hover,
  .secondary:hover {
    filter: brightness(1.04);
  }

  .compact {
    white-space: nowrap;
  }

  .danger {
    color: var(--sc-failed);
  }

  .btn:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
</style>
