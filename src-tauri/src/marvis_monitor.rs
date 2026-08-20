//! Passive Marvis lifecycle and exact token-usage monitor.
//!
//! Marvis writes structured AG-UI lifecycle rows and token counters to
//! `~/.marvis/database/data.db`. SpringCat opens that database read-only and
//! selects only lifecycle events, short user titles, bounded final summaries,
//! approval states, and numeric usage fields. Streaming reasoning, tool
//! arguments, tool results, credentials, and full transcripts are never copied
//! into SpringCat's database.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::domain::{TaskEvent, TaskEventType, TaskSource};
use crate::event_collector::{self, CollectorState};
use crate::repository::UsageRecord;
use crate::settings_store::{occurred_at_rfc3339, PersistedSettings};

const SYNC_DEBOUNCE: Duration = Duration::from_millis(250);
const SETUP_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const TITLE_LIMIT: usize = 80;
const SUMMARY_LIMIT: usize = 160;

pub struct MarvisMonitorState {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
}

#[derive(Debug, Default)]
struct MonitorCursor {
    initialized: bool,
    last_event_rowid: i64,
    last_usage_id: i64,
    titles: HashMap<String, String>,
    active: HashMap<String, ActiveRun>,
    approval_statuses: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct ActiveRun {
    conversation_id: String,
    title: String,
    started_at: String,
    progress_seen: bool,
}

#[derive(Debug)]
struct VendorEvent {
    rowid: i64,
    event_id: String,
    conversation_id: String,
    response_id: String,
    event_type: String,
    data: String,
    timestamp: String,
}

#[derive(Debug)]
struct ApprovalState {
    approval_id: String,
    conversation_id: String,
    status: String,
    occurred_at: String,
}

#[derive(Debug)]
struct PendingLifecycle {
    vendor_event_id: String,
    task_id: String,
    session_id: String,
    event_type: TaskEventType,
    title: String,
    summary: Option<String>,
    occurred_at: String,
}

pub fn database_path() -> Option<PathBuf> {
    marvis_home().map(|home| home.join("database").join("data.db"))
}

/// Resolve a SpringCat Marvis task (a response id) back to the owning
/// conversation and build the route understood by Marvis' pseudo protocol.
/// This lookup also repairs tasks collected before deep links were persisted.
pub(crate) fn conversation_link_for_response(response_id: &str) -> Option<String> {
    if response_id.is_empty() || response_id.len() > 256 {
        return None;
    }
    let database = database_path()?;
    let conn = open_read_only(&database).ok()?;
    let conversation_id: String = conn
        .query_row(
            "SELECT conversation_id FROM agui_events
             WHERE response_id = ?1
             ORDER BY rowid DESC LIMIT 1",
            [response_id],
            |row| row.get(0),
        )
        .optional()
        .ok()??;
    conversation_deep_link(&conversation_id)
}

fn conversation_deep_link(conversation_id: &str) -> Option<String> {
    let safe = !conversation_id.is_empty()
        && conversation_id.len() <= 256
        && conversation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    safe.then(|| {
        format!("marvis://client/gotoRoute?feature=springcat&url=%2Fchat%2F{conversation_id}")
    })
}

fn marvis_home() -> Option<PathBuf> {
    std::env::var_os("MARVIS_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".marvis")))
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let database = database_path().ok_or_else(|| "无法定位 Marvis 数据目录".to_string())?;
    let database_dir = database
        .parent()
        .ok_or_else(|| "Marvis 数据库路径无效".to_string())?
        .to_path_buf();
    let marvis_home = database_dir
        .parent()
        .ok_or_else(|| "Marvis 数据目录无效".to_string())?
        .to_path_buf();
    let user_home = marvis_home
        .parent()
        .ok_or_else(|| "用户目录无效".to_string())?
        .to_path_buf();

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .map_err(|err| err.to_string())?;
    let initial_watch = if database_dir.is_dir() {
        database_dir.clone()
    } else if marvis_home.is_dir() {
        marvis_home.clone()
    } else {
        user_home
    };
    watcher
        .watch(&initial_watch, RecursiveMode::NonRecursive)
        .map_err(|err| err.to_string())?;

    let watcher = Arc::new(Mutex::new(watcher));
    app.manage(MarvisMonitorState {
        _watcher: watcher.clone(),
    });

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-marvis-monitor".into())
        .spawn(move || {
            let mut watched_path = initial_watch;
            let mut watching_database = watched_path == database_dir;
            let mut cursor = MonitorCursor::default();
            let mut dirty = database.is_file();
            let mut last_sync = Instant::now()
                .checked_sub(SYNC_DEBOUNCE)
                .unwrap_or_else(Instant::now);
            let mut last_setup_check = Instant::now()
                .checked_sub(SETUP_CHECK_INTERVAL)
                .unwrap_or_else(Instant::now);

            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(event)) if is_create_or_modify(&event.kind) => {
                        if !watching_database
                            || event.paths.iter().any(|path| is_database_file(path))
                        {
                            dirty = true;
                        }
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => tracing::warn!(error = %err, "Marvis watcher error"),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }

                if !watching_database && last_setup_check.elapsed() >= SETUP_CHECK_INTERVAL {
                    last_setup_check = Instant::now();
                    if database_dir.is_dir() {
                        let mut guard = watcher.lock().expect("Marvis watcher");
                        if guard
                            .watch(&database_dir, RecursiveMode::NonRecursive)
                            .is_ok()
                        {
                            let _ = guard.unwatch(&watched_path);
                            watched_path = database_dir.clone();
                            watching_database = true;
                            dirty = true;
                        }
                    }
                }

                if dirty && database.is_file() && last_sync.elapsed() >= SYNC_DEBOUNCE {
                    match sync_database(&handle, &database, &mut cursor) {
                        Ok(()) => dirty = false,
                        Err(err) => {
                            tracing::debug!(error = %err, "Marvis database read deferred");
                        }
                    }
                    last_sync = Instant::now();
                }
            }
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn sync_database(
    app: &AppHandle,
    database: &Path,
    cursor: &mut MonitorCursor,
) -> Result<(), String> {
    let conn = open_read_only(database)?;
    ensure_schema(&conn)?;

    let (events, usage) = if cursor.initialized {
        let current_max: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(rowid), 0) FROM agui_events",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        if current_max < cursor.last_event_rowid {
            *cursor = MonitorCursor::default();
            bootstrap(&conn, cursor)?
        } else {
            incremental(&conn, cursor)?
        }
    } else {
        bootstrap(&conn, cursor)?
    };

    apply_updates(app, events, usage);
    Ok(())
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(|err| err.to_string())?;
    conn.busy_timeout(Duration::from_millis(750))
        .map_err(|err| err.to_string())?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    for table in ["agui_events", "messages", "llm_token_usage"] {
        let found: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        if !found {
            return Err(format!("Marvis 数据库缺少 {table} 表"));
        }
    }
    Ok(())
}

fn bootstrap(
    conn: &Connection,
    cursor: &mut MonitorCursor,
) -> Result<(Vec<PendingLifecycle>, Vec<UsageRecord>), String> {
    cursor.last_event_rowid = conn
        .query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM agui_events",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    cursor.approval_statuses = read_approval_states(conn)?
        .into_iter()
        .map(|approval| (approval.approval_id, approval.status))
        .collect();

    let mut lifecycle = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT started.event_id, started.conversation_id, started.response_id,
                    started.timestamp
             FROM agui_events AS started
             WHERE started.event_type = 'RUN_STARTED'
               AND started.seq = (
                    SELECT MAX(candidate.seq)
                    FROM agui_events AS candidate
                    WHERE candidate.response_id = started.response_id
                      AND candidate.event_type = 'RUN_STARTED'
               )
               AND NOT EXISTS (
                    SELECT 1
                    FROM agui_events AS terminal
                    WHERE terminal.response_id = started.response_id
                      AND terminal.event_type IN ('RUN_FINISHED', 'RUN_ERROR')
                      AND terminal.seq > started.seq
               )
             ORDER BY started.timestamp ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|err| err.to_string())?;
    for row in rows {
        let (event_id, conversation_id, response_id, timestamp) =
            row.map_err(|err| err.to_string())?;
        let timestamp = occurred_at_rfc3339(&timestamp);
        let title =
            title_for_response(conn, &response_id).unwrap_or_else(|| "Marvis 任务".to_string());
        cursor.active.insert(
            response_id.clone(),
            ActiveRun {
                conversation_id: conversation_id.clone(),
                title: title.clone(),
                started_at: timestamp.clone(),
                progress_seen: false,
            },
        );
        lifecycle.push(PendingLifecycle {
            vendor_event_id: event_id,
            task_id: response_id,
            session_id: conversation_id,
            event_type: TaskEventType::Started,
            title,
            summary: None,
            occurred_at: timestamp,
        });
    }

