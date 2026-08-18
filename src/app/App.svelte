<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import DemoPlayground from "./demo/DemoPlayground.svelte";
  import OverlayShell from "./OverlayShell.svelte";
  import PanelContextMenu from "$components/panel-menu/PanelContextMenu.svelte";
  import SettingsPage from "$components/settings/SettingsPage.svelte";

  const isPanelMenu =
    typeof window !== "undefined" && window.location.hash === "#panel-menu";
  const isSettings =
    typeof window !== "undefined" &&
    (window.location.hash === "#settings" || new URLSearchParams(window.location.search).get("window") === "settings");
</script>

{#if isPanelMenu}
  <PanelContextMenu />
{:else if isSettings}
  <SettingsPage />
{:else if isTauri()}
  <OverlayShell />
{:else}
  <DemoPlayground />
{/if}
