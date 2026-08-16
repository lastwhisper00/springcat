<script lang="ts">
  import type { TaskItem } from "$domain";
  import { formatClock, formatDuration, SOURCE_LABEL, STATUS_LABEL } from "$components/work-panel/copy";
  import ToolLogo from "$components/work-panel/ToolLogo.svelte";

  let {
    tasks,
    ontaskopen,
  }: {
    tasks: TaskItem[];
    ontoggle?: (event: MouseEvent) => void;
    ontaskopen?: (task: TaskItem) => void;
  } = $props();

  const visible = $derived(tasks.slice(0, 50));
</script>

<div class="drawer" onpointerdown={(event) => event.stopPropagation()} role="presentation">
  <ul class="list">
    {#each visible as task (task.id)}
      <li class="row" data-status={task.status}>
        <button
          class="hit"
          type="button"
          aria-label={`打开任务：${task.title}`}
          onclick={(event) => {
            event.stopPropagation();
            ontaskopen?.(task);
          }}
        >
          <span class="source-icon" aria-hidden="true">
            <ToolLogo source={task.source} />
          </span>

          <span class="task-copy">
            <span class="title-line">
              <span class="title" title={task.title}>{task.title}</span>
            </span>
            <span class="meta">
              <span>{SOURCE_LABEL[task.source]}</span>
              <i class="separator"></i>
              <span>{formatDuration(task)}</span>
            </span>
          </span>

          <span class="trail">
            <span class="status"><i></i>{STATUS_LABEL[task.status]}</span>
            <time datetime={task.completedAt ?? task.updatedAt}>
              {formatClock(task.completedAt ?? task.updatedAt)}
            </time>
          </span>
        </button>
      </li>
    {:else}
      <li class="empty">暂无任务</li>
    {/each}
  </ul>
</div>

<style>
  .drawer {
    position: relative;
    display: flex;
    min-height: 0;
    flex: 1;
    overflow: hidden;
    padding: 4px 0 8px;
    border-top: 1px solid color-mix(in srgb, var(--sc-text) 7%, transparent);
  }

  .list {
    margin: 0;
    padding: 2px 0 8px;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow: auto;
    min-height: 0;
    flex: 1;
    scrollbar-gutter: stable;
  }

  .list::-webkit-scrollbar {
    width: 4px;
  }

  .list::-webkit-scrollbar-track,
  .list::-webkit-scrollbar-corner {
    background: transparent;
  }

  .list::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: color-mix(in srgb, var(--sc-text) 28%, transparent);
  }

  .list::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--sc-text) 48%, transparent);
  }

  .list::-webkit-scrollbar-button {
    display: none;
    width: 0;
    height: 0;
  }

  .row,
  .empty {
    color: var(--sc-text);
  }

  .row {
    --row-status: var(--sc-muted);
    position: relative;
    flex: 0 0 auto;
    overflow: hidden;
    background: transparent;
    transition: background-color 150ms var(--sc-ease);
  }

  .row:not(:last-child)::after {
    content: "";
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 44px;
    height: 1px;
    background: color-mix(in srgb, var(--sc-text) 6%, transparent);
    pointer-events: none;
  }

  .row[data-status="running"] {
    --row-status: var(--sc-working);
  }

  .row[data-status="waiting"] {
    --row-status: var(--sc-waiting);
  }

  .row[data-status="completed"] {
    --row-status: var(--sc-completed);
  }

  .row[data-status="failed"] {
    --row-status: var(--sc-failed);
  }

  .empty {
    padding: 16px 10px;
    color: var(--sc-muted);
    text-align: center;
  }

  .hit {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    align-items: center;
    gap: 11px;
    width: 100%;
    min-height: 58px;
    padding: 9px 12px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .row:hover,
  .row:focus-within {
    background: color-mix(in srgb, var(--sc-text) 9%, transparent);
  }

  .hit:focus-visible {
    outline: 0;
    box-shadow: inset 2px 0 var(--row-status);
  }

  .source-icon {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 2px;
    color: color-mix(in srgb, var(--sc-text) 78%, var(--row-status));
  }

  .task-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 4px;
  }

  .title-line {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    font-size: 10.5px;
    color: var(--sc-muted);
    white-space: nowrap;
  }

  .separator {
    width: 2px;
    height: 2px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.58;
  }

  .title {
    min-width: 0;
    flex: 0 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 650;
    line-height: 1.25;
  }

  .trail {
    display: flex;
    min-width: 62px;
    align-items: flex-end;
    flex-direction: column;
    gap: 5px;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 2px;
    color: color-mix(in srgb, var(--sc-text) 76%, var(--row-status));
    font-size: 9.5px;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }

  .status i {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--row-status);
  }

  time {
    padding-right: 2px;
    color: var(--sc-muted);
    font-size: 9.5px;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .row {
      transition: none;
    }
  }
</style>
