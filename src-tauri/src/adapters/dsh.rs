//! DeepSeek Harness Desktop passive-monitor payload adapter.

use serde_json::Value;

use crate::domain::{TaskEvent, TaskSource};

use super::build_event;

pub fn from_vendor(vendor: &Value, envelope: &Value) -> Result<TaskEvent, String> {
    Ok(build_event(TaskSource::DshDesktop, vendor, envelope))
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
                "conversation_id": "session-1",
                "title": "优化监听"
            }),
            &json!({}),
        )
        .unwrap();
        assert_eq!(event.source, TaskSource::DshDesktop);
        assert_eq!(event.event_type, TaskEventType::Completed);
    }
}
