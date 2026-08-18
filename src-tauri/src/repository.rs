//! SQLite task repository. Never persist full transcripts, source code, or credentials.

use std::path::Path;

use chrono::Datelike;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::domain::{
    apply_event_to_task, TaskAction, TaskEvent, TaskEventType, TaskItem, TaskSource, TaskStatus,
    UNTITLED_TASK_TITLE,
};

const MAX_LIST: usize = 50;

pub struct Repository {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub id: String,
    pub source: TaskSource,
    pub external_event_id: String,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub occurred_at: String,
    pub local_date: String,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub collection_method: String,
    pub accuracy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub source: TaskSource,
    pub model: Option<String>,
    pub context_tier: String,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        let repo = Self { conn };
        repo.init()?;
        Ok(repo)
    }

    #[allow(dead_code)]
    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| err.to_string())?;
        let repo = Self { conn };
        repo.init()?;
        Ok(repo)
    }

    fn init(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                CREATE TABLE IF NOT EXISTS events (
                    event_id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    occurred_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS tasks (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    title TEXT NOT NULL,
                    summary TEXT,
                    status TEXT NOT NULL,
                    started_at TEXT,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT,
                    unread INTEGER NOT NULL DEFAULT 0,
                    action_label TEXT,
                    action_deep_link TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_tasks_updated ON tasks(updated_at DESC);
                CREATE TABLE IF NOT EXISTS usage_records (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    external_event_id TEXT NOT NULL,
                    session_id TEXT,
                    model TEXT,
                    occurred_at TEXT NOT NULL,
                    local_date TEXT NOT NULL,
                    input_tokens INTEGER NOT NULL DEFAULT 0,
                    cached_input_tokens INTEGER NOT NULL DEFAULT 0,
                    output_tokens INTEGER NOT NULL DEFAULT 0,
                    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
                    total_tokens INTEGER NOT NULL DEFAULT 0,
                    collection_method TEXT NOT NULL,
                    accuracy TEXT NOT NULL,
                    UNIQUE(source, external_event_id)
                );
                CREATE INDEX IF NOT EXISTS idx_usage_calendar
                    ON usage_records(local_date, source);
                ",
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn event_seen(&self, event_id: &str) -> Result<bool, String> {
        let exists: Option<String> = self
            .conn
            .query_row(
                "SELECT event_id FROM events WHERE event_id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        Ok(exists.is_some())
    }

    pub fn insert_event(&self, event: &TaskEvent) -> Result<bool, String> {
        let changed = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO events (event_id, task_id, event_type, occurred_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    event.event_id,
                    event.task_id,
                    event_type_sql(event.event_type),
                    event.occurred_at
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(changed == 1)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<TaskItem>, String> {
        self.conn
            .query_row(
                "SELECT id, source, title, summary, status, started_at, updated_at, completed_at,
                        unread, action_label, action_deep_link
                 FROM tasks WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()
            .map_err(|err| err.to_string())
    }

    pub fn upsert_task(&self, task: &TaskItem) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO tasks (
                    id, source, title, summary, status, started_at, updated_at, completed_at,
                    unread, action_label, action_deep_link
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    source = excluded.source,
                    title = excluded.title,
                    summary = excluded.summary,
                    status = excluded.status,
                    started_at = excluded.started_at,
                    updated_at = excluded.updated_at,
                    completed_at = excluded.completed_at,
                    unread = excluded.unread,
                    action_label = excluded.action_label,
                    action_deep_link = excluded.action_deep_link",
                params![
                    task.id,
                    source_sql(task.source),
                    task.title,
                    task.summary,
                    status_sql(task.status),
                    task.started_at,
                    task.updated_at,
                    task.completed_at,
                    task.unread as i32,
                    task.action.as_ref().map(|action| action.label.clone()),
                    task.action
                        .as_ref()
                        .and_then(|action| action.deep_link.clone()),
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn apply_event(&self, event: &TaskEvent) -> Result<Option<TaskItem>, String> {
        if !self.insert_event(event)? {
            return Ok(None);
        }
        let existing = self.get_task(&event.task_id)?;
        let next = apply_ordered(existing.as_ref(), event);
        self.upsert_task(&next)?;
        Ok(Some(next))
    }

    pub fn list_recent(&self) -> Result<Vec<TaskItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, source, title, summary, status, started_at, updated_at, completed_at,
                        unread, action_label, action_deep_link
                 FROM tasks
                 ORDER BY updated_at DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![MAX_LIST as i64], row_to_task)
            .map_err(|err| err.to_string())?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|err| err.to_string())?);
        }
        Ok(items)
    }

    pub fn list_active_for_source(&self, source: TaskSource) -> Result<Vec<TaskItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, source, title, summary, status, started_at, updated_at, completed_at,
                        unread, action_label, action_deep_link
                 FROM tasks
                 WHERE source = ?1 AND status IN ('running', 'waiting')
                 ORDER BY updated_at DESC, id DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![source_sql(source)], row_to_task)
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn insert_usage(&self, usage: &UsageRecord) -> Result<bool, String> {
        let changed = self
            .conn
            .execute(
                "INSERT INTO usage_records (
                    id, source, external_event_id, session_id, model, occurred_at, local_date,
                    input_tokens, cached_input_tokens, output_tokens, reasoning_tokens,
                    total_tokens, collection_method, accuracy
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET model = excluded.model
                 WHERE (usage_records.model IS NULL OR usage_records.model = '')
                   AND excluded.model IS NOT NULL
                   AND excluded.model != ''",
                params![
                    usage.id,
                    source_sql(usage.source),
                    usage.external_event_id,
                    usage.session_id,
                    usage.model,
                    usage.occurred_at,
                    usage.local_date,
                    usage.input_tokens,
                    usage.cached_input_tokens,
                    usage.output_tokens,
                    usage.reasoning_tokens,
                    usage.total_tokens,
                    usage.collection_method,
                    usage.accuracy,
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(changed == 1)
    }

    pub fn list_usage_month(&self, month: &str) -> Result<Vec<DailyUsage>, String> {
        let (start, end) = month_range(month)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT local_date, source, model,
                        CASE
                          WHEN source = 'codex' AND input_tokens > 272000 THEN 'long'
                          WHEN source = 'grok-cli' AND input_tokens >= 200000 THEN 'long'
                          ELSE 'short'
                        END AS context_tier,
                        SUM(input_tokens), SUM(cached_input_tokens), SUM(output_tokens),
                        SUM(reasoning_tokens), SUM(total_tokens)
                 FROM usage_records
                 WHERE local_date >= ?1 AND local_date < ?2
                 GROUP BY local_date, source, model, context_tier
                 ORDER BY local_date ASC, source ASC, model ASC, context_tier ASC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                let source: String = row.get(1)?;
                Ok(DailyUsage {
                    date: row.get(0)?,
                    source: source_from_sql(&source),
                    model: row.get(2)?,
                    context_tier: row.get(3)?,
                    input_tokens: row.get(4)?,
                    cached_input_tokens: row.get(5)?,
                    output_tokens: row.get(6)?,
                    reasoning_tokens: row.get(7)?,
                    total_tokens: row.get(8)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn untitled_task_ids(&self, source: TaskSource) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id FROM tasks
                 WHERE source = ?1 AND title = ?2
                 ORDER BY updated_at DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![source_sql(source), UNTITLED_TASK_TITLE], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn update_title_if_untitled(&self, task_id: &str, title: &str) -> Result<bool, String> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(false);
        }
        self.conn
            .execute(
                "UPDATE tasks SET title = ?1 WHERE id = ?2 AND title = ?3",
                params![title, task_id, UNTITLED_TASK_TITLE],
            )
            .map(|changed| changed == 1)
            .map_err(|err| err.to_string())
    }

    /// Merge the complete task/event history into another cache database.
    /// The source remains untouched so changing cache locations is recoverable.
    pub fn copy_all_to(&self, path: &Path) -> Result<(), String> {
        let destination = Repository::open(path)?;

        let mut task_stmt = self
            .conn
            .prepare(
                "SELECT id, source, title, summary, status, started_at, updated_at, completed_at,
                        unread, action_label, action_deep_link
                 FROM tasks
                 ORDER BY updated_at ASC, id ASC",
            )
            .map_err(|err| err.to_string())?;
        let tasks = task_stmt
            .query_map([], row_to_task)
            .map_err(|err| err.to_string())?;
        for task in tasks {
            destination.upsert_task(&task.map_err(|err| err.to_string())?)?;
        }

        let mut event_stmt = self
            .conn
            .prepare(
                "SELECT event_id, task_id, event_type, occurred_at
                 FROM events
                 ORDER BY occurred_at ASC, event_id ASC",
            )
            .map_err(|err| err.to_string())?;
        let events = event_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|err| err.to_string())?;
        for event in events {
            let (event_id, task_id, event_type, occurred_at) =
                event.map_err(|err| err.to_string())?;
            destination
                .conn
                .execute(
                    "INSERT OR IGNORE INTO events (event_id, task_id, event_type, occurred_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![event_id, task_id, event_type, occurred_at],
                )
                .map_err(|err| err.to_string())?;
        }

        let mut usage_stmt = self
            .conn
            .prepare(
                "SELECT id, source, external_event_id, session_id, model, occurred_at, local_date,
                        input_tokens, cached_input_tokens, output_tokens, reasoning_tokens,
                        total_tokens, collection_method, accuracy
                 FROM usage_records
                 ORDER BY occurred_at ASC, id ASC",
            )
            .map_err(|err| err.to_string())?;
        let usage_rows = usage_stmt
            .query_map([], |row| {
                let source: String = row.get(1)?;
                Ok(UsageRecord {
                    id: row.get(0)?,
                    source: source_from_sql(&source),
                    external_event_id: row.get(2)?,
                    session_id: row.get(3)?,
                    model: row.get(4)?,
                    occurred_at: row.get(5)?,
                    local_date: row.get(6)?,
                    input_tokens: row.get(7)?,
                    cached_input_tokens: row.get(8)?,
                    output_tokens: row.get(9)?,
                    reasoning_tokens: row.get(10)?,
                    total_tokens: row.get(11)?,
                    collection_method: row.get(12)?,
                    accuracy: row.get(13)?,
                })
            })
            .map_err(|err| err.to_string())?;
        for usage in usage_rows {
            destination.insert_usage(&usage.map_err(|err| err.to_string())?)?;
        }
        Ok(())
    }

    pub fn mark_read(&self, task_id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tasks SET unread = 0 WHERE id = ?1",
                params![task_id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn mark_all_read(&self) -> Result<(), String> {
        self.conn
            .execute("UPDATE tasks SET unread = 0", [])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn purge(&self, retention_days: u32) -> Result<(), String> {
        if retention_days == 0 {
            self.conn
                .execute(
                    "DELETE FROM tasks WHERE status NOT IN ('running', 'waiting')",
                    [],
                )
                .map_err(|err| err.to_string())?;
            return Ok(());
        }
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        let cutoff = cutoff.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.conn
            .execute(
                "DELETE FROM tasks WHERE updated_at < ?1 AND status NOT IN ('running', 'waiting')",
                params![cutoff],
            )
            .map_err(|err| err.to_string())?;
        self.conn
            .execute("DELETE FROM events WHERE occurred_at < ?1", params![cutoff])
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn month_range(month: &str) -> Result<(String, String), String> {
    let start = chrono::NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
        .map_err(|_| "月份格式必须是 YYYY-MM".to_string())?;
    let (next_year, next_month) = if start.month() == 12 {
        (start.year() + 1, 1)
    } else {
        (start.year(), start.month() + 1)
    };
    let end = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .ok_or_else(|| "月份超出支持范围".to_string())?;
    Ok((start.to_string(), end.to_string()))
}

fn apply_ordered(existing: Option<&TaskItem>, event: &TaskEvent) -> TaskItem {
    if let Some(current) = existing {
        if current.updated_at > event.occurred_at {
            let mut kept = current.clone();
            if kept.started_at.is_none() && event.event_type == TaskEventType::Started {
                kept.started_at = Some(event.occurred_at.clone());
            }
            if kept.title == UNTITLED_TASK_TITLE {
                if let Some(title) = event
                    .title
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    kept.title = title.to_string();
                }
            }
            return kept;
        }
    }
    apply_event_to_task(existing, event)
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskItem> {
    let source_raw: String = row.get(1)?;
    let status_raw: String = row.get(4)?;
    let unread: i32 = row.get(8)?;
    let action_label: Option<String> = row.get(9)?;
    let action_deep_link: Option<String> = row.get(10)?;
    Ok(TaskItem {
        id: row.get(0)?,
        source: source_from_sql(&source_raw),
        title: row.get(2)?,
        summary: row.get(3)?,
        status: status_from_sql(&status_raw),
        started_at: row.get(5)?,
        updated_at: row.get(6)?,
        completed_at: row.get(7)?,
        unread: unread != 0,
        action: action_label.map(|label| TaskAction {
            label,
            deep_link: action_deep_link,
        }),
    })
}

fn source_sql(source: TaskSource) -> &'static str {
    match source {
        TaskSource::Codex => "codex",
        TaskSource::Cursor => "cursor",
        TaskSource::GrokCli => "grok-cli",
        TaskSource::GeminiCli => "gemini-cli",
        TaskSource::WorkBuddy => "workbuddy",
        TaskSource::Marvis => "marvis",
        TaskSource::Unknown => "unknown",
    }
}

fn source_from_sql(value: &str) -> TaskSource {
    match value {
        "codex" => TaskSource::Codex,
        "cursor" => TaskSource::Cursor,
        "grok-cli" => TaskSource::GrokCli,
        "gemini-cli" => TaskSource::GeminiCli,
        "workbuddy" => TaskSource::WorkBuddy,
        "marvis" => TaskSource::Marvis,
        _ => TaskSource::Unknown,
    }
}

fn status_sql(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Running => "running",
        TaskStatus::Waiting => "waiting",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn status_from_sql(value: &str) -> TaskStatus {
    match value {
        "waiting" => TaskStatus::Waiting,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Running,
    }
}

fn event_type_sql(event_type: TaskEventType) -> &'static str {
    match event_type {
        TaskEventType::Started => "task.started",
        TaskEventType::Progress => "task.progress",
        TaskEventType::Waiting => "task.waiting",
        TaskEventType::Completed => "task.completed",
        TaskEventType::Failed => "task.failed",
        TaskEventType::Cancelled => "task.cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(id: &str, task: &str, ty: &str, at: &str) -> TaskEvent {
        crate::domain::parse_task_event(&json!({
            "schemaVersion": 1,
            "eventId": id,
            "source": "codex",
            "type": ty,
            "taskId": task,
            "title": "fix login",
            "occurredAt": at
        }))
        .unwrap()
    }

    #[test]
    fn dedupes_event_id() {
        let repo = Repository::memory().unwrap();
        let first = repo
            .apply_event(&event(
                "e1",
                "t1",
                "task.completed",
                "2026-08-13T04:00:00.000Z",
            ))
            .unwrap();
        let second = repo
            .apply_event(&event(
                "e1",
                "t1",
                "task.completed",
                "2026-08-13T04:00:00.000Z",
            ))
            .unwrap();
        assert!(first.is_some());
        assert!(second.is_none());
        assert_eq!(repo.list_recent().unwrap().len(), 1);
    }

    #[test]
    fn out_of_order_start_does_not_regress_completed() {
        let repo = Repository::memory().unwrap();
        repo.apply_event(&event(
            "e2",
            "t1",
            "task.completed",
            "2026-08-13T04:02:00.000Z",
        ))
        .unwrap();
        repo.apply_event(&event(
            "e1",
            "t1",
            "task.started",
            "2026-08-13T04:00:00.000Z",
        ))
        .unwrap();
        let task = repo.get_task("t1").unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.started_at.as_deref(), Some("2026-08-13T04:00:00.000Z"));
    }

    #[test]
    fn copies_history_to_another_database() {
        let source = Repository::memory().unwrap();
        source
            .apply_event(&event(
                "e1",
                "t1",
                "task.completed",
                "2026-08-13T04:00:00.000Z",
            ))
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let destination_path = temp.path().join("tasks.sqlite");

        source.copy_all_to(&destination_path).unwrap();

        let destination = Repository::open(&destination_path).unwrap();
        assert_eq!(destination.list_recent().unwrap().len(), 1);
        assert!(destination.event_seen("e1").unwrap());
    }

    #[test]
    fn backfill_only_replaces_untitled_tasks() {
        let repo = Repository::memory().unwrap();
        repo.apply_event(
            &crate::domain::parse_task_event(&json!({
                "schemaVersion": 1,
                "eventId": "cursor-untitled",
                "source": "cursor",
                "type": "task.completed",
                "taskId": "cursor-1",
                "occurredAt": "2026-08-13T04:00:00.000Z"
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            repo.untitled_task_ids(TaskSource::Cursor).unwrap(),
            vec!["cursor-1"]
        );
        assert!(repo
            .update_title_if_untitled("cursor-1", "Cursor generated title")
            .unwrap());
        assert!(!repo
            .update_title_if_untitled("cursor-1", "Second title")
            .unwrap());
        assert_eq!(
            repo.get_task("cursor-1").unwrap().unwrap().title,
            "Cursor generated title"
        );
    }

    #[test]
    fn lists_all_active_tasks_for_one_source() {
        let repo = Repository::memory().unwrap();
        repo.apply_event(
            &crate::domain::parse_task_event(&json!({
                "schemaVersion": 1,
                "eventId": "cursor-running",
                "source": "cursor",
                "type": "task.started",
                "taskId": "cursor-1",
                "occurredAt": "2026-08-13T04:00:00.000Z"
            }))
            .unwrap(),
        )
        .unwrap();
        repo.apply_event(
            &crate::domain::parse_task_event(&json!({
                "schemaVersion": 1,
                "eventId": "codex-running",
                "source": "codex",
                "type": "task.started",
                "taskId": "codex-1",
                "occurredAt": "2026-08-13T04:01:00.000Z"
            }))
            .unwrap(),
        )
        .unwrap();

        let cursor = repo.list_active_for_source(TaskSource::Cursor).unwrap();
        assert_eq!(cursor.len(), 1);
        assert_eq!(cursor[0].id, "cursor-1");
    }

    #[test]
    fn aggregates_usage_by_day_and_source() {
        let repo = Repository::memory().unwrap();
        for (id, source, input, output) in [
            ("u1", TaskSource::Codex, 1_000, 200),
            ("u2", TaskSource::Codex, 800, 100),
            ("u3", TaskSource::GrokCli, 600, 80),
        ] {
            repo.insert_usage(&UsageRecord {
                id: id.to_string(),
                source,
                external_event_id: id.to_string(),
                session_id: Some("session-1".into()),
                model: Some("test-model".into()),
                occurred_at: "2026-08-14T04:00:00.000Z".into(),
                local_date: "2026-08-14".into(),
                input_tokens: input,
                cached_input_tokens: input / 2,
                output_tokens: output,
                reasoning_tokens: output / 2,
                total_tokens: input + output,
                collection_method: "fixture".into(),
                accuracy: "exact".into(),
            })
            .unwrap();
        }

        let rows = repo.list_usage_month("2026-08").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source, TaskSource::Codex);
        assert_eq!(rows[0].model.as_deref(), Some("test-model"));
        assert_eq!(rows[0].context_tier, "short");
        assert_eq!(rows[0].input_tokens, 1_800);
        assert_eq!(rows[0].total_tokens, 2_100);
        assert_eq!(rows[1].source, TaskSource::GrokCli);
        assert_eq!(rows[1].total_tokens, 680);
    }

    #[test]
    fn rejects_invalid_usage_month() {
        let repo = Repository::memory().unwrap();
        assert!(repo.list_usage_month("2026-13").is_err());
        assert!(repo.list_usage_month("August").is_err());
    }
}
