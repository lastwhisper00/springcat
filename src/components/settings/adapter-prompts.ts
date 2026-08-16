import type { AdapterSource } from "$services/tauri";

export const ADAPTERS: { id: AdapterSource; label: string; detail: string }[] = [
  { id: "codex", label: "Codex", detail: "桌面端与 CLI" },
  { id: "cursor", label: "Cursor", detail: "编辑器与 Agent CLI" },
  { id: "grok-cli", label: "Grok CLI", detail: "终端编码 Agent" },
  { id: "gemini-cli", label: "Gemini CLI", detail: "Google 终端 Agent" },
  { id: "workbuddy", label: "WorkBuddy", detail: "本地会话监听" },
];

export function bindPrompt(source: AdapterSource, bridgePath: string, _inboxDir: string): string {
  if (source === "workbuddy") {
    return "WorkBuddy 无需安装 hooks。SpringCat 会只读监听 ~/.workbuddy/projects 下的会话 JSONL。";
  }
  const bridge = bridgePath.replace(/\\/g, "/");
  const command = (event: "task.started" | "task.progress" | "task.completed") =>
    `"${bridge}" emit --source ${source} --event ${event}`;
  const direct = (event: "task.started" | "task.progress" | "task.completed") => ({
    type: "command",
    command: command(event),
    timeout: 5,
  });
  const nested = (event: "task.started" | "task.progress" | "task.completed") => ({
    hooks: [direct(event)],
  });

  if (source === "gemini-cli") {
    const geminiNested = (event: "task.started" | "task.progress" | "task.completed") => ({
      hooks: [
        {
          ...direct(event),
          name: "SpringCat lifecycle",
          timeout: 5000,
        },
      ],
    });
    return JSON.stringify(
      {
        hooks: {
          BeforeAgent: [geminiNested("task.started")],
          AfterTool: [geminiNested("task.progress")],
          AfterAgent: [geminiNested("task.completed")],
        },
      },
      null,
      2,
    );
  }

  if (source === "cursor") {
    return JSON.stringify(
      {
        version: 1,
        hooks: {
          beforeSubmitPrompt: [direct("task.started")],
          postToolUse: [direct("task.progress")],
          afterAgentResponse: [direct("task.progress")],
          stop: [direct("task.completed")],
        },
      },
      null,
      2,
    );
  }

  if (source === "grok-cli") {
    return JSON.stringify(
      {
        description: "SpringCat lifecycle hooks",
        hooks: {
          UserPromptSubmit: [nested("task.started")],
          PostToolUse: [nested("task.progress")],
          PostToolUseFailure: [nested("task.progress")],
          Stop: [nested("task.completed")],
          StopFailure: [nested("task.completed")],
        },
      },
      null,
      2,
    );
  }

  return JSON.stringify(
    {
      description: "SpringCat lifecycle hooks",
      hooks: {
        UserPromptSubmit: [nested("task.started")],
        PostToolUse: [nested("task.progress")],
        Stop: [nested("task.completed")],
      },
    },
    null,
    2,
  );
}
