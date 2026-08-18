//! Marvis passive-monitor payload adapter used by the settings connection test.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::Marvis, vendor, envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskEventType;
    use serde_json::json;

    #[test]
    fn maps_completed_message() {
        let event = from_vendor(
            &json!({
                "type": "completed",
                "conversation_id": "conv-1",
                "title": "整理桌面"
            }),
            &json!({}),
        )
        .unwrap();
        assert_eq!(event.source, TaskSource::Marvis);
        assert_eq!(event.event_type, TaskEventType::Completed);
    }
}
