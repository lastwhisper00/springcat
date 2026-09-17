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

export type DemoKind = SurfaceState["kind"] | "completed-many" | "working-many" | "mixed" | "many";

export function tasksForKind(kind: DemoKind): TaskItem[] {
  switch (kind) {
    case "mixed":
      return DEMO_TASK_LIST;
    case "many":
      return [...DEMO_TASK_LIST,
        task({ id: "copy-review", source: "gemini-cli", status: "waiting", title: "校对界面文案", updatedAt: at(3) }),
        task({ id: "build-check", source: "codex", status: "running", title: "验证构建流程", updatedAt: at(2) }),
        task({ id: "research-notes", source: "cursor", status: "completed", title: "汇总调研记录", updatedAt: at(1) }),
      ];
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
  task({ id: "settings-interaction", source: "codex", status: "waiting", title: "重构设置页交互", updatedAt: at(9) }),
  task({ id: "interview-review", source: "cursor", status: "running", title: "整理用户访谈", startedAt: at(2), updatedAt: at(8), unread: false }),
  task({ id: "design-handoff", source: "gemini-cli", status: "waiting", title: "设计交接文档", updatedAt: at(7) }),
  task({ id: "api-review", source: "codex", status: "waiting", title: "检查接口变更", updatedAt: at(6) }),
  task({ id: "interaction-tests", source: "cursor", status: "completed", title: "补充交互测试", updatedAt: at(5) }),
];
