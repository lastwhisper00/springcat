import type { TaskItem } from "$domain";

export function sortTaskIds(tasks: TaskItem[]): string[] {
  return [...tasks]
    .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
    .map((task) => task.id);
}

/** Preserve the rows being read and append new tasks without moving the viewport. */
export function reconcileTaskOrder(order: string[] | null, tasks: TaskItem[]): string[] {
  if (order === null) return sortTaskIds(tasks);
  const available = new Set(tasks.map((task) => task.id));
  const retained = order.filter((id) => available.has(id));
  const retainedIds = new Set(retained);
  const additions = sortTaskIds(tasks.filter((task) => !retainedIds.has(task.id)));
  return [...retained, ...additions];
}