    let usage = read_usage_records(conn, cursor)?;
    cursor.initialized = true;
    Ok((lifecycle, usage))
}

fn incremental(
    conn: &Connection,
    cursor: &mut MonitorCursor,
) -> Result<(Vec<PendingLifecycle>, Vec<UsageRecord>), String> {
    let max_rowid: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM agui_events",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    let mut events = read_vendor_events(conn, cursor.last_event_rowid, max_rowid)?;
    events.sort_by_key(|event| event.rowid);
    let mut lifecycle = Vec::new();

    for event in events {
        match event.event_type.as_str() {
            "HUMAN_MESSAGE" => {
                if let Some(title) = title_from_event_data(&event.data) {
                    cursor.titles.insert(event.response_id, title);
                }
            }
            "RUN_STARTED" => {
                let title = cursor
                    .titles
                    .remove(&event.response_id)
                    .or_else(|| title_for_response(conn, &event.response_id))
                    .unwrap_or_else(|| "Marvis 任务".to_string());
                cursor.active.insert(
                    event.response_id.clone(),
                    ActiveRun {
                        conversation_id: event.conversation_id.clone(),
                        title: title.clone(),
                        started_at: event.timestamp.clone(),
                        progress_seen: false,
                    },
                );
                lifecycle.push(PendingLifecycle {
                    vendor_event_id: event.event_id,
                    task_id: event.response_id,
                    session_id: event.conversation_id,
                    event_type: TaskEventType::Started,
                    title,
                    summary: None,
                    occurred_at: event.timestamp,
                });
            }
            "REASONING_START" | "TEXT_MESSAGE_START" | "TOOL_CALL_START" => {
                let Some(active) = cursor.active.get_mut(&event.response_id) else {
                    continue;
                };
                if active.progress_seen {
                    continue;
                }
                active.progress_seen = true;
                lifecycle.push(PendingLifecycle {
                    vendor_event_id: event.event_id,
                    task_id: event.response_id,
                    session_id: active.conversation_id.clone(),
                    event_type: TaskEventType::Progress,
                    title: active.title.clone(),
                    summary: None,
                    occurred_at: event.timestamp,
                });
            }
            "RUN_FINISHED" => {
                let active = cursor.active.remove(&event.response_id);
                let title = active
                    .as_ref()
                    .map(|run| run.title.clone())
                    .or_else(|| title_for_response(conn, &event.response_id))
                    .unwrap_or_else(|| "Marvis 任务".to_string());
                let session_id = active
                    .map(|run| run.conversation_id)
                    .unwrap_or(event.conversation_id);
                lifecycle.push(PendingLifecycle {
                    vendor_event_id: event.event_id,
                    task_id: event.response_id.clone(),
                    session_id,
                    event_type: TaskEventType::Completed,
                    title,
                    summary: final_summary(conn, &event.response_id),
                    occurred_at: event.timestamp,
                });
            }
            "RUN_ERROR" => {
                let active = cursor.active.remove(&event.response_id);
                let title = active
                    .as_ref()
                    .map(|run| run.title.clone())
                    .or_else(|| title_for_response(conn, &event.response_id))
                    .unwrap_or_else(|| "Marvis 任务".to_string());
                let session_id = active
                    .map(|run| run.conversation_id)
                    .unwrap_or(event.conversation_id);
                let (event_type, summary) = error_outcome(&event.data);
                lifecycle.push(PendingLifecycle {
                    vendor_event_id: event.event_id,
                    task_id: event.response_id,
                    session_id,
                    event_type,
                    title,
                    summary,
                    occurred_at: event.timestamp,
                });
            }
            _ => {}
        }
    }
    cursor.last_event_rowid = max_rowid;
    lifecycle.extend(process_approvals(conn, cursor)?);
    let usage = read_usage_records(conn, cursor)?;
    Ok((lifecycle, usage))
}

