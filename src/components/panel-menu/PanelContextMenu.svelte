<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isMuted } from "$domain";
  import { getSettings, runPanelMenuAction, type PanelMenuAction } from "$services/tauri";
  import MenuIcon from "./MenuIcon.svelte";

  let muted = $state(false);
  let focusMode = $state(false);
  let dynamicIsland = $state(false);
  let pinned = $state(false);
  let busy = $state(false);
  let menuRoot: HTMLElement;

  async function refresh() {
    if (!isTauri()) return;
    const settings = await getSettings();
    muted = isMuted(settings);
    focusMode = settings.focusMode;
    dynamicIsland = settings.dynamicIslandCompatible;
    pinned = settings.alwaysOnTop;
  }

  function focusFirst() {
    requestAnimationFrame(() => {
      menuRoot?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    });
  }

  async function closeMenu() {
    if (isTauri()) await getCurrentWindow().hide();
  }

  async function run(action: PanelMenuAction) {
    if (busy || !isTauri()) return;
    busy = true;
    try {
      await runPanelMenuAction(action);
    } catch (error) {
      busy = false;
      console.error("Unable to run panel menu action", error);
    }
  }

  function onMenuKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      void closeMenu();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const items = Array.from(
      menuRoot.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
    );
    if (!items.length) return;
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : event.key === "ArrowUp"
            ? (current - 1 + items.length) % items.length
            : (current + 1) % items.length;
    items[next]?.focus();
  }

  onMount(() => {
    document.documentElement.classList.add("panel-menu");
    focusFirst();
    if (!isTauri()) return;

    let readyToBlur = false;
    const readyTimer = window.setTimeout(() => {
      readyToBlur = true;
    }, 120);
    const unlistenFocus = getCurrentWindow().onFocusChanged((event) => {
      if (readyToBlur && !event.payload) void closeMenu();
    });
    const unlistenSettings = listen("settings-changed", () => void refresh());
    const unlistenOpened = listen("panel-menu-opened", () => {
      busy = false;
      void refresh();
      focusFirst();
    });
    void refresh();

    return () => {
      window.clearTimeout(readyTimer);
      document.documentElement.classList.remove("panel-menu");
      void unlistenFocus.then((fn) => fn());
      void unlistenSettings.then((fn) => fn());
      void unlistenOpened.then((fn) => fn());
    };
  });
</script>

<div class="window-pad">
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    aria-label="SpringCat 快捷菜单"
    bind:this={menuRoot}
    onkeydown={onMenuKeydown}
    oncontextmenu={(event) => event.preventDefault()}
  >
    <button class="item primary" role="menuitem" type="button" onclick={() => void run("view-tasks")}>
      <span class="icon"><MenuIcon name="tasks" /></span>
      <span class="label">查看所有任务</span>
      <span class="arrow" aria-hidden="true">›</span>
    </button>

    <div class="divider" role="separator"></div>

    <button class="item" role="menuitemcheckbox" type="button" aria-checked={muted} onclick={() => void run("mute")}>
      <span class="icon"><MenuIcon name="mute" /></span>
      <span class="label">静音 1 小时</span>
      <span class="switch" class:on={muted} aria-hidden="true"><i></i></span>
    </button>
    <button class="item" role="menuitemcheckbox" type="button" aria-checked={focusMode} onclick={() => void run("focus")}>
      <span class="icon"><MenuIcon name="focus" /></span>
      <span class="label">专注模式</span>
      <span class="switch" class:on={focusMode} aria-hidden="true"><i></i></span>
    </button>
    <button class="item" role="menuitemcheckbox" type="button" aria-checked={dynamicIsland} onclick={() => void run("dynamic-island")}>
      <span class="icon"><MenuIcon name="island" /></span>
      <span class="label">兼容灵动岛</span>
      <span class="switch" class:on={dynamicIsland} aria-hidden="true"><i></i></span>
    </button>
    <button class="item" role="menuitemcheckbox" type="button" aria-checked={pinned} onclick={() => void run("pin")}>
      <span class="icon"><MenuIcon name="pin" /></span>
      <span class="label">置顶</span>
      <span class="switch" class:on={pinned} aria-hidden="true"><i></i></span>
    </button>
    <button class="item" role="menuitem" type="button" disabled>
      <span class="icon"><MenuIcon name="pet" /></span>
      <span class="label">切换宠物模式</span>
      <span class="soon">即将推出</span>
    </button>

    <div class="divider" role="separator"></div>

    <button class="item" role="menuitem" type="button" onclick={() => void run("settings")}>
      <span class="icon"><MenuIcon name="settings" /></span>
      <span class="label">设置</span>
    </button>
    <button class="item danger" role="menuitem" type="button" onclick={() => void run("quit")}>
      <span class="icon"><MenuIcon name="quit" /></span>
      <span class="label">退出</span>
    </button>
  </div>
