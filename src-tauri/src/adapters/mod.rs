//! Tool adapters convert vendor-specific hook payloads into TaskEvent.
//! One adapter failing must not take the others down.

use std::panic::{catch_unwind, AssertUnwindSafe};

use serde_json::Value;

use crate::domain::{TaskEvent, TaskEventType, TaskSource};
use crate::settings_store::now_rfc3339;

pub mod codex;
pub mod cursor;
pub mod dsh;
pub mod gemini_cli;
pub mod grok_cli;
pub mod marvis;
pub mod workbuddy;

pub fn adapt(source: TaskSource, vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    let result = catch_unwind(AssertUnwindSafe(|| match source {
        TaskSource::Codex => codex::from_vendor(vendor, envelope),
        TaskSource::Cursor => cursor::from_vendor(vendor, envelope),
        TaskSource::GeminiCli => gemini_cli::from_vendor(vendor, envelope),
        TaskSource::GrokCli => grok_cli::from_vendor(vendor, envelope),
        TaskSource::WorkBuddy => workbuddy::from_vendor(vendor, envelope),
        TaskSource::Marvis => marvis::from_vendor(vendor, envelope),
        TaskSource::DshDesktop => dsh::from_vendor(vendor, envelope),
        TaskSource::Unknown => Err("unknown source".to_string()),
    }));
    match result {
        Ok(ok) => ok,
        Err(_) => Err("adapter panicked".to_string()),
    }
}

pub(crate) fn read_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(found) = value
            .get(*key)
            .and_then(Value::as_str)
            .filter(|item| !item.is_empty())
        {
            return Some(found);
        }
    }
    None
}

pub(crate) fn event_type_from_hint(value: &Value, fallback: &Value) -> TaskEventType {
    for source in [value, fallback] {
        for key in [
            "type",
            "event",
            "status",
            "hook_event_name",
            "hookEventName",
        ] {
            let Some(hint) = source.get(key).and_then(Value::as_str) else {
                continue;
            };
            let mapped = match hint {
                "task.started" | "started" | "start" | "running" | "agent-turn-start"
                | "UserPromptSubmit" | "user_prompt_submit" | "beforeSubmitPrompt"
                | "BeforeAgent" => Some(TaskEventType::Started),
                "task.progress"
                | "progress"
                | "PreToolUse"
                | "pre_tool_use"
                | "PostToolUse"
                | "post_tool_use"
                | "PostToolUseFailure"
                | "post_tool_use_failure"
                | "postToolUse"
                | "afterAgentResponse"
                | "AfterTool" => Some(TaskEventType::Progress),
                "task.waiting" | "waiting" | "ask" | "needs-input" => Some(TaskEventType::Waiting),
                "task.failed" | "failed" | "error" | "errored" => Some(TaskEventType::Failed),
                "task.cancelled" | "cancelled" | "canceled" | "aborted" => {
                    Some(TaskEventType::Cancelled)
                }
                "task.completed"
                | "completed"
                | "complete"
                | "success"
                | "agent-turn-complete"
                | "stop"
                | "Stop"
                | "StopFailure"
                | "stop_failure"
                | "AfterAgent" => Some(TaskEventType::Completed),
                _ => None,
            };
            if let Some(mapped) = mapped {
                return mapped;
            }
        }
    }
    TaskEventType::Completed
}