fn read_vendor_events(
    conn: &Connection,
    after_rowid: i64,
    max_rowid: i64,
) -> Result<Vec<VendorEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT rowid, event_id, conversation_id, response_id, event_type, data, timestamp
             FROM agui_events
             WHERE rowid > ?1 AND rowid <= ?2
               AND event_type IN (
                    'HUMAN_MESSAGE', 'RUN_STARTED', 'RUN_FINISHED', 'RUN_ERROR',
                    'REASONING_START', 'TEXT_MESSAGE_START', 'TOOL_CALL_START'
               )
             ORDER BY rowid ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![after_rowid, max_rowid], |row| {
            Ok(VendorEvent {
                rowid: row.get(0)?,
                event_id: row.get(1)?,
                conversation_id: row.get(2)?,
                response_id: row.get(3)?,
                event_type: row.get(4)?,
                data: row.get(5)?,
                timestamp: occurred_at_rfc3339(&row.get::<_, String>(6)?),
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn title_for_response(conn: &Connection, response_id: &str) -> Option<String> {
    let data: String = conn
        .query_row(
            "SELECT data FROM agui_events
             WHERE response_id = ?1 AND event_type = 'HUMAN_MESSAGE'
             ORDER BY seq ASC LIMIT 1",
            [response_id],
            |row| row.get(0),
        )
        .optional()
        .ok()??;
    title_from_event_data(&data)
}

fn title_from_event_data(data: &str) -> Option<String> {
    let value: Value = serde_json::from_str(data).ok()?;
    compact_text(value.get("content")?.as_str()?, TITLE_LIMIT)
}

fn final_summary(conn: &Connection, response_id: &str) -> Option<String> {
    let content: String = conn
        .query_row(
            "SELECT content FROM messages
             WHERE response_id = ?1 AND role = 'assistant'
               AND content IS NOT NULL AND TRIM(content) != ''
             ORDER BY created_at DESC, message_seq DESC LIMIT 1",
            [response_id],
            |row| row.get(0),
        )
        .optional()
        .ok()??;
    let decoded = serde_json::from_str::<Value>(&content)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or(content);
    compact_text(&decoded, SUMMARY_LIMIT)
}

fn error_outcome(data: &str) -> (TaskEventType, Option<String>) {
    let value: Value = serde_json::from_str(data).unwrap_or(Value::Null);
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        code.as_str(),
        "cancelled" | "canceled" | "aborted" | "stopped"
    ) {
        return (TaskEventType::Cancelled, None);
    }
    let summary = value
        .get("message")
        .and_then(Value::as_str)
        .and_then(|message| compact_text(message, SUMMARY_LIMIT));
    (TaskEventType::Failed, summary)
}

fn process_approvals(
    conn: &Connection,
    cursor: &mut MonitorCursor,
) -> Result<Vec<PendingLifecycle>, String> {
    let approvals = read_approval_states(conn)?;
    let mut lifecycle = Vec::new();
    for approval in approvals {
        let previous = cursor.approval_statuses.get(&approval.approval_id).cloned();
        if previous
            .as_ref()
            .is_some_and(|status| status == &approval.status)
        {
            continue;
        }
        cursor
            .approval_statuses
            .insert(approval.approval_id.clone(), approval.status.clone());
        let Some((response_id, active)) =
            latest_active_for_conversation(&cursor.active, &approval.conversation_id)
        else {
            continue;
        };
        let pending = approval.status.eq_ignore_ascii_case("pending");
        let was_pending = previous.is_some_and(|status| status.eq_ignore_ascii_case("pending"));
        if !pending && !was_pending {
            continue;
        }
        lifecycle.push(PendingLifecycle {
            vendor_event_id: format!("approval:{}:{}", approval.approval_id, approval.status),
            task_id: response_id.to_string(),
            session_id: active.conversation_id.clone(),
            event_type: if pending {
                TaskEventType::Waiting
            } else {
                TaskEventType::Progress
            },
            title: active.title.clone(),
            summary: pending.then(|| "等待你在 Marvis 中确认".to_string()),
            occurred_at: approval.occurred_at,
        });
    }
    Ok(lifecycle)
}

fn read_approval_states(conn: &Connection) -> Result<Vec<ApprovalState>, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'approvals')",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT approval_id, conversation_id, status,
                    COALESCE(decided_at, created_at)
             FROM approvals ORDER BY created_at ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ApprovalState {
                approval_id: row.get(0)?,
                conversation_id: row.get(1)?,
                status: row.get(2)?,
                occurred_at: occurred_at_rfc3339(&row.get::<_, String>(3)?),
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn latest_active_for_conversation<'a>(
    active: &'a HashMap<String, ActiveRun>,
    conversation_id: &str,
) -> Option<(&'a str, &'a ActiveRun)> {
    active
        .iter()
        .filter(|(_, run)| run.conversation_id == conversation_id)
        .max_by(|left, right| left.1.started_at.cmp(&right.1.started_at))
        .map(|(response_id, run)| (response_id.as_str(), run))
}

fn read_usage_records(
    conn: &Connection,
    cursor: &mut MonitorCursor,
) -> Result<Vec<UsageRecord>, String> {
    let max_id: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(id), 0) FROM llm_token_usage",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    if max_id < cursor.last_usage_id {
        cursor.last_usage_id = 0;
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, usage_date, conversation_id, model_id,
                    input_tokens, output_tokens, thinking_tokens, cached_tokens,
                    total_tokens, created_at
             FROM llm_token_usage
             WHERE id > ?1 AND id <= ?2
             ORDER BY id ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![cursor.last_usage_id, max_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|err| err.to_string())?;
    let mut usage = Vec::new();
    for row in rows {
        let (
            id,
            usage_date,
            conversation_id,
            model,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            cached_input_tokens,
            total_tokens,
            occurred_at,
        ) = row.map_err(|err| err.to_string())?;
        if NaiveDate::parse_from_str(&usage_date, "%Y-%m-%d").is_err() || total_tokens <= 0 {
            continue;
        }
        let external_event_id = format!("marvis:{conversation_id}:{id}");
        usage.push(UsageRecord {
            id: external_event_id.clone(),
            source: TaskSource::Marvis,
            external_event_id,
            session_id: Some(conversation_id),
            model: (!model.trim().is_empty()).then_some(model),
            occurred_at: occurred_at_rfc3339(&occurred_at),
            local_date: usage_date,
            input_tokens: input_tokens.max(0),
            cached_input_tokens: cached_input_tokens.max(0),
            output_tokens: output_tokens.max(0),
            reasoning_tokens: reasoning_tokens.max(0),
            total_tokens: total_tokens.max(0),
            collection_method: "marvis-sqlite".into(),
            accuracy: "exact".into(),
        });
    }
    cursor.last_usage_id = max_id;
    Ok(usage)
}

