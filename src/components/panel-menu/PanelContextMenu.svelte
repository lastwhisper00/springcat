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
    <div class="group">
      <button class="item primary" role="menuitem" type="button" onclick={() => void run("view-tasks")}>
        <span class="icon"><MenuIcon name="tasks" /></span>
        <span class="label">查看所有任务</span>
        <span class="chevron" aria-hidden="true"></span>
      </button>
      <button class="item" role="menuitem" type="button" onclick={() => void run("mark-all-read")}>
        <span class="icon"><MenuIcon name="read" /></span>
        <span class="label">全部标为已读</span>
      </button>
    </div>

    <div class="group">
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
    </div>

    <div class="group">
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
</div>

<style>
  .window-pad {
    width: 100%;
    height: 100%;
    padding: 11px;
  }

  .menu {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 100%;
    height: auto;
    padding: 7px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, white 18%, var(--sc-border));
    border-radius: 18px;
    background: color-mix(in srgb, Canvas 94%, transparent);
    box-shadow:
      0 18px 44px rgb(0 0 0 / 32%),
      0 2px 8px rgb(0 0 0 / 12%),
      inset 1px 1px 0 rgb(255 255 255 / 38%),
      inset -1px -1px 0 rgb(255 255 255 / 10%);
    backdrop-filter: blur(22px) saturate(1.25);
    animation: menu-enter 160ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .group + .group {
    padding-top: 5px;
    border-top: 1px solid color-mix(in srgb, CanvasText 8%, transparent);
  }

  .item {
    appearance: none;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 0 8px 0 6px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: color-mix(in srgb, CanvasText 88%, transparent);
    font: 500 13px/1.2 "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
    letter-spacing: 0.01em;
    text-align: left;
    cursor: pointer;
    transition:
      color 120ms ease,
      background-color 120ms ease;
  }

  .item:hover,
  .item:focus-visible {
    outline: none;
    background: color-mix(in srgb, CanvasText 8%, transparent);
    color: CanvasText;
  }

  .item:active:not(:disabled) {
    background: color-mix(in srgb, CanvasText 12%, transparent);
  }

  .item.primary {
    font-weight: 600;
  }

  .item.danger {
    color: color-mix(in srgb, #e95750 78%, CanvasText);
  }

  .item.danger:hover,
  .item.danger:focus-visible {
    background: color-mix(in srgb, #ef655d 12%, transparent);
    color: #ef655d;
  }

  .item:disabled {
    color: color-mix(in srgb, CanvasText 38%, transparent);
    cursor: default;
  }

  .item:disabled:hover,
  .item:disabled:focus-visible {
    background: transparent;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    color: color-mix(in srgb, CanvasText 62%, transparent);
  }

  .primary .icon {
    color: color-mix(in srgb, var(--sc-accent) 70%, CanvasText);
  }

  .danger .icon {
    color: inherit;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    width: 6px;
    height: 6px;
    margin-right: 2px;
    border-right: 1.5px solid currentColor;
    border-top: 1.5px solid currentColor;
    opacity: 0.38;
    transform: rotate(45deg);
  }

  .switch {
    position: relative;
    width: 32px;
    height: 18px;
    border-radius: 999px;
    background: color-mix(in srgb, CanvasText 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, CanvasText 6%, transparent);
    transition: background-color 160ms ease;
  }

  .switch i {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 22%);
    transition: transform 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .switch.on {
    background: color-mix(in srgb, var(--sc-accent) 82%, CanvasText);
    box-shadow: none;
  }

  .switch.on i {
    transform: translateX(14px);
  }

  .soon {
    color: color-mix(in srgb, CanvasText 46%, transparent);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.02em;
  }

  @keyframes menu-enter {
    from {
      opacity: 0;
      transform: translateY(-5px) scale(0.96);
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