pub(crate) fn event_id(vendor: &Value, envelope: &Value) -> String {
    read_str(envelope, &["eventId", "event_id"])
        .or_else(|| {
            read_str(
                vendor,
                &["eventId", "event_id", "id", "generation_id", "generationId"],
            )
        })
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub(crate) fn task_id(vendor: &Value, envelope: &Value) -> String {
    read_str(envelope, &["taskId", "task_id"])
        .or_else(|| {
            read_str(
                vendor,
                &[
                    "taskId",
                    "task_id",
                    "thread_id",
                    "thread-id",
                    "threadId",
                    "conversation_id",
                    "conversationId",
                    "session_id",
                    "sessionId",
                ],
            )
        })
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub(crate) fn occurred_at(vendor: &Value, envelope: &Value) -> String {
    read_str(envelope, &["occurredAt", "occurred_at"])
        .or_else(|| read_str(vendor, &["occurredAt", "timestamp", "time"]))
        .map(str::to_string)
        .unwrap_or_else(now_rfc3339)
}

pub(crate) fn title_from(vendor: &Value, envelope: &Value) -> Option<String> {
    read_str(envelope, &["title"])
        .or_else(|| read_str(vendor, &["title", "task_title", "taskTitle"]))
        .or_else(|| read_str(vendor, &["prompt"]))
        .or_else(|| first_user_message(vendor))
        .and_then(compact_title)
}

fn first_user_message(vendor: &Value) -> Option<&str> {
    vendor
        .get("input_messages")
        .or_else(|| vendor.get("input-messages"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
}

fn compact_title(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .collect();
    let first_line = cleaned.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() {
        return None;
    }
    Some(first_line.chars().take(80).collect())
}

pub(crate) fn workspace(vendor: &Value) -> Option<String> {
    if let Some(path) = read_str(
        vendor,
        &[
            "cwd",
            "workspace_path",
            "workspacePath",
            "workspaceRoot",
            "workspace",
        ],
    ) {
        return Some(path.to_string());
    }
    vendor
        .get("workspace_roots")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn summary_from(vendor: &Value, envelope: &Value) -> Option<String> {
    read_str(envelope, &["summary"])
        .or_else(|| {
            read_str(
                vendor,
                &[
                    "summary",
                    "last-assistant-message",
                    "last_assistant_message",
                    "message",
                    "text",
                ],
            )
        })
        .map(str::to_string)
}

pub(crate) fn deep_link(vendor: &Value, envelope: &Value) -> Option<String> {
    read_str(envelope, &["deepLink", "deep_link"])
        .or_else(|| read_str(vendor, &["deepLink", "deep_link", "url"]))
        .map(str::to_string)
}

pub(crate) fn build_event(source: TaskSource, vendor: &Value, envelope: &Value) -> TaskEvent {
    TaskEvent {
        schema_version: 1,
        event_id: event_id(vendor, envelope),
        source,
        event_type: event_type_from_hint(vendor, envelope),
        task_id: task_id(vendor, envelope),
        session_id: read_str(
            vendor,
            &["session_id", "sessionId", "thread_id", "conversation_id"],
        )
        .map(str::to_string),
        parent_task_id: None,
        project_name: read_str(vendor, &["project_name", "projectName", "project"])
            .map(str::to_string),
        workspace_path: workspace(vendor),
        title: title_from(vendor, envelope),
        summary: summary_from(vendor, envelope),
        occurred_at: occurred_at(vendor, envelope),
        deep_link: deep_link(vendor, envelope),
        raw: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;
    use std::fs;

    fn load_fixture(rel: &str) -> Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(rel);
        serde_json::from_slice(&fs::read(&path).expect(rel)).expect(rel)
    }

    #[test]
    fn vendor_fixtures_become_task_events() {
        let codex = adapt(
            TaskSource::Codex,
            &load_fixture("tests/fixtures/codex/turn-complete.json"),
            &json!({}),
        )
        .unwrap();
        assert_eq!(codex.event_type, TaskEventType::Completed);
        assert_eq!(codex.task_id, "thread-codex-1");

        let cursor = adapt(
            TaskSource::Cursor,
            &load_fixture("tests/fixtures/cursor/stop-completed.json"),
            &json!({}),
        )
        .unwrap();
        assert_eq!(cursor.source, TaskSource::Cursor);
        assert_eq!(cursor.task_id, "conv-cursor-1");

        let grok = adapt(
            TaskSource::GrokCli,
            &load_fixture("tests/fixtures/grok-cli/complete.json"),
            &json!({}),
        )
        .unwrap();
        assert_eq!(grok.source, TaskSource::GrokCli);
        assert_eq!(grok.event_type, TaskEventType::Completed);
    }

    #[test]
    fn unknown_source_is_isolated_error() {
        assert!(adapt(TaskSource::Unknown, &json!({}), &json!({})).is_err());
    }

    #[test]
    fn maps_hook_lifecycle_without_losing_vendor_fields() {
        let started = adapt(
            TaskSource::Codex,
            &json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": "thread-42",
                "turn_id": "turn-1",
                "prompt": "修复登录测试\n不要改接口"
            }),
            &json!({
                "eventId": "event-started",
                "type": "task.started",
                "occurredAt": "2026-08-13T04:00:00.000Z"
            }),
        )
        .unwrap();
        let progress = adapt(
            TaskSource::Codex,
            &json!({
                "hook_event_name": "PostToolUse",
                "session_id": "thread-42",
                "turn_id": "turn-1"
            }),
            &json!({
                "eventId": "event-progress",
                "type": "task.progress",
                "occurredAt": "2026-08-13T04:00:01.000Z"
            }),
        )
        .unwrap();

        assert_eq!(started.task_id, "thread-42");
        assert_eq!(progress.task_id, "thread-42");
        assert_eq!(started.title.as_deref(), Some("修复登录测试"));
        assert_eq!(progress.event_type, TaskEventType::Progress);
    }

    #[test]
    fn cursor_stop_status_wins_when_bridge_event_is_omitted() {
        let failed = adapt(
            TaskSource::Cursor,
            &json!({
                "hook_event_name": "stop",
                "status": "error",
                "conversation_id": "conv-9"
            }),
            &json!({
                "eventId": "event-failed",
                "occurredAt": "2026-08-13T04:00:02.000Z"
            }),
        )
        .unwrap();

        assert_eq!(failed.event_type, TaskEventType::Failed);
        assert_eq!(failed.task_id, "conv-9");
    }

    #[test]
    fn maps_native_grok_camel_case_lifecycle() {
        let started = adapt(
            TaskSource::GrokCli,
            &json!({
                "hookEventName": "user_prompt_submit",
                "sessionId": "grok-42",
                "workspaceRoot": "E:/workspace/app",
                "timestamp": "2026-08-13T04:00:00.000Z"
            }),
            &json!({}),
        )
        .unwrap();
        let progress = adapt(
            TaskSource::GrokCli,
            &json!({
                "hookEventName": "post_tool_use",
                "sessionId": "grok-42",
                "timestamp": "2026-08-13T04:00:01.000Z"
            }),
            &json!({}),
        )
        .unwrap();

        assert_eq!(started.event_type, TaskEventType::Started);
        assert_eq!(started.task_id, "grok-42");
        assert_eq!(started.workspace_path.as_deref(), Some("E:/workspace/app"));
        assert_eq!(progress.event_type, TaskEventType::Progress);
        assert_eq!(progress.task_id, "grok-42");
    }
}
