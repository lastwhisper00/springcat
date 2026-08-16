import { describe, expect, it } from "vitest";
import { bindPrompt } from "./adapter-prompts";

describe("bindPrompt", () => {
  it("builds complete Codex lifecycle hooks", () => {
    const text = bindPrompt("codex", "E:/bridge/springcat-bridge.exe", "E:/data/inbox");
    expect(text).toContain("E:/bridge/springcat-bridge.exe");
    expect(text).toContain("--source codex");
    expect(text).toContain("task.started");
    expect(text).toContain("task.progress");
    expect(text).toContain("task.completed");
    expect(() => JSON.parse(text)).not.toThrow();
  });

  it("uses native Grok lifecycle events", () => {
    const text = bindPrompt("grok-cli", "E:/bridge/springcat-bridge.exe", "E:/data/inbox");
    const config = JSON.parse(text);
    expect(config.hooks.UserPromptSubmit).toHaveLength(1);
    expect(config.hooks.PostToolUseFailure).toHaveLength(1);
    expect(config.hooks.StopFailure).toHaveLength(1);
  });

  it("uses native Gemini CLI agent hooks with millisecond timeouts", () => {
    const text = bindPrompt("gemini-cli", "E:/bridge/springcat-bridge.exe", "E:/data/inbox");
    const config = JSON.parse(text);
    expect(config.hooks.BeforeAgent).toHaveLength(1);
    expect(config.hooks.AfterTool).toHaveLength(1);
    expect(config.hooks.AfterAgent).toHaveLength(1);
    expect(config.hooks.BeforeAgent[0].hooks[0].timeout).toBe(5000);
    expect(config.hooks.AfterAgent[0].hooks[0].command).toContain(
      "--source gemini-cli --event task.completed",
    );
  });

  it("explains that WorkBuddy uses passive JSONL monitoring", () => {
    const text = bindPrompt("workbuddy", "E:/bridge/springcat-bridge.exe", "E:/data/inbox");
    expect(text).toContain("无需安装 hooks");
    expect(text).toContain(".workbuddy/projects");
  });
});
