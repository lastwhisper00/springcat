<script lang="ts">
  import type { Snippet } from "svelte";

  export interface SelectOption {
    value: string;
    label: string;
    disabled?: boolean;
  }

  let {
    label,
    hint,
    value,
    options,
    ariaLabel,
    disabled = false,
    onchange,
    extra,
  }: {
    label: string;
    hint?: string;
    value: string;
    options: SelectOption[];
    ariaLabel?: string;
    disabled?: boolean;
    onchange?: (value: string) => void;
    /** Optional extra control rendered beside the select (e.g. a browse button). */
    extra?: Snippet;
  } = $props();
</script>

<label class="field-row">
  <span class="field-label">
    <strong>{label}</strong>
    {#if hint}<small>{hint}</small>{/if}
  </span>
  <span class="field-controls">
    <span class="select-wrap">
      <select
        {value}
        {disabled}
        aria-label={ariaLabel}
        onchange={(event) => onchange?.(event.currentTarget.value)}
      >
        {#each options as option (option.value)}
          <option value={option.value} disabled={option.disabled}>{option.label}</option>
        {/each}
      </select>
    </span>
    {#if extra}{@render extra()}{/if}
  </span>
</label>

<style>
  .field-row {
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(220px, 290px);
    align-items: center;
    gap: 20px;
    min-height: 57px;
    border-bottom: 1px solid var(--settings-border);
  }

  .field-row:last-child {
    border-bottom: 0;
  }

  .field-label {
    display: block;
  }

  .field-label strong {
    display: block;
    font-size: 10.5px;
    font-weight: 620;
  }

  .field-label small {
    display: block;
    margin-top: 3px;
    color: var(--sc-muted);
    font-size: 8.5px;
  }

  .field-controls {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 7px;
  }

  .select-wrap {
    position: relative;
    display: block;
    min-width: 0;
  }

  .select-wrap::after {
    content: "";
    position: absolute;
    top: 50%;
    right: 13px;
    width: 6px;
    height: 6px;
    border-right: 1.5px solid var(--sc-muted);
    border-bottom: 1.5px solid var(--sc-muted);
    pointer-events: none;
    transform: translateY(-68%) rotate(45deg);
  }

  select {
    width: 100%;
    height: 36px;
    padding: 0 36px 0 11px;
    border: 1px solid var(--settings-border);
    border-radius: 10px;
    outline: none;
    appearance: none;
    background: var(--settings-control);
    color: var(--sc-text);
    font: inherit;
    cursor: pointer;
  }

  select:hover {
    border-color: color-mix(in srgb, var(--settings-accent) 32%, var(--settings-border));
  }

  select:focus {
    border-color: var(--settings-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--settings-accent) 12%, transparent);
  }

  select:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }

  @media (max-width: 760px) {
    .field-row {
      grid-template-columns: 1fr;
      gap: 8px;
      padding: 10px 0;
    }
  }
</style>
