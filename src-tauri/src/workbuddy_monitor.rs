//! Passive WorkBuddy lifecycle monitor.
//!
//! WorkBuddy stores append-only conversation JSONL under
//! `~/.workbuddy/projects`. SpringCat reads only structural lifecycle fields,
//! the current user query as a short title, and the final assistant text as a
//! bounded summary. Full transcripts, reasoning, tool arguments, and tool
//! outputs never reach SpringCat's database.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use chrono::TimeZone;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::domain::{TaskEvent, TaskEventType, TaskSource};
use crate::event_collector::{self, CollectorState};
use crate::settings_store::PersistedSettings;

const COMPLETION_QUIET_PERIOD: Duration = Duration::from_millis(1_500);
const BOOTSTRAP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub struct WorkBuddyMonitorState {
    _watcher: Mutex<RecommendedWatcher>,
}

#[derive(Debug, Clone)]
struct ActiveTurn {
    id: String,
    occurred_at: String,
    title: String,
    progress_seen: bool,
}

#[derive(Debug, Clone)]
struct CompletionCandidate {
    id: String,
    occurred_at: String,
    summary: Option<String>,
    observed_at: Instant,
}

#[derive(Debug, Default)]
struct FileCursor {
    offset: u64,
    session_id: Option<String>,
    workspace: Option<String>,
    active_turn: Option<ActiveTurn>,
    completion: Option<CompletionCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorEventKind {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

impl MonitorEventKind {
    fn task_event_type(self) -> TaskEventType {
        match self {
            Self::Started => TaskEventType::Started,
            Self::Progress => TaskEventType::Progress,
            Self::Completed => TaskEventType::Completed,
            Self::Failed => TaskEventType::Failed,
            Self::Cancelled => TaskEventType::Cancelled,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Progress => "progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone)]
struct PendingDelivery {
    kind: MonitorEventKind,
    row_id: String,
    occurred_at: String,
    turn_id: String,
    title: String,
    summary: Option<String>,
}

#[derive(Debug)]
enum WorkBuddyRow {
    Started {
        row_id: String,
        occurred_at: String,
        session_id: Option<String>,
        workspace: Option<String>,
        title: String,
    },
    Progress {
        row_id: String,
        occurred_at: String,
        session_id: Option<String>,
        workspace: Option<String>,
    },
    Assistant {
        row_id: String,
        occurred_at: String,
        session_id: Option<String>,
        workspace: Option<String>,
        status: String,
        summary: Option<String>,
    },
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let Some(root) = projects_dir() else {
        return Ok(());
    };
    if !root.is_dir() {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .map_err(|err| err.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|err| err.to_string())?;
    app.manage(WorkBuddyMonitorState {
        _watcher: Mutex::new(watcher),
    });

    let cursors = Arc::new(Mutex::new(HashMap::<PathBuf, FileCursor>::new()));
    bootstrap_recent(app, &root, &cursors);

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-workbuddy-monitor".into())
        .spawn(move || loop {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(event)) if is_create_or_modify(&event.kind) => {
                    for path in event.paths {
                        if is_session_jsonl(&path) {
                            process_incremental(&handle, &cursors, &path);
                        }
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(err)) => tracing::warn!(error = %err, "WorkBuddy watcher error"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            flush_completions(&handle, &cursors);
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

pub fn projects_dir() -> Option<PathBuf> {
    std::env::var_os("WORKBUDDY_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".workbuddy")))
        .map(|home| home.join("projects"))
}

fn bootstrap_recent(
    app: &AppHandle,
    root: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, FileCursor>>>,
) {
    for path in session_files(root) {
        let recent = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age <= BOOTSTRAP_MAX_AGE);

        let mut cursor = FileCursor::default();
        if let Err(err) = read_appended(&path, &mut cursor, false) {
            tracing::debug!(error = %err, file = %path.display(), "WorkBuddy bootstrap read skipped");
            continue;
        }

        // A stable assistant message at EOF represents an already-finished
        // historical turn. Do not replay it as a fresh notification on start.
        if cursor.completion.is_some() {
            cursor.completion = None;
            cursor.active_turn = None;
        } else if recent {
            if let Some(active) = cursor.active_turn.as_ref() {
                deliver(
                    app,
                    &cursor,
                    PendingDelivery {
                        kind: MonitorEventKind::Started,
                        row_id: format!("{}-bootstrap", active.id),
                        occurred_at: active.occurred_at.clone(),
                        turn_id: active.id.clone(),
                        title: active.title.clone(),
                        summary: None,
                    },
                );
            }
        }

        cursors
            .lock()
            .expect("WorkBuddy cursors")
            .insert(path, cursor);
    }
}

fn session_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(projects) = fs::read_dir(root) else {
        return files;
    };
    for project in projects.flatten() {
        let path = project.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        files.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| is_session_jsonl(path)),
        );
    }
    files
}

fn process_incremental(
    app: &AppHandle,
    cursors: &Arc<Mutex<HashMap<PathBuf, FileCursor>>>,
    path: &Path,
) {
    let mut cursor = {
        let mut guard = cursors.lock().expect("WorkBuddy cursors");
        guard.remove(path).unwrap_or_default()
    };

    let deliveries = match read_appended(path, &mut cursor, true) {
        Ok(deliveries) => deliveries,
        Err(err) => {
            tracing::debug!(error = %err, file = %path.display(), "WorkBuddy JSONL read deferred");
            cursors
                .lock()
                .expect("WorkBuddy cursors")
                .insert(path.to_path_buf(), cursor);
            return;
        }
    };

    for event in deliveries {
        deliver(app, &cursor, event);
    }
    cursors
        .lock()
        .expect("WorkBuddy cursors")
        .insert(path.to_path_buf(), cursor);
}

fn read_appended(
    path: &Path,
    cursor: &mut FileCursor,
    collect_deliveries: bool,
) -> Result<Vec<PendingDelivery>, String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let length = file.metadata().map_err(|err| err.to_string())?.len();
    if length < cursor.offset {
        *cursor = FileCursor::default();
    }
    file.seek(SeekFrom::Start(cursor.offset))
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let mut deliveries = Vec::new();

    loop {
        let line_start = reader.stream_position().map_err(|err| err.to_string())?;
        let mut line = String::new();
        let read = reader.read_line(&mut line).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        if !line.ends_with('\n') {
            cursor.offset = line_start;
            break;
        }
        cursor.offset = reader.stream_position().map_err(|err| err.to_string())?;
        let Some(row) = parse_row(&line) else {
            continue;
        };
        apply_row(cursor, row, collect_deliveries, &mut deliveries);
    }
    Ok(deliveries)
}

fn apply_row(
    cursor: &mut FileCursor,
    row: WorkBuddyRow,
    collect_deliveries: bool,
    deliveries: &mut Vec<PendingDelivery>,
) {
    match row {
        WorkBuddyRow::Started {
            row_id,
            occurred_at,
            session_id,
            workspace,
            title,
        } => {
            update_metadata(cursor, session_id, workspace);
            cursor.completion = None;
            cursor.active_turn = Some(ActiveTurn {
                id: row_id.clone(),
                occurred_at: occurred_at.clone(),
                title: title.clone(),
                progress_seen: false,
            });
            if collect_deliveries {
                deliveries.push(PendingDelivery {
                    kind: MonitorEventKind::Started,
                    row_id: row_id.clone(),
                    occurred_at,
                    turn_id: row_id,
                    title,
                    summary: None,
                });
            }
        }
        WorkBuddyRow::Progress {
            row_id,
            occurred_at,
            session_id,
            workspace,
        } => {
            update_metadata(cursor, session_id, workspace);
            cursor.completion = None;
            let Some(active) = cursor.active_turn.as_mut() else {
                return;
            };
            if !active.progress_seen {
                active.progress_seen = true;
                if collect_deliveries {
                    deliveries.push(PendingDelivery {
                        kind: MonitorEventKind::Progress,
                        row_id,
                        occurred_at,
                        turn_id: active.id.clone(),
                        title: active.title.clone(),
                        summary: None,
                    });
                }
            }
        }
        WorkBuddyRow::Assistant {
            row_id,
            occurred_at,
            session_id,
            workspace,
            status,
            summary,
        } => {
            update_metadata(cursor, session_id, workspace);
            let Some(active) = cursor.active_turn.as_ref() else {
                return;
            };
            match status.as_str() {
                "failed" | "error" | "errored" => {
                    if collect_deliveries {
                        deliveries.push(PendingDelivery {
                            kind: MonitorEventKind::Failed,
                            row_id,
                            occurred_at,
                            turn_id: active.id.clone(),
                            title: active.title.clone(),
                            summary,
                        });
                    }
                    cursor.active_turn = None;
                    cursor.completion = None;
                }
                "cancelled" | "canceled" | "aborted" => {
                    if collect_deliveries {
                        deliveries.push(PendingDelivery {
                            kind: MonitorEventKind::Cancelled,
                            row_id,
                            occurred_at,
                            turn_id: active.id.clone(),
                            title: active.title.clone(),
                            summary,
                        });
                    }
                    cursor.active_turn = None;
                    cursor.completion = None;
                }
                "completed" | "complete" | "success" => {
                    cursor.completion = Some(CompletionCandidate {
                        id: row_id,
                        occurred_at,
                        summary,
                        observed_at: Instant::now(),
                    });
                }
                _ => {
                    cursor.completion = None;
                }
            }
        }
    }
}

fn update_metadata(cursor: &mut FileCursor, session_id: Option<String>, workspace: Option<String>) {
    if session_id.is_some() {
        cursor.session_id = session_id;
    }
    if workspace.is_some() {
        cursor.workspace = workspace;
    }
}

fn flush_completions(app: &AppHandle, cursors: &Arc<Mutex<HashMap<PathBuf, FileCursor>>>) {
    let mut ready = Vec::new();
    {
        let mut guard = cursors.lock().expect("WorkBuddy cursors");
        for cursor in guard.values_mut() {
            let Some(candidate) = cursor.completion.as_ref() else {
                continue;
            };
            if candidate.observed_at.elapsed() < COMPLETION_QUIET_PERIOD {
                continue;
            }
            let Some(active) = cursor.active_turn.as_ref() else {
                cursor.completion = None;
                continue;
            };
            ready.push((
                cursor.session_id.clone(),
                cursor.workspace.clone(),
                PendingDelivery {
                    kind: MonitorEventKind::Completed,
                    row_id: candidate.id.clone(),
                    occurred_at: candidate.occurred_at.clone(),
                    turn_id: active.id.clone(),
                    title: active.title.clone(),
                    summary: candidate.summary.clone(),
                },
            ));
            cursor.completion = None;
            cursor.active_turn = None;
        }
    }

    for (session_id, workspace, delivery) in ready {
        let snapshot = FileCursor {
            session_id,
            workspace,
            ..FileCursor::default()
        };
        deliver(app, &snapshot, delivery);
    }
}

fn parse_row(line: &str) -> Option<WorkBuddyRow> {
    let value: Value = serde_json::from_str(line).ok()?;
    let row_type = value.get("type")?.as_str()?;
    let row_id = value.get("id")?.as_str()?.to_string();
    let occurred_at = timestamp_rfc3339(value.get("timestamp")?)?;
    let session_id = string_field(&value, "sessionId");
    let workspace = string_field(&value, "cwd");

    match row_type {
        "message" if value.get("role").and_then(Value::as_str) == Some("user") => {
            let title = user_title(value.get("content")?)?;
            Some(WorkBuddyRow::Started {
                row_id,
                occurred_at,
                session_id,
                workspace,
                title,
            })
        }
        "message" if value.get("role").and_then(Value::as_str) == Some("assistant") => {
            Some(WorkBuddyRow::Assistant {
                row_id,
                occurred_at,
                session_id,
                workspace,
                status: value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                summary: assistant_summary(value.get("content")),
            })
        }
        "reasoning" | "function_call" | "function_call_result" => Some(WorkBuddyRow::Progress {
            row_id,
            occurred_at,
            session_id,
            workspace,
        }),
        _ => None,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn timestamp_rfc3339(value: &Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        if chrono::DateTime::parse_from_rfc3339(raw).is_ok() {
            return Some(raw.to_string());
        }
        if let Ok(millis) = raw.parse::<i64>() {
            return chrono::Utc
                .timestamp_millis_opt(millis)
                .single()
                .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        }
    }
    value.as_i64().and_then(|millis| {
        chrono::Utc
            .timestamp_millis_opt(millis)
            .single()
            .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
    })
}

fn user_title(content: &Value) -> Option<String> {
    let text = content_text(content, "input_text");
    let query = extract_between(&text, "<user_query>", "</user_query>")
        .map(str::to_string)
        .or_else(|| (!text.contains("<system-reminder")).then_some(text))?;
    compact_text(&query, 80)
}

fn assistant_summary(content: Option<&Value>) -> Option<String> {
    compact_text(&content_text(content?, "output_text"), 160)
}

fn content_text(content: &Value, expected_type: &str) -> String {
    match content {
        Value::Array(items) => items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(expected_type))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) if content.get("type").and_then(Value::as_str) == Some(expected_type) => {
            content
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }
        Value::String(text) => text.clone(),
        _ => String::new(),
    }
}

fn extract_between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = value.find(start)? + start.len();
    let tail = &value[from..];
    let to = tail.find(end)?;
    Some(&tail[..to])
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

fn deliver(app: &AppHandle, cursor: &FileCursor, delivery: PendingDelivery) {
    if !workbuddy_adapter_enabled(app) {
        return;
    }
    let Some(session_id) = cursor.session_id.as_ref() else {
        return;
    };
    let event = TaskEvent {
        schema_version: 1,
        event_id: format!(
            "workbuddy-monitor:{session_id}:{}:{}:{}",
            delivery.turn_id,
            delivery.kind.key(),
            delivery.row_id
        ),
        source: TaskSource::WorkBuddy,
        event_type: delivery.kind.task_event_type(),
        task_id: session_id.clone(),
        session_id: Some(session_id.clone()),
        parent_task_id: None,
        project_name: cursor.workspace.as_deref().and_then(project_name),
        workspace_path: cursor.workspace.clone(),
        title: Some(delivery.title),
        summary: delivery.summary,
        occurred_at: delivery.occurred_at,
        deep_link: None,
        raw: None,
    };

    let Some(collector) = app.try_state::<CollectorState>() else {
        return;
    };
    let applied = match collector.db.lock().expect("db").apply_event(&event) {
        Ok(applied) => applied,
        Err(err) => {
            tracing::warn!(error = %err, event_id = %event.event_id, "WorkBuddy monitor event failed");
            return;
        }
    };
    if applied.is_some() {
        event_collector::emit_tasks(app);
    }
}

fn project_name(workspace: &str) -> Option<String> {
    Path::new(workspace)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
}

fn workbuddy_adapter_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<PersistedSettings>>()
        .map(|settings| settings.lock().expect("settings").app.adapters.work_buddy)
        .unwrap_or(false)
}

fn is_session_jsonl(path: &Path) -> bool {
    path.is_file()
        && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        && path
            .file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok())
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extracts_only_the_real_user_query() {
        let row = parse_row(
            r#"{"id":"user-1","timestamp":1786691784070,"type":"message","role":"user","content":[{"type":"input_text","text":"<system-reminder>private</system-reminder><user_query>优化 WorkBuddy 监听</user_query>"}],"sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760","cwd":"E:\\workspace\\springcat-ai"}"#,
        )
        .unwrap();

        match row {
            WorkBuddyRow::Started { title, .. } => {
                assert_eq!(title, "优化 WorkBuddy 监听");
            }
            _ => panic!("expected start row"),
        }
    }

    #[test]
    fn ignores_internal_user_messages_without_a_query() {
        assert!(parse_row(
            r#"{"id":"user-internal","timestamp":1786691784070,"type":"message","role":"user","content":{"type":"input_text","text":"<system-reminder>compaction</system-reminder>"}}"#,
        )
        .is_none());
    }

    #[test]
    fn a_tool_call_after_assistant_text_cancels_early_completion() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("6d5fa380-7917-4bb0-9dc3-173f651aa760.jsonl");
        let mut file = File::create(&path).unwrap();
        for line in [
            r#"{"id":"user-1","timestamp":1786691784070,"type":"message","role":"user","content":{"type":"input_text","text":"做一个图标"},"sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760","cwd":"E:\\workspace"}"#,
            r#"{"id":"assistant-1","timestamp":1786691785000,"type":"message","role":"assistant","status":"completed","content":{"type":"output_text","text":"我先检查文件"},"sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760"}"#,
            r#"{"id":"tool-1","timestamp":1786691785001,"type":"function_call","name":"Read","sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760"}"#,
        ] {
            writeln!(file, "{line}").unwrap();
        }
        drop(file);

        let mut cursor = FileCursor::default();
        let deliveries = read_appended(&path, &mut cursor, true).unwrap();
        assert!(cursor.active_turn.is_some());
        assert!(cursor.completion.is_none());
        assert_eq!(deliveries[0].kind, MonitorEventKind::Started);
        assert_eq!(deliveries[1].kind, MonitorEventKind::Progress);
    }

    #[test]
    fn final_assistant_text_becomes_a_completion_candidate() {
        let mut cursor = FileCursor::default();
        let mut deliveries = Vec::new();
        apply_row(
            &mut cursor,
            parse_row(
                r#"{"id":"user-1","timestamp":1786691784070,"type":"message","role":"user","content":{"type":"input_text","text":"完成任务"},"sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760"}"#,
            )
            .unwrap(),
            true,
            &mut deliveries,
        );
        apply_row(
            &mut cursor,
            parse_row(
                r#"{"id":"assistant-1","timestamp":1786691785000,"type":"message","role":"assistant","status":"completed","content":{"type":"output_text","text":"任务已经完成"},"sessionId":"6d5fa380-7917-4bb0-9dc3-173f651aa760"}"#,
            )
            .unwrap(),
            true,
            &mut deliveries,
        );

        assert!(cursor.completion.is_some());
        assert_eq!(
            cursor.completion.unwrap().summary.as_deref(),
            Some("任务已经完成")
        );
    }
}
