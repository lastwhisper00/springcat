//! Gemini CLI native hooks adapter.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::GeminiCli, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_native_agent_hooks() {
        let started = from_vendor(
            &json!({
                "hook_event_name": "BeforeAgent",
                "session_id": "gemini-42",
                "cwd": "E:/workspace/app",
                "prompt": "修复登录测试"
            }),
            &json!({}),
        )
        .unwrap();
        let completed = from_vendor(
            &json!({
                "hook_event_name": "AfterAgent",
                "session_id": "gemini-42",
                "cwd": "E:/workspace/app",
                "prompt_response": "已完成"
            }),
            &json!({}),
        )
        .unwrap();

        assert_eq!(started.source, TaskSource::GeminiCli);
        assert_eq!(started.event_type, TaskEventType::Started);
        assert_eq!(started.task_id, "gemini-42");
        assert_eq!(started.title.as_deref(), Some("修复登录测试"));
        assert_eq!(completed.event_type, TaskEventType::Completed);
    }
}
