<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import DemoPlayground from "./demo/DemoPlayground.svelte";
  import OverlayShell from "./OverlayShell.svelte";
  import SettingsPage from "$components/settings/SettingsPage.svelte";

  const isSettings =
    typeof window !== "undefined" &&
    (window.location.hash === "#settings" || new URLSearchParams(window.location.search).get("window") === "settings");
</script>

{#if isSettings}
  <SettingsPage />
{:else if isTauri()}
  <OverlayShell />
{:else}
  <DemoPlayground />
{/if}
