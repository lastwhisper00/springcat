//! Tool payload → TaskEvent. Adapters feed this module; UI never sees raw tool fields.

use serde_json::Value;

use crate::adapters;
use crate::domain::{parse_task_event, sanitize_summary, TaskEvent, TaskSource};

#[derive(Debug)]
pub enum NormalizeError {
    InvalidJson,
    Unusable,
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson => write!(f, "invalid json"),
            Self::Unusable => write!(f, "payload could not be normalized"),
        }
    }
}

pub fn parse_json(bytes: &[u8]) -> Result<Value, NormalizeError> {
    serde_json::from_slice(bytes).map_err(|_| NormalizeError::InvalidJson)
}

/// Convert inbox JSON into a TaskEvent. Unknown fields are ignored.
/// `raw` is dropped so transcripts never reach SQLite.
pub fn normalize_value(
    value: &Value,
    fallback_source: Option<TaskSource>,
) -> Result<TaskEvent, NormalizeError> {
    if let Some(mut event) = parse_task_event(value) {
        event.summary = sanitize_summary(event.summary.as_deref());
        event.raw = None;
        if matches!(event.source, TaskSource::Unknown) {
            if let Some(source) = fallback_source {
                event.source = source;
            }
        }
        return Ok(event);
    }

    let source = value
        .get("source")
        .and_then(|item| serde_json::from_value(item.clone()).ok())
        .or(fallback_source)
        .unwrap_or(TaskSource::Unknown);

    let vendor = value.get("vendor").unwrap_or(value);
    match adapters::adapt(source, vendor, value) {
        Ok(mut event) => {
            event.summary = sanitize_summary(event.summary.as_deref());
            event.raw = None;
            Ok(event)
        }
        Err(err) => {
            tracing::warn!(error = %err, "adapter failed");
            Err(NormalizeError::Unusable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drops_raw_and_unknown_fields() {
        let event = normalize_value(
            &json!({
                "schemaVersion": 1,
                "eventId": "e1",
                "source": "codex",
                "type": "task.completed",
                "taskId": "t1",
                "title": "ok",
                "occurredAt": "2026-08-13T04:00:00.000Z",
                "raw": { "prompt": "secret" },
                "prompt": "also secret"
            }),
            None,
        )
        .unwrap();
        assert!(event.raw.is_none());
        assert_eq!(event.title.as_deref(), Some("ok"));
    }

    #[test]
    fn corrupt_fixture_is_invalid_json() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/inbox/corrupt.json");
        let bytes = std::fs::read(path).unwrap();
        assert!(matches!(
            parse_json(&bytes),
            Err(NormalizeError::InvalidJson)
        ));
    }

    #[test]
    fn unknown_field_fixture_normalizes() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/inbox/unknown-fields.json");
        let value = parse_json(&std::fs::read(path).unwrap()).unwrap();
        let event = normalize_value(&value, None).unwrap();
        assert_eq!(event.event_id, "evt-unknown-fields");
        assert!(event.raw.is_none());
    }
}
