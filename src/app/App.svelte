<script lang="ts">
  import DemoPlayground from "./demo/DemoPlayground.svelte";
  import OverlayShell from "./OverlayShell.svelte";
  import PanelContextMenu from "$components/panel-menu/PanelContextMenu.svelte";
  import SettingsPage from "$components/settings/SettingsPage.svelte";
  import { resolveWindowKind } from "./window-kind";

  const kind = resolveWindowKind();
</script>

{#if kind === "panel-menu"}
  <PanelContextMenu />
{:else if kind === "settings"}
  <SettingsPage />
{:else if kind === "overlay"}
  <OverlayShell />
{:else}
  <DemoPlayground />
{/if}
