<script lang="ts">
  let {
    checked,
    disabled = false,
    ariaLabel,
    onchange,
  }: {
    checked: boolean;
    disabled?: boolean;
    ariaLabel?: string;
    onchange?: (checked: boolean) => void;
  } = $props();
</script>

<input
  class="switch"
  type="checkbox"
  {checked}
  {disabled}
  aria-label={ariaLabel}
  onchange={(event) => onchange?.(event.currentTarget.checked)}
/>

<style>
  .switch {
    position: relative;
    width: 34px;
    height: 20px;
    margin: 0;
    border: 0;
    border-radius: 99px;
    outline: none;
    appearance: none;
    background: color-mix(in srgb, var(--sc-muted) 28%, transparent);
    cursor: pointer;
    transition: background 140ms ease;
  }

  .switch::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--settings-accent-text);
    box-shadow: 0 1px 4px rgb(0 0 0 / 22%);
    transition: transform 160ms var(--sc-ease);
  }

  .switch:checked {
    background: var(--settings-accent);
  }

  .switch:checked::after {
    transform: translateX(14px);
  }

  .switch:focus-visible {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--settings-accent) 18%, transparent);
  }

  .switch:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
</style>
