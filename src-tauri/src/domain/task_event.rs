use serde::{Deserialize, Serialize};

use super::constants::SUMMARY_MAX_LENGTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskSource {
    Codex,
    Cursor,
    #[serde(rename = "grok-cli")]
    GrokCli,
    #[serde(rename = "gemini-cli")]
    GeminiCli,
    #[serde(rename = "workbuddy")]
    WorkBuddy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEventType {
    #[serde(rename = "task.started")]
    Started,
    #[serde(rename = "task.progress")]
    Progress,
    #[serde(rename = "task.waiting")]
    Waiting,
    #[serde(rename = "task.completed")]
    Completed,
    #[serde(rename = "task.failed")]
    Failed,
    #[serde(rename = "task.cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub source: TaskSource,
    #[serde(rename = "type")]
    pub event_type: TaskEventType,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

pub fn sanitize_summary(summary: Option<&str>) -> Option<String> {
    let value = summary?;
    let cleaned: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.chars().count() > SUMMARY_MAX_LENGTH {
        Some(cleaned.chars().take(SUMMARY_MAX_LENGTH).collect())
    } else {
        Some(cleaned)
    }
}

fn read_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// Parse JSON while ignoring unknown fields. Returns None when required fields are missing.
pub fn parse_task_event(input: &serde_json::Value) -> Option<TaskEvent> {
    let object = input.as_object()?;
    let event_id = read_string(object.get("eventId"))?;
    let task_id = read_string(object.get("taskId"))?;
    let occurred_at = read_string(object.get("occurredAt"))?;
    let type_raw = read_string(object.get("type"))?;
    let event_type: TaskEventType =
        serde_json::from_value(serde_json::Value::String(type_raw)).ok()?;
    let source = object
        .get("source")
        .and_then(|item| serde_json::from_value(item.clone()).ok())
        .unwrap_or(TaskSource::Unknown);

    let mut event = TaskEvent {
        schema_version: 1,
        event_id,
        source,
        event_type,
        task_id,
        session_id: None,
        parent_task_id: None,
        project_name: None,
        workspace_path: None,
        title: None,
        summary: None,
        occurred_at,
        deep_link: None,
        raw: None,
    };

    event.session_id = read_string(object.get("sessionId"));
    event.parent_task_id = read_string(object.get("parentTaskId"));
    event.project_name = read_string(object.get("projectName"));
    event.workspace_path = read_string(object.get("workspacePath"));
    event.title = read_string(object.get("title"));
    event.summary = sanitize_summary(read_string(object.get("summary")).as_deref());
    event.deep_link = read_string(object.get("deepLink"));
    if let Some(raw) = object.get("raw") {
        event.raw = Some(raw.clone());
    }

    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ignores_unknown_fields() {
        let parsed = parse_task_event(&json!({
            "schemaVersion": 1,
            "eventId": "e1",
            "source": "codex",
            "type": "task.completed",
            "taskId": "t1",
            "title": "fix login tests",
            "occurredAt": "2026-08-13T04:00:00.000Z",
            "extraToolField": { "nested": true },
            "prompt": "should not leak"
        }))
        .expect("valid event");

        assert_eq!(parsed.event_id, "e1");
        assert_eq!(parsed.source, TaskSource::Codex);
        let encoded = serde_json::to_value(&parsed).unwrap();
        assert!(encoded.get("extraToolField").is_none());
        assert!(encoded.get("prompt").is_none());
    }

    #[test]
    fn sanitizes_summary() {
        assert_eq!(
            sanitize_summary(Some("ok\u{0000}job")).as_deref(),
            Some("okjob")
        );
        assert_eq!(sanitize_summary(Some("   ")), None);
        assert_eq!(
            sanitize_summary(Some(&"x".repeat(200)))
                .unwrap()
                .chars()
                .count(),
            160
        );
    }
}
