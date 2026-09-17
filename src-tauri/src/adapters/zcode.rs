//! ZCode configuration-file hooks adapter.
//!
//! ZCode sends Claude-Code-style hook payloads (`hook_event_name`, `session_id`,
//! `cwd`, `prompt`) on stdin; the bridge already strips conversation content, so
//! this adapter only maps the lifecycle fields.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::Zcode, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_native_lifecycle_hooks() {
        let started = from_vendor(
            &json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": "zcode-42",
                "cwd": "E:/workspace/app",
                "title": "修复登录测试"
            }),
            &json!({ "type": "task.started" }),
        )
        .unwrap();
        let progress = from_vendor(
            &json!({
                "hook_event_name": "PostToolUseFailure",
                "session_id": "zcode-42",
                "cwd": "E:/workspace/app"
            }),
            &json!({ "type": "task.progress" }),
        )
        .unwrap();
        let completed = from_vendor(
            &json!({
                "hook_event_name": "Stop",
                "session_id": "zcode-42",
                "cwd": "E:/workspace/app"
            }),
            &json!({ "type": "task.completed" }),
        )
        .unwrap();

        assert_eq!(started.source, TaskSource::Zcode);
        assert_eq!(started.event_type, TaskEventType::Started);
        assert_eq!(started.task_id, "zcode-42");
        assert_eq!(started.title.as_deref(), Some("修复登录测试"));
        assert_eq!(
            started.workspace_path.as_deref(),
            Some("E:/workspace/app")
        );
        assert_eq!(progress.event_type, TaskEventType::Progress);
        assert_eq!(completed.event_type, TaskEventType::Completed);
        assert_eq!(completed.task_id, "zcode-42");
    }
}
