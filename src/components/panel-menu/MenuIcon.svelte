<script lang="ts">
  let { name }: { name: "tasks" | "mute" | "focus" | "island" | "pin" | "pet" | "settings" | "quit" } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
  {#if name === "tasks"}
    <path d="M8 6h12M8 12h12M8 18h12" />
    <path d="m3.5 6 1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2" />
  {:else if name === "mute"}
    <path d="M11 5 7 9H4v6h3l4 4V5Z" />
    <path d="m16 9 5 5m0-5-5 5" />
  {:else if name === "focus"}
    <path d="M8 3H4a1 1 0 0 0-1 1v4m13-5h4a1 1 0 0 1 1 1v4M8 21H4a1 1 0 0 1-1-1v-4m13 5h4a1 1 0 0 0 1-1v-4" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === "island"}
    <rect x="3" y="8" width="18" height="8" rx="4" />
    <path d="M8 12h8" />
  {:else if name === "pin"}
    <path d="m9 4 6 0-.7 5 3 3v1H6.7v-1l3-3L9 4Z" />
    <path d="M12 13v8" />
  {:else if name === "pet"}
    <path d="M7 10 5 5l5 2h4l5-2-2 5v5c0 3-2 5-5 5s-5-2-5-5v-5Z" />
    <path d="M9.5 13h.01m4.99 0h.01M10 17c1.2.7 2.8.7 4 0" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path d="M19 13.5v-3l-2-.7-.7-1.7.9-1.9-2.1-2.1-1.9.9-1.7-.7L10.8 2h-3l-.7 2.3-1.7.7-1.9-.9-2.1 2.1.9 1.9-.7 1.7-2 .7v3l2 .7.7 1.7-.9 1.9 2.1 2.1 1.9-.9 1.7.7.7 2.3h3l.7-2.3 1.7-.7 1.9.9 2.1-2.1-.9-1.9.7-1.7 2-.7Z" transform="translate(1.6) scale(.87)" />
  {:else}
    <path d="M10 4H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h5M14 8l4 4-4 4m4-4H8" />
  {/if}
</svg>

<style>
  svg {
    width: 17px;
    height: 17px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
