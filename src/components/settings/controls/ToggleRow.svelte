<script lang="ts">
  import Toggle from "./Toggle.svelte";

  let {
    label,
    hint,
    checked,
    disabled = false,
    onchange,
  }: {
    label: string;
    hint?: string;
    checked: boolean;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  } = $props();
</script>

<label class="field-row switch-row">
  <span class="field-label">
    <strong>{label}</strong>
    {#if hint}<small>{hint}</small>{/if}
  </span>
  <Toggle {checked} {disabled} {onchange} />
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

  .switch-row {
    grid-template-columns: minmax(180px, 1fr) auto;
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

  @media (max-width: 760px) {
    .field-row {
      grid-template-columns: 1fr;
      gap: 8px;
      padding: 10px 0;
    }

    .switch-row {
      grid-template-columns: 1fr auto;
    }
  }
</style>
