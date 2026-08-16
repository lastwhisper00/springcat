import type { SurfaceState } from "$domain";
import { deriveSurfaceState } from "$domain";
import type { TaskItem } from "$domain";

let items = $state<TaskItem[]>([]);

export const taskStore = {
  get items(): TaskItem[] {
    return items;
  },
  set items(next: TaskItem[]) {
    items = next;
  },
  get surface(): SurfaceState {
    return deriveSurfaceState(items);
  },
};
