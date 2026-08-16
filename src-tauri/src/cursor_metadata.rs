//! Best-effort Cursor title lookup. Only the generated conversation name is
//! read; prompts, responses, transcripts, and tool payloads stay out of SpringCat.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::domain::TaskSource;
use crate::repository::Repository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationState {
    pub status: String,
    pub updated_at_millis: i64,
    pub generation_id: Option<String>,
}

pub fn conversation_title(conversation_id: &str) -> Option<String> {
    title_from_db(&state_db_path()?, conversation_id)
        .ok()
        .flatten()
}

/// Read only the lifecycle fields needed to repair a missed Cursor terminal
/// hook. Conversation text, tool payloads, and transcripts never leave the
/// Cursor database.
pub fn conversation_state(conversation_id: &str) -> Option<ConversationState> {
    state_from_db(&state_db_path()?, conversation_id)
        .ok()
        .flatten()
}

pub fn backfill_untitled(repo: &Repository) -> Result<usize, String> {
    let mut changed = 0;
    for task_id in repo.untitled_task_ids(TaskSource::Cursor)? {
        let Some(title) = conversation_title(&task_id) else {
            continue;
        };
        if repo.update_title_if_untitled(&task_id, &title)? {
            changed += 1;
        }
    }
    Ok(changed)
}

pub fn state_db_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"),
    )
}

fn state_from_db(path: &Path, conversation_id: &str) -> Result<Option<ConversationState>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| err.to_string())?;
    let key = format!("composerData:{conversation_id}");
    let raw = connection
        .query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            [&key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;
    Ok(raw.as_deref().and_then(state_from_json))
}

fn title_from_db(path: &Path, conversation_id: &str) -> Result<Option<String>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| err.to_string())?;
    let key = format!("composerData:{conversation_id}");

    if let Ok(Some(raw)) = connection
        .query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            [&key],
            |row| row.get::<_, String>(0),
        )
        .optional()
    {
        if let Some(title) = title_from_json(&raw) {
            return Ok(Some(title));
        }
    }

    let raw = connection
        .query_row(
            "SELECT value FROM composerHeaders WHERE composerId = ?1",
            [conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap_or(None);
    Ok(raw.as_deref().and_then(title_from_json))
}

fn title_from_json(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    compact_title(value.get("name")?.as_str()?)
}

fn state_from_json(raw: &str) -> Option<ConversationState> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let status = value.get("status")?.as_str()?.trim().to_ascii_lowercase();
    if status.is_empty() {
        return None;
    }
    let updated_at_millis = ["conversationCheckpointLastUpdatedAt", "lastUpdatedAt"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(json_millis))
        .max()?;
    let generation_id = value
        .get("latestChatGenerationUUID")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(ConversationState {
        status,
        updated_at_millis,
        generation_id,
    })
}

fn json_millis(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn compact_title(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .collect();
    let first_line = cleaned.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() || is_placeholder_title(first_line) {
        return None;
    }
    Some(first_line.chars().take(80).collect())
}

fn is_placeholder_title(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "new chat" | "new conversation" | "untitled" | "未命名" | "未命名任务"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_the_generated_name_for_an_exact_conversation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.vscdb");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE cursorDiskKV (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB);",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                (
                    "composerData:cursor-1",
                    r#"{"name":"Guanchao backend logging setup","text":"private"}"#,
                ),
            )
            .unwrap();
        drop(connection);

        assert_eq!(
            title_from_db(&path, "cursor-1").unwrap().as_deref(),
            Some("Guanchao backend logging setup")
        );
        assert!(title_from_db(&path, "missing").unwrap().is_none());
    }

    #[test]
    fn ignores_cursor_placeholder_names() {
        assert!(title_from_json(r#"{"name":"New Chat"}"#).is_none());
    }

    #[test]
    fn reads_only_terminal_reconciliation_fields() {
        let state = state_from_json(
            r#"{
                "status":"completed",
                "lastUpdatedAt":100,
                "conversationCheckpointLastUpdatedAt":250,
                "latestChatGenerationUUID":"generation-2",
                "text":"private prompt",
                "conversationMap":{"private":"content"}
            }"#,
        )
        .unwrap();

        assert_eq!(state.status, "completed");
        assert_eq!(state.updated_at_millis, 250);
        assert_eq!(state.generation_id.as_deref(), Some("generation-2"));
    }

    #[test]
    fn state_requires_a_cursor_update_timestamp() {
        assert!(state_from_json(r#"{"status":"completed"}"#).is_none());
    }
}
