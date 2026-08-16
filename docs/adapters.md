# 适配器安装

SpringCat 不轮询 Codex、Cursor、Grok CLI、Gemini CLI 或 WorkBuddy 的窗口，也不开本地 HTTP 服务。Codex 桌面端与 WorkBuddy 由 SpringCat 直接读取本机追加写入的结构化生命周期记录；Codex、Cursor、Grok 和 Gemini 的 hooks 则通过本地 `springcat-bridge` 发送同样的三类信号：

- 开始：一次新提交开始运行
- 进度：工具调用或 Agent 响应有更新
- 完成：本轮停止

两条链路都只保留会话 ID、状态、工作区和短标题等生命周期元数据。Cursor 标题通过会话 ID 精确读取本机状态库中的生成名称；名称尚未生成时，只截取首条 prompt 的第一行（最多 80 字符）作为临时标题。WorkBuddy 从其 `<user_query>` 提取最多 80 字的任务标题，并在完成时保留最多 160 字的最终回复摘要。完整 prompt、推理、工具参数/结果和 transcript 路径不会进入 SpringCat 的 inbox 或 SQLite。

## 一键安装（推荐）

1. 打开 SpringCat → 设置 → 适配器。
2. 选择 Codex 桌面端 / CLI、Cursor、Grok CLI、Gemini CLI 或 WorkBuddy。
3. 点击「启用监听」。

SpringCat 会自动完成以下操作：

- 把随应用发布的 `springcat-bridge` 安装到应用数据目录；
- 合并写入用户级 hooks，不覆盖其他工具已有的 hook；
- 首次修改已有配置时保留 `hooks.json.springcat.bak`；
- 自动启用对应的 SpringCat 来源开关。

点击「移除监听」只删除 SpringCat 自己的命令，其他 hook 会保留。

WorkBuddy 不需要安装或修改 hooks。设置页检测到 `~/.workbuddy/projects` 后即可启用直接监听；关闭来源开关只会暂停 SpringCat 接收事件，不会改动 WorkBuddy 会话文件。

此外，SpringCat 启动时会检查所有已启用来源；发现 bridge 或 hooks 缺失时会自动修复。设置页的来源复选框也会同时完成“安装监听 + 启用来源”，避免只打开接收开关却没有实际绑定。

### Codex 桌面端 / CLI

配置文件：`~/.codex/hooks.json`

| Codex hook | SpringCat 状态 |
|---|---|
| `UserPromptSubmit` | 开始 |
| `PostToolUse` | 进度 |
| `Stop` | 完成 |

Codex 采用双通道监听：

- SpringCat 直接监听本机 Codex 追加写入的会话文件，只解析 `task_started`、工具完成、`task_complete` / `turn_aborted` 等结构化生命周期字段，并从 Codex 本机状态库读取会话默认标题。Codex 已经在运行、尚未重新加载 hooks 时，这条通道仍然有效。
- 官方 hooks 继续作为额外实时通道。Codex 会审查用户级 command hooks，首次加载或配置变化时可能要求在 `/hooks` 中确认信任；SpringCat 不会绕过这一安全边界，也不会把它作为桌面端监听的前置条件。

监听器启动时会回放最近 24 小时内每个会话的最新一轮状态，因此 SpringCat 晚于 Codex 启动时，也能恢复正在运行或刚刚完成的任务。它不会把 prompt、assistant 回复、工具参数或工具结果写入 SpringCat。

### Cursor

配置文件：`~/.cursor/hooks.json`

| Cursor hook | SpringCat 状态 |
|---|---|
| `beforeSubmitPrompt` | 开始 |
| `postToolUse` | 进度 |
| `afterAgentResponse` | 进度 |
| `stop` | 完成 |

Cursor 会自动重新加载用户级 `hooks.json`，一般不需要重启或进入 Cursor 设置。

