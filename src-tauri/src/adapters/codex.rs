//! Codex CLI notify / hooks adapter.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::Codex, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_turn_complete() {
        let vendor = json!({
            "type": "agent-turn-complete",
            "thread_id": "thread-1",
            "cwd": "E:/workspace/app",
            "last-assistant-message": "fixed login tests"
        });
        let event = from_vendor(&vendor, &json!({})).unwrap();
        assert_eq!(event.source, TaskSource::Codex);
        assert_eq!(event.event_type, TaskEventType::Completed);
        assert_eq!(event.task_id, "thread-1");
        assert_eq!(event.workspace_path.as_deref(), Some("E:/workspace/app"));
    }
}
