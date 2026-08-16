# SpringCat AI

常驻桌面的 AI 任务提醒层。当前地基是 **工作模式**：透明无边框面板，任务模型与事件通道已留好模块边界。宠物模式后置。

Codex CLI、Cursor、Grok CLI、Gemini CLI 与 WorkBuddy 可自动监听生命周期，只采集任务的开始、进度和完成状态；WorkBuddy 仅额外保留当前任务短标题和最多 160 字的完成摘要，不保存完整对话、推理或工具内容。

任务历史、事件收件箱和日志的保存位置可在「设置 → 常规 → 存储」中修改；切换时会复制已有历史，并在重启 SpringCat 后生效。

## 开发

浏览器演示（不挡桌面）：

```bash
pnpm dev
```

桌面工作面板（右上角置顶）。**用托盘图标退出**；左键托盘可隐藏/显示面板：

```bash
pnpm tauri dev
```

## 文档

- [项目描述](./docs/项目描述.md)
- [实现步骤 springcat-ai-v1](./docs/springcat-ai-v1.md)
- [工程记录](./docs/dev-notes.md)
- [Codex / Cursor / Grok CLI / Gemini CLI / WorkBuddy 适配器安装](./docs/adapters.md)

## 目录要点

- `src/domain` / `src-tauri/src/domain`：唯一任务真相
- `src/components/work-panel`：工作面板展示层
- `src/components/pet`：S10 前不实现
- `src-tauri/src/adapters`：各 AI 工具适配器（S07）
- `bridge/`：本地 inbox 桥（S06）