SpringCat 启动时还会按 Cursor 会话 ID 回填历史记录中的“未命名任务”，并监听 Cursor 本地状态库纠正缺失 `stop` hook 的完成/中止状态。纠偏只读取会话状态、状态更新时间和生成 ID，不读取对话、工具参数或结果。Cursor 状态库不可用时不会影响开始、进度和完成事件，标题会退回首条 prompt 的短标题。

### Grok CLI

配置文件：`~/.grok/hooks/springcat.json`

| Grok hook | SpringCat 状态 |
|---|---|
| `UserPromptSubmit` | 开始 |
| `PostToolUse` / `PostToolUseFailure` | 进度 |
| `Stop` / `StopFailure` | 完成 |

Grok 的全局 hooks 始终受信任，不需要项目级 `/hooks-trust`。Grok 会在新会话启动时读取配置，因此绑定后请新建或重启会话。bridge 兼容 Grok 原生的 `hookEventName`、`sessionId`、`workspaceRoot` camelCase 协议，并会忽略会话关闭时额外产生的 Stop。

### Gemini CLI

配置文件：`~/.gemini/settings.json`

| Gemini CLI hook | SpringCat 状态 |
|---|---|
| `BeforeAgent` | 开始 |
| `AfterTool` | 进度 |
| `AfterAgent` | 完成 |

Gemini CLI 使用官方全局 hooks，每次用户提交、工具执行完成和 Agent 最终响应都会触发对应状态。SpringCat 只合并 `hooks` 字段，保留现有认证、安全和其他 hook 配置；首次修改已有 `settings.json` 时会创建 `settings.json.springcat.bak`。Gemini 的 hook 超时单位是毫秒，因此安装项使用 `5000`，而不是其他适配器使用的秒数。

bridge 会从 `BeforeAgent` 的 `prompt` 派生最多 80 字的短标题，但不会保存原始 prompt；`AfterAgent` 的 `prompt_response`、工具参数/结果和 `transcript_path` 同样会在写入 inbox 前删除。绑定后请新建或重启一次 Gemini CLI 会话。

### WorkBuddy

会话目录：`~/.workbuddy/projects/<workspace>/<conversation-id>.jsonl`

WorkBuddy 适配器使用只读递增游标监听 JSONL：

| WorkBuddy 记录 | SpringCat 状态 |
|---|---|
| 带真实 `<user_query>` 的 user message | 开始 |
| `reasoning` / `function_call` / `function_call_result` | 进度 |
| 文件末尾稳定停留在 completed assistant message 1.5 秒 | 完成 |
| assistant message 的 error / cancelled 状态 | 失败 / 取消 |

WorkBuddy 会把工具调用前的中间说明也标记为 `completed`。SpringCat 不会据此立刻提醒，而是等待短暂静默；如果后续紧跟工具调用，就继续视为运行中。启动时只恢复最近 24 小时内仍停留在推理或工具阶段的任务，不把历史完成对话重新标成未读。

## 数据链路

```text
Codex 本机会话文件 ───────────────────────┐
WorkBuddy 本地会话 JSONL ─────────────────┤
Codex/Cursor/Grok/Gemini hook             │
  → springcat-bridge（先删除对话和工具内容）│
  → %APPDATA%\springcat-ai\inbox\         │
  └───────────────────────────────────────┤
                                          → Rust 规范化/去重
                                          → SQLite 生命周期状态
                                          → Svelte 面板
```

某个适配器解析失败只记录日志并隔离到 `inbox-failed/`，不会影响另外的适配器。设置页开关关闭后，该来源的新事件会直接丢弃。

## 开发环境

Tauri 的开发和打包命令会先自动构建 bridge。也可以单独构建：

```text
pnpm bridge:build
```

手动发送一条测试生命周期事件：

```powershell
'{"session_id":"test-session"}' | bridge\target\release\springcat-bridge.exe emit --source codex --event task.started
```

若自动写入因权限或损坏的 JSON 失败，设置页会显示目标路径与完整配置。用户可以手动合并，也可以把配置和路径直接交给 Cursor/Grok，让工具主动完成绑定。
