import type { SurfaceState, TaskItem } from "$domain";

function at(offsetMinutes: number): string {
  return new Date(Date.parse("2026-08-13T06:00:00.000Z") + offsetMinutes * 60_000).toISOString();
}

function task(partial: Partial<TaskItem> & Pick<TaskItem, "id" | "status" | "title">): TaskItem {
  return {
    source: "codex",
    updatedAt: at(1),
    unread: partial.status === "completed" || partial.status === "failed" || partial.status === "waiting",
    ...partial,
  };
}

export type DemoKind = SurfaceState["kind"] | "completed-many" | "working-many";

export function tasksForKind(kind: DemoKind): TaskItem[] {
  switch (kind) {
    case "idle":
      return [];
    case "working":
      return [
        task({
          id: "login-tests",
          status: "running",
          title: "修复登录页测试",
          source: "codex",
          startedAt: at(0),
          updatedAt: at(4),
        }),
      ];
    case "waiting":
      return [
        task({
          id: "login-tests",
          status: "waiting",
          title: "修复登录页测试",
          summary: "需要确认后才能继续改测试",
          source: "codex",
          startedAt: at(0),
          updatedAt: at(6),
          action: { label: "去处理" },
        }),
      ];
    case "failed":
      return [
        task({
          id: "login-tests",
          status: "failed",
          title: "修复登录页测试",
          summary: "登录页断言失败：expected 200, got 500",
          source: "codex",
          startedAt: at(0),
          completedAt: at(8),
          updatedAt: at(8),
          action: { label: "查看原因" },
        }),
      ];
    case "completed":
      return [
        task({
          id: "login-tests",
          status: "completed",
          title: "修复登录页测试",
          summary: "已补齐登录页失败用例",
          source: "codex",
          startedAt: at(0),
          completedAt: at(9),
          updatedAt: at(9),
          unread: true,
          action: { label: "查看结果" },
        }),
      ];
    case "working-many":
      return [
        task({
          id: "login-tests",
          status: "running",
          title: "修复登录页测试",
          source: "codex",
          startedAt: at(0),
          updatedAt: at(4),
        }),
        task({
          id: "review",
          status: "running",
          title: "审查鉴权中间件",
          source: "cursor",
          startedAt: at(2),
          updatedAt: at(5),
          unread: false,
        }),
      ];
    case "completed-many":
      return [
        task({
          id: "t1",
          status: "completed",
          title: "修复登录页测试",
          source: "codex",
          unread: true,
          updatedAt: at(9),
        }),
        task({
          id: "t2",
          status: "completed",
          title: "整理 hooks 文档",
          source: "cursor",
          unread: true,
          updatedAt: at(10),
        }),
        task({
          id: "t3",
          status: "completed",
          title: "补齐 CLI 适配",
          source: "grok-cli",
          unread: true,
          updatedAt: at(11),
        }),
      ];
  }
}

export const DEMO_TASK_LIST: TaskItem[] = [
  ...tasksForKind("waiting"),
  task({
    id: "review",
    status: "running",
    title: "审查鉴权中间件",
    source: "cursor",
    startedAt: at(2),
    updatedAt: at(7),
    unread: false,
  }),
  ...tasksForKind("completed"),
];
