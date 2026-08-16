<script lang="ts">
  import type { TaskSource } from "$domain";
  import codexLogo from "../../assets/tool-logos/codex.svg";
  import cursorLogo from "../../assets/tool-logos/cursor.svg";
  import geminiCliLogo from "../../assets/tool-logos/gemini-cli.svg";
  import grokLogo from "../../assets/tool-logos/grok.svg";
  import workBuddyLogo from "../../assets/tool-logos/workbuddy.svg";

  const LOGO_URL: Partial<Record<TaskSource, string>> = {
    codex: codexLogo,
    cursor: cursorLogo,
    "gemini-cli": geminiCliLogo,
    "grok-cli": grokLogo,
    workbuddy: workBuddyLogo,
  };

  let { source }: { source: TaskSource | null } = $props();
  const logo = $derived(source ? LOGO_URL[source] : undefined);
</script>

{#if logo && source === "workbuddy"}
  <img class="tool-logo-image" src={logo} alt="" aria-hidden="true" />
{:else if logo}
  <span
    class="tool-logo"
    data-source={source}
    style:--tool-logo={`url("${logo}")`}
  ></span>
{:else}
  <svg viewBox="0 0 24 24" class="idle-mark">
    <path
      d="M12 5.5c.38 3.94 2.56 6.12 6.5 6.5-3.94.38-6.12 2.56-6.5 6.5-.38-3.94-2.56-6.12-6.5-6.5 3.94-.38 6.12-2.56 6.5-6.5Z"
      fill="currentColor"
    />
    <circle cx="12" cy="12" r="1.2" fill="white" />
  </svg>
{/if}

<style>
  .tool-logo {
    width: 100%;
    height: 100%;
    background: currentColor;
    -webkit-mask: var(--tool-logo) center / contain no-repeat;
    mask: var(--tool-logo) center / contain no-repeat;
  }

  .tool-logo[data-source="cursor"] {
    width: 92%;
    height: 92%;
  }

  .tool-logo[data-source="grok-cli"] {
    width: 94%;
    height: 94%;
  }

  .tool-logo[data-source="gemini-cli"] {
    width: 92%;
    height: 92%;
  }

  .tool-logo-image {
    width: 94%;
    height: 94%;
    border-radius: 22%;
    object-fit: contain;
  }

  .idle-mark {
    width: 46%;
    height: 46%;
    opacity: 0.9;
    filter: drop-shadow(0 0 2px rgba(255, 255, 255, 0.55));
  }
</style>
