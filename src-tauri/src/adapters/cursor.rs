//! Cursor hooks adapter (`stop`, `sessionStart`, etc.).

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::Cursor, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_stop_completed() {
        let vendor = json!({
            "hook_event_name": "stop",
            "status": "completed",
            "conversation_id": "conv-9",
            "generation_id": "gen-1",
            "workspace_roots": ["E:/workspace/app"],
            "title": "refactor auth"
        });
        let event = from_vendor(&vendor, &json!({})).unwrap();
        assert_eq!(event.source, TaskSource::Cursor);
        assert_eq!(event.event_type, TaskEventType::Completed);
        assert_eq!(event.task_id, "conv-9");
        assert_eq!(event.title.as_deref(), Some("refactor auth"));
    }
}