</div>

<style>
  .window-pad {
    width: 100%;
    height: 100%;
    padding: 9px;
  }

  .menu {
    width: 100%;
    height: 100%;
    padding: 7px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, CanvasText 14%, transparent);
    border-radius: 16px;
    background: color-mix(in srgb, Canvas 94%, transparent);
    box-shadow:
      0 18px 50px rgb(0 0 0 / 28%),
      0 3px 12px rgb(0 0 0 / 14%),
      inset 0 1px rgb(255 255 255 / 8%);
    backdrop-filter: blur(24px) saturate(1.35);
    animation: menu-enter 150ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .item {
    appearance: none;
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 38px;
    padding: 0 9px 0 7px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: color-mix(in srgb, CanvasText 90%, transparent);
    font: 500 13px/1 "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
    text-align: left;
    cursor: pointer;
    transition:
      color 120ms ease,
      background-color 120ms ease,
      transform 120ms ease;
  }

  .item:hover,
  .item:focus-visible {
    outline: none;
    background: color-mix(in srgb, CanvasText 9%, transparent);
    color: CanvasText;
  }

  .item:active:not(:disabled) {
    transform: scale(0.985);
  }

  .item.primary {
    font-weight: 650;
  }

  .item.danger:hover,
  .item.danger:focus-visible {
    background: color-mix(in srgb, #ef655d 13%, transparent);
    color: #e95750;
  }

  .item:disabled {
    color: color-mix(in srgb, CanvasText 32%, transparent);
    cursor: default;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 27px;
    height: 27px;
    border-radius: 8px;
    background: color-mix(in srgb, CanvasText 7%, transparent);
    color: color-mix(in srgb, CanvasText 72%, transparent);
  }

  .primary .icon {
    background: color-mix(in srgb, var(--sc-accent) 16%, transparent);
    color: color-mix(in srgb, var(--sc-accent) 82%, CanvasText);
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .divider {
    height: 1px;
    margin: 6px 9px;
    background: color-mix(in srgb, CanvasText 11%, transparent);
  }

  .arrow {
    color: color-mix(in srgb, CanvasText 42%, transparent);
    font-size: 21px;
    font-weight: 300;
    transform: translateY(-1px);
  }

  .switch {
    position: relative;
    width: 28px;
    height: 16px;
    border-radius: 999px;
    background: color-mix(in srgb, CanvasText 16%, transparent);
    transition: background-color 140ms ease;
  }

  .switch i {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: color-mix(in srgb, Canvas 95%, CanvasText);
    box-shadow: 0 1px 3px rgb(0 0 0 / 24%);
    transition: transform 160ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .switch.on {
    background: color-mix(in srgb, var(--sc-accent) 78%, CanvasText);
  }

  .switch.on i {
    transform: translateX(12px);
  }

  .soon {
    padding: 4px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, CanvasText 6%, transparent);
    font-size: 9px;
    font-weight: 600;
  }

  @keyframes menu-enter {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }
  }
</style>