fn apply_updates(app: &AppHandle, lifecycle: Vec<PendingLifecycle>, usage: Vec<UsageRecord>) {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return;
    };
    let lifecycle_enabled = marvis_adapter_enabled(app);
    let mut tasks_changed = false;
    let mut usage_changed = false;
    let db = collector.db.lock().expect("db");

    if lifecycle_enabled {
        for pending in lifecycle {
            let deep_link = conversation_deep_link(&pending.session_id);
            let event = TaskEvent {
                schema_version: 1,
                event_id: format!("marvis-monitor:{}", pending.vendor_event_id),
                source: TaskSource::Marvis,
                event_type: pending.event_type,
                task_id: pending.task_id,
                session_id: Some(pending.session_id),
                parent_task_id: None,
                project_name: None,
                workspace_path: None,
                title: Some(pending.title),
                summary: pending.summary,
                occurred_at: pending.occurred_at,
                deep_link,
                raw: None,
            };
            match db.apply_event(&event) {
                Ok(Some(_)) => tasks_changed = true,
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(error = %err, event_id = %event.event_id, "Marvis event insert failed")
                }
            }
        }
    }

    for record in usage {
        match db.insert_usage(&record) {
            Ok(true) => usage_changed = true,
            Ok(false) => {}
            Err(err) => tracing::warn!(error = %err, "Marvis usage insert failed"),
        }
    }
    drop(db);

    if tasks_changed {
        event_collector::emit_tasks(app);
    }
    if usage_changed {
        let _ = app.emit("usage-updated", ());
    }
}

