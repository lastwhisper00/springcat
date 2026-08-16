//! Grok CLI hooks adapter. Vendor fields stay here; TaskEvent stays shared.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::GrokCli, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_complete_event() {
        let vendor = json!({
            "hookEventName": "stop",
            "sessionId": "grok-22",
            "workspaceRoot": "E:/workspace/app",
            "timestamp": "2026-08-13T12:00:00Z",
            "reason": "end_turn"
        });
        let event = from_vendor(&vendor, &json!({})).unwrap();
        assert_eq!(event.source, TaskSource::GrokCli);
        assert_eq!(event.event_type, TaskEventType::Completed);
        assert_eq!(event.task_id, "grok-22");
        assert_eq!(event.workspace_path.as_deref(), Some("E:/workspace/app"));
    }
}
