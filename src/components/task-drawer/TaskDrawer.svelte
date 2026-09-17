<script lang="ts">
  import { untrack } from "svelte";
  import type { TaskItem } from "$domain";
  import { formatClock, SOURCE_LABEL } from "$components/work-panel/copy";
  import ToolLogo from "$components/work-panel/ToolLogo.svelte";
  import { reconcileTaskOrder, sortTaskIds } from "./task-order";

  let {
    tasks,
    ontaskopen,
    onmarkallread,
    onhoverchange,
  }: {
    tasks: TaskItem[];
    ontaskopen?: (task: TaskItem) => void;
    onmarkallread?: () => void;
    onhoverchange?: (hovered: boolean) => void;
  } = $props();

  const statusLabel: Record<TaskItem["status"], string> = {
    running: "进行中",
    waiting: "待确认",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
  };

  let readingOrder = $state<string[] | null>(null);
  const taskById = $derived(new Map(tasks.map((task) => [task.id, task])));
  const orderedIds = $derived(readingOrder ?? sortTaskIds(tasks));
  const orderedTasks = $derived(
    orderedIds.flatMap((id) => {
      const task = taskById.get(id);
      return task ? [task] : [];
    }),
  );
  const unreadCount = $derived(tasks.filter((task) => task.unread).length);

  $effect(() => {
    const currentTasks = [...tasks];
    untrack(() => {
      readingOrder = reconcileTaskOrder(readingOrder, currentTasks);
    });
  });

</script>

<div
  class="drawer"
  onpointerdown={(event) => event.stopPropagation()}
  onclick={(event) => event.stopPropagation()}
  ondblclick={(event) => event.stopPropagation()}
  onkeydown={(event) => {
    if (event.key !== "Escape") event.stopPropagation();
  }}
  onpointerenter={() => onhoverchange?.(true)}
  onpointerleave={() => onhoverchange?.(false)}
  role="presentation"
>
  <header class="drawer-header">
    <h2>最近任务 <span class="total">{tasks.length}</span></h2>
    <div class="header-actions">
      {#if onmarkallread}
        <button
          class="text-control"
          type="button"
          disabled={unreadCount === 0}
          onclick={() => onmarkallread?.()}
        >{unreadCount === 0 ? "全部已读" : "标记已读"}</button>
      {/if}
    </div>
  </header>

  <ul class="list" aria-label="最近任务">
    {#each orderedTasks as task (task.id)}
      <li class="row" data-status={task.status}>
        <button
          class="hit"
          type="button"
          aria-label={`打开任务：${task.title}，${statusLabel[task.status]}`}
          title={task.title}
          onclick={() => ontaskopen?.(task)}
        >
          <span class="source-icon" aria-hidden="true"><ToolLogo source={task.source} /></span>
          <span class="task-copy">
            <span class="title">{task.title}</span>
            <span class="meta">
              <span>{SOURCE_LABEL[task.source]}</span>
              <span aria-hidden="true">·</span>
              <time datetime={task.updatedAt}>{formatClock(task.updatedAt)}</time>
            </span>
          </span>
          <span class="status"><i aria-hidden="true"></i>{statusLabel[task.status]}</span>
        </button>
      </li>
    {:else}
      <li class="empty">暂无任务</li>
    {/each}
  </ul>
</div>

<style>
  .drawer {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 0 0 8px;
    color: var(--sc-text);
    cursor: default;
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 40px;
    flex: 0 0 40px;
    padding: 0 24px;
  }

  h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    color: var(--sc-muted);
    font-size: 13px;
    font-weight: 400;
    line-height: 20px;
    white-space: nowrap;
  }

  .total,
  time { font-variant-numeric: tabular-nums; }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .text-control {
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--sc-muted);
    font: inherit;
    font-size: 13px;
    line-height: 20px;
    cursor: pointer;
  }

  .text-control {
    min-height: 28px;
    padding: 3px 4px;
    white-space: nowrap;
  }

  .text-control:hover:not(:disabled) { background: var(--sc-row-hover); }
  .text-control:active:not(:disabled) { background: var(--sc-row-pressed); }
  button:disabled { cursor: default; opacity: 0.5; }

  .list {
    --sc-row-hover: rgba(255, 255, 255, 0.14);
    --sc-row-pressed: rgba(255, 255, 255, 0.18);
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior-y: contain;
    scrollbar-gutter: stable;
  }

  .list::-webkit-scrollbar { width: 6px; }
  .list::-webkit-scrollbar-track,
  .list::-webkit-scrollbar-corner { background: transparent; }
  .list::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: color-mix(in srgb, var(--sc-text) 25%, transparent);
  }
  .list::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--sc-text) 45%, transparent);
  }
  .list::-webkit-scrollbar-button { display: none; width: 0; height: 0; }

  .row { --row-status: var(--sc-muted); height: 64px; }
  .row[data-status="running"] { --row-status: var(--sc-working); }
  .row[data-status="waiting"] { --row-status: var(--sc-waiting); }
  .row[data-status="completed"] { --row-status: var(--sc-completed); }
  .row[data-status="failed"] { --row-status: var(--sc-failed); }

  .hit {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    align-items: center;
    gap: 13px;
    width: 100%;
    height: 64px;
    padding: 10px 24px;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--sc-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color 140ms ease;
  }

  .hit:hover,
  .hit:focus-visible { background: var(--sc-row-hover); }
  .hit:active { background: var(--sc-row-pressed); }
  button:focus-visible { outline: 2px solid var(--sc-working); outline-offset: -2px; }

  .source-icon {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    color: var(--sc-muted);
  }

  .task-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 14px;
    font-weight: 500;
    line-height: 20px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    color: #969696;
    font-size: 11px;
    font-weight: 400;
    line-height: 16px;
    white-space: nowrap;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--sc-muted);
    font-size: 12px;
    font-weight: 400;
    line-height: 20px;
    white-space: nowrap;
  }

  .status i {
    width: 4px;
    height: 4px;
    flex: 0 0 4px;
    border-radius: 50%;
    background: var(--row-status);
  }

  .empty {
    padding: 36px 10px;
    color: var(--sc-muted);
    text-align: center;
    font-size: 13px;
    line-height: 20px;
  }

  @media (prefers-reduced-motion: reduce) { .hit { transition: none; } }
</style>