fn marvis_adapter_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<PersistedSettings>>()
        .map(|settings| settings.lock().expect("settings").app.adapters.marvis)
        .unwrap_or(false)
}

fn compact_text(value: &str, limit: usize) -> Option<String> {
    let mut output = String::new();
    let mut previous_space = false;
    for ch in value.chars() {
        if ch.is_control() || ch.is_whitespace() {
            if !previous_space && !output.is_empty() {
                output.push(' ');
            }
            previous_space = true;
        } else {
            output.push(ch);
            previous_space = false;
        }
        if output.chars().count() >= limit {
            break;
        }
    }
    let trimmed = output.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_database_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| matches!(name, "data.db" | "data.db-wal" | "data.db-shm"))
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE agui_events (
                event_id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                response_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                data TEXT NOT NULL,
                metadata TEXT DEFAULT '{}',
                timestamp TEXT NOT NULL
            );
            CREATE TABLE messages (
                message_id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                response_id TEXT,
                role TEXT NOT NULL,
                content TEXT,
                message_seq INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE approvals (
                approval_id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                created_at TEXT NOT NULL,
                decided_at TEXT
            );
            CREATE TABLE llm_token_usage (
                id INTEGER PRIMARY KEY,
                usage_date TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                thinking_tokens INTEGER NOT NULL,
                cached_tokens INTEGER NOT NULL,
                total_tokens INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_event(
        conn: &Connection,
        event_id: &str,
        seq: i64,
        event_type: &str,
        data: Value,
        timestamp: &str,
    ) {
        conn.execute(
            "INSERT INTO agui_events (
                event_id, conversation_id, response_id, seq, event_type, data, timestamp
             ) VALUES (?1, 'conv-1', 'resp-1', ?2, ?3, ?4, ?5)",
            params![event_id, seq, event_type, data.to_string(), timestamp],
        )
        .unwrap();
    }

    #[test]
    fn maps_cancelled_run_error_separately_from_failure() {
        assert_eq!(
            error_outcome(r#"{"code":"cancelled","message":"user cancelled"}"#).0,
            TaskEventType::Cancelled
        );
        let failed = error_outcome(r#"{"code":"agent_error","message":"model unavailable"}"#);
        assert_eq!(failed.0, TaskEventType::Failed);
        assert_eq!(failed.1.as_deref(), Some("model unavailable"));
    }

    #[test]
    fn builds_only_safe_marvis_conversation_routes() {
        assert_eq!(
            conversation_deep_link("conv_1a014129c66_dc0c8f7d85f9").as_deref(),
            Some(
                "marvis://client/gotoRoute?feature=springcat&url=%2Fchat%2Fconv_1a014129c66_dc0c8f7d85f9"
            )
        );
        assert_eq!(conversation_deep_link("../settings"), None);
        assert_eq!(conversation_deep_link("conversation&id=other"), None);
    }

    #[test]
    fn extracts_only_a_bounded_compact_user_title() {
        let data = serde_json::json!({
            "content": format!("  整理桌面\n{}", "x".repeat(100))
        })
        .to_string();
        let title = title_from_event_data(&data).unwrap();
        assert!(title.starts_with("整理桌面 "));
        assert_eq!(title.chars().count(), TITLE_LIMIT);
    }

    #[test]
    fn reads_exact_usage_without_message_content() {
        let conn = fixture_connection();
        conn.execute_batch(
            "INSERT INTO llm_token_usage VALUES (
                7, '2026-08-18', 'conv-1', 'main-agent',
                100, 20, 5, 40, 120, '2026-08-18T09:00:00.000000'
            );",
        )
        .unwrap();
        let mut cursor = MonitorCursor::default();
        let records = read_usage_records(&conn, &mut cursor).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source, TaskSource::Marvis);
        assert_eq!(records[0].cached_input_tokens, 40);
        assert_eq!(records[0].reasoning_tokens, 5);
        assert_eq!(records[0].total_tokens, 120);
        assert!(
            chrono::DateTime::parse_from_rfc3339(&records[0].occurred_at).is_ok(),
            "Marvis usage timestamps must be UTC RFC3339, got {}",
            records[0].occurred_at
        );
        assert_eq!(cursor.last_usage_id, 7);
    }

    #[test]
    fn bootstraps_active_run_then_reads_progress_and_completion() {
        let conn = fixture_connection();
        insert_event(
            &conn,
            "human-1",
            1,
            "HUMAN_MESSAGE",
            serde_json::json!({"content":"整理桌面文件"}),
            "2026-08-18T09:00:00.000000",
        );
        insert_event(
            &conn,
            "start-1",
            2,
            "RUN_STARTED",
            serde_json::json!({}),
            "2026-08-18T09:00:01.000000",
        );

        let mut cursor = MonitorCursor::default();
        let (started, _) = bootstrap(&conn, &mut cursor).unwrap();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].event_type, TaskEventType::Started);
        assert_eq!(started[0].title, "整理桌面文件");
        assert!(
            chrono::DateTime::parse_from_rfc3339(&started[0].occurred_at).is_ok(),
            "Marvis timestamps must be UTC RFC3339, got {}",
            started[0].occurred_at
        );
        assert!(started[0].occurred_at.ends_with('Z'));

        insert_event(
            &conn,
            "reasoning-1",
            3,
            "REASONING_START",
            serde_json::json!({"private_reasoning":"must never be selected"}),
            "2026-08-18T09:00:02.000000",
        );
        conn.execute(
            "INSERT INTO messages (
                message_id, conversation_id, response_id, role, content, message_seq, created_at
             ) VALUES ('assistant-1', 'conv-1', 'resp-1', 'assistant',
                       '桌面文件已经整理完成', 4, '2026-08-18T09:00:03.000000')",
            [],
        )
        .unwrap();
        insert_event(
            &conn,
            "finish-1",
            5,
            "RUN_FINISHED",
            serde_json::json!({}),
            "2026-08-18T09:00:04.000000",
        );

        let (completed, _) = incremental(&conn, &mut cursor).unwrap();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].event_type, TaskEventType::Progress);
        assert_eq!(completed[1].event_type, TaskEventType::Completed);
        assert_eq!(
            completed[1].summary.as_deref(),
            Some("桌面文件已经整理完成")
        );
        assert!(cursor.active.is_empty());
    }

    #[test]
    fn maps_pending_and_decided_approvals_to_waiting_then_progress() {
        let conn = fixture_connection();
        insert_event(
            &conn,
            "human-1",
            1,
            "HUMAN_MESSAGE",
            serde_json::json!({"content":"修改系统设置"}),
            "2026-08-18T09:00:00.000000",
        );
        insert_event(
            &conn,
            "start-1",
            2,
            "RUN_STARTED",
            serde_json::json!({}),
            "2026-08-18T09:00:01.000000",
        );
        let mut cursor = MonitorCursor::default();
        bootstrap(&conn, &mut cursor).unwrap();

        conn.execute(
            "INSERT INTO approvals (approval_id, conversation_id, status, created_at)
             VALUES ('approval-1', 'conv-1', 'pending', '2026-08-18T09:00:02.000000')",
            [],
        )
        .unwrap();
        let (waiting, _) = incremental(&conn, &mut cursor).unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].event_type, TaskEventType::Waiting);

        conn.execute(
            "UPDATE approvals SET status = 'approved', decided_at = '2026-08-18T09:00:03.000000'
             WHERE approval_id = 'approval-1'",
            [],
        )
        .unwrap();
        let (resumed, _) = incremental(&conn, &mut cursor).unwrap();
        assert_eq!(resumed.len(), 1);
        assert_eq!(resumed[0].event_type, TaskEventType::Progress);
    }

    #[test]
    #[ignore = "requires a local Marvis installation"]
    fn live_marvis_database_is_read_only_compatible() {
        let Some(path) = database_path().filter(|path| path.is_file()) else {
            return;
        };
        let conn = open_read_only(&path).unwrap();
        ensure_schema(&conn).unwrap();
        let mut cursor = MonitorCursor::default();
        let _ = bootstrap(&conn, &mut cursor).unwrap();
        assert!(cursor.initialized);
        let response_id = conn
            .query_row(
                "SELECT response_id FROM agui_events
                 WHERE response_id IS NOT NULL AND response_id != ''
                 ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .unwrap();
        if let Some(response_id) = response_id {
            assert!(conversation_link_for_response(&response_id).is_some());
        }
    }
}
