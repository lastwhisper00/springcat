//! Hook-independent Codex lifecycle monitor.
//!
//! Codex command hooks are the preferred path, but a Codex process that was
//! already running when SpringCat installed `hooks.json` may not load them.
//! This fallback watches Codex's append-only rollout files and reads only
//! structural lifecycle fields plus the thread title from Codex's state DB.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::domain::{TaskEvent, TaskEventType, TaskSource};
use crate::event_collector::{self, CollectorState};
use crate::settings_store::PersistedSettings;

const BOOTSTRAP_LIMIT: i64 = 64;
const BOOTSTRAP_MAX_AGE_HOURS: u128 = 24;

pub struct CodexMonitorState {
    _watcher: Mutex<RecommendedWatcher>,
}

#[derive(Debug, Clone)]
struct ThreadMetadata {
    id: String,
    title: Option<String>,
    workspace: Option<String>,
    rollout_path: PathBuf,
}

#[derive(Debug, Default)]
struct FileCursor {
    offset: u64,
    active_turn: Option<String>,
    metadata: Option<ThreadMetadata>,
    pending_events: Vec<MonitorEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorEventKind {
    Started,
    Progress,
    Completed,
    Cancelled,
}

impl MonitorEventKind {
    fn task_event_type(self) -> TaskEventType {
        match self {
            Self::Started => TaskEventType::Started,
            Self::Progress => TaskEventType::Progress,
            Self::Completed => TaskEventType::Completed,
            Self::Cancelled => TaskEventType::Cancelled,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Progress => "progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone)]
struct MonitorEvent {
    kind: MonitorEventKind,
    occurred_at: String,
    turn_id: Option<String>,
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let Some(codex_home) = codex_home() else {
        return Ok(());
    };
    let watch_roots: Vec<PathBuf> = ["sessions", "archived_sessions"]
        .iter()
        .map(|name| codex_home.join(name))
        .filter(|path| path.is_dir())
        .collect();
    if watch_roots.is_empty() {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .map_err(|err| err.to_string())?;
    for root in &watch_roots {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|err| err.to_string())?;
    }
    app.manage(CodexMonitorState {
        _watcher: Mutex::new(watcher),
    });

    // Start watching before the initial scan so an event written during
    // bootstrap is queued and then replayed from the saved byte offset.
    let cursors = Arc::new(Mutex::new(HashMap::<PathBuf, FileCursor>::new()));
    bootstrap_recent(app, &codex_home, &cursors);

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-codex-monitor".into())
        .spawn(move || {
            for message in rx {
                match message {
                    Ok(event) if is_create_or_modify(&event.kind) => {
                        for path in event.paths {
                            if is_rollout(&path) {
                                process_incremental(&handle, &codex_home, &cursors, &path);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(err) => tracing::warn!(error = %err, "Codex rollout watcher error"),
                }
            }
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn bootstrap_recent(
    app: &AppHandle,
    codex_home: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, FileCursor>>>,
) {
    let cutoff = millis().saturating_sub(BOOTSTRAP_MAX_AGE_HOURS * 60 * 60 * 1000);
    let Ok(threads) = recent_threads(codex_home, cutoff, BOOTSTRAP_LIMIT) else {
        return;
    };
    for metadata in threads {
        let path = metadata.rollout_path.clone();
        let Ok((offset, active_turn, snapshot)) = scan_snapshot(&path) else {
            continue;
        };
        for event in snapshot {
            deliver(app, &metadata, event);
        }
        cursors.lock().expect("Codex cursors").insert(
            path,
            FileCursor {
                offset,
                active_turn,
                metadata: Some(metadata),
                pending_events: Vec::new(),
            },
        );
    }
}

fn process_incremental(
    app: &AppHandle,
    codex_home: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, FileCursor>>>,
    path: &Path,
) {
    let mut cursor = {
        let mut guard = cursors.lock().expect("Codex cursors");
        guard.remove(path).unwrap_or_default()
    };
    refresh_metadata(codex_home, path, &mut cursor);

    let events = match read_appended(path, &mut cursor) {
        Ok(events) => events,
        Err(err) => {
            tracing::debug!(error = %err, file = %path.display(), "Codex rollout read deferred");
            cursors
                .lock()
                .expect("Codex cursors")
                .insert(path.to_path_buf(), cursor);
            return;
        }
    };
    cursor.pending_events.extend(events);

    refresh_metadata(codex_home, path, &mut cursor);
    if let Some(metadata) = cursor.metadata.as_ref() {
        for event in cursor.pending_events.drain(..) {
            deliver(app, metadata, event);
        }
    }
    cursors
        .lock()
        .expect("Codex cursors")
        .insert(path.to_path_buf(), cursor);
}

fn refresh_metadata(codex_home: &Path, path: &Path, cursor: &mut FileCursor) {
    if cursor
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.title.is_some())
    {
        return;
    }
    let Some(thread_id) = thread_id_from_path(path) else {
        return;
    };
    if let Ok(Some(metadata)) = thread_metadata(codex_home, &thread_id) {
        cursor.metadata = Some(metadata);
    } else if cursor.metadata.is_none() {
        cursor.metadata = metadata_from_rollout(path);
    }
}

fn read_appended(path: &Path, cursor: &mut FileCursor) -> Result<Vec<MonitorEvent>, String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let length = file.metadata().map_err(|err| err.to_string())?.len();
    if length < cursor.offset {
        cursor.offset = 0;
        cursor.active_turn = None;
        cursor.pending_events.clear();
    }
    file.seek(SeekFrom::Start(cursor.offset))
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let mut events = Vec::new();

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
        if let Some(event) = parse_monitor_event(&line, cursor.active_turn.as_deref()) {
            match event.kind {
                MonitorEventKind::Started => cursor.active_turn = event.turn_id.clone(),
                MonitorEventKind::Completed | MonitorEventKind::Cancelled => {
                    cursor.active_turn = None;
                }
                MonitorEventKind::Progress => {}
            }
            events.push(event);
        }
    }
    Ok(events)
}

fn scan_snapshot(path: &Path) -> Result<(u64, Option<String>, Vec<MonitorEvent>), String> {
    let mut cursor = FileCursor::default();
    let events = read_appended(path, &mut cursor)?;
    let mut last_started: Option<MonitorEvent> = None;
    let mut last_progress: Option<MonitorEvent> = None;
    let mut last_terminal: Option<MonitorEvent> = None;

    for event in events {
        match event.kind {
            MonitorEventKind::Started => {
                last_started = Some(event);
                last_progress = None;
                last_terminal = None;
            }
            MonitorEventKind::Progress if last_started.is_some() => last_progress = Some(event),
            MonitorEventKind::Completed | MonitorEventKind::Cancelled if last_started.is_some() => {
                last_terminal = Some(event)
            }
            _ => {}
        }
    }

    let mut snapshot = Vec::new();
    if let Some(started) = last_started {
        snapshot.push(started);
        if let Some(progress) = last_progress {
            snapshot.push(progress);
        }
        if let Some(terminal) = last_terminal {
            snapshot.push(terminal);
        }
    }
    Ok((cursor.offset, cursor.active_turn, snapshot))
}

fn parse_monitor_event(line: &str, active_turn: Option<&str>) -> Option<MonitorEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let occurred_at = value.get("timestamp")?.as_str()?.to_string();
    let item_type = value.get("type")?.as_str()?;
    let payload = value.get("payload")?.as_object()?;
    let payload_type = payload.get("type")?.as_str()?;
    let explicit_turn = payload
        .get("turn_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    let kind = match (item_type, payload_type) {
        ("event_msg", "task_started") => MonitorEventKind::Started,
        ("event_msg", "task_complete") => MonitorEventKind::Completed,
        ("event_msg", "turn_aborted") => MonitorEventKind::Cancelled,
        ("response_item", "custom_tool_call_output") if active_turn.is_some() => {
            MonitorEventKind::Progress
        }
        _ => return None,
    };
    Some(MonitorEvent {
        kind,
        occurred_at,
        turn_id: explicit_turn.or_else(|| active_turn.map(str::to_string)),
    })
}

fn deliver(app: &AppHandle, metadata: &ThreadMetadata, event: MonitorEvent) {
    if !codex_adapter_enabled(app) {
        return;
    }
    let turn_key = event.turn_id.as_deref().unwrap_or("unknown-turn");
    let task_event = TaskEvent {
        schema_version: 1,
        event_id: format!(
            "codex-monitor:{}:{turn_key}:{}:{}",
            metadata.id,
            event.kind.key(),
            event.occurred_at
        ),
        source: TaskSource::Codex,
        event_type: event.kind.task_event_type(),
        task_id: metadata.id.clone(),
        session_id: Some(metadata.id.clone()),
        parent_task_id: None,
        project_name: metadata.workspace.as_deref().and_then(project_name),
        workspace_path: metadata.workspace.clone(),
        title: metadata.title.clone(),
        summary: None,
        occurred_at: event.occurred_at,
        deep_link: None,
        raw: None,
    };

    let Some(collector) = app.try_state::<CollectorState>() else {
        return;
    };
    let applied = match collector.db.lock().expect("db").apply_event(&task_event) {
        Ok(applied) => applied,
        Err(err) => {
            tracing::warn!(error = %err, event_id = %task_event.event_id, "Codex monitor event failed");
            return;
        }
    };
    if applied.is_some() {
        event_collector::emit_tasks(app);
    }
}

fn recent_threads(
    codex_home: &Path,
    cutoff_ms: u128,
    limit: i64,
) -> Result<Vec<ThreadMetadata>, String> {
    let Some(path) = state_db_path(codex_home) else {
        return Ok(Vec::new());
    };
    let connection = readonly_connection(&path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, COALESCE(NULLIF(name, ''), NULLIF(title, '')), cwd, rollout_path
             FROM threads
             WHERE thread_source = 'user' AND updated_at_ms >= ?1
             ORDER BY updated_at_ms DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = statement
        .query_map(params![cutoff_ms as i64, limit], |row| {
            Ok(ThreadMetadata {
                id: row.get(0)?,
                title: row
                    .get::<_, Option<String>>(1)?
                    .and_then(|value| compact_title(&value)),
                workspace: row.get::<_, Option<String>>(2)?,
                rollout_path: PathBuf::from(row.get::<_, String>(3)?),
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn thread_metadata(codex_home: &Path, thread_id: &str) -> Result<Option<ThreadMetadata>, String> {
    let Some(path) = state_db_path(codex_home) else {
        return Ok(None);
    };
    let connection = readonly_connection(&path)?;
    connection
        .query_row(
            "SELECT id, COALESCE(NULLIF(name, ''), NULLIF(title, '')), cwd, rollout_path
             FROM threads WHERE id = ?1 AND thread_source = 'user'",
            [thread_id],
            |row| {
                Ok(ThreadMetadata {
                    id: row.get(0)?,
                    title: row
                        .get::<_, Option<String>>(1)?
                        .and_then(|value| compact_title(&value)),
                    workspace: row.get::<_, Option<String>>(2)?,
                    rollout_path: PathBuf::from(row.get::<_, String>(3)?),
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())
}

fn state_db_path(codex_home: &Path) -> Option<PathBuf> {
    fs::read_dir(codex_home)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let version = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            Some((version, path))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, path)| path)
}

fn readonly_connection(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| err.to_string())
}

fn metadata_from_rollout(path: &Path) -> Option<ThreadMetadata> {
    let file = File::open(path).ok()?;
    let mut lines = BufReader::new(file).lines();
    let first_line = lines.next()?.ok()?;
    let value: serde_json::Value = serde_json::from_str(&first_line).ok()?;
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    let payload = value.get("payload")?.as_object()?;
    if payload
        .get("thread_source")
        .and_then(serde_json::Value::as_str)
        != Some("user")
    {
        return None;
    }
    let id = payload
        .get("id")
        .or_else(|| payload.get("session_id"))?
        .as_str()?
        .to_string();
    if thread_id_from_path(path).as_deref() != Some(id.as_str()) {
        return None;
    }
    Some(ThreadMetadata {
        id,
        title: None,
        workspace: payload
            .get("cwd")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        rollout_path: path.to_path_buf(),
    })
}

fn thread_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let id = stem.rsplit('-').collect::<Vec<_>>();
    if id.len() < 5 {
        return None;
    }
    let id = id[..5].iter().rev().copied().collect::<Vec<_>>().join("-");
    (id.len() == 36).then_some(id)
}

fn compact_title(value: &str) -> Option<String> {
    let first_line = value.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() {
        return None;
    }
    Some(first_line.chars().take(80).collect())
}

fn project_name(workspace: &str) -> Option<String> {
    Path::new(workspace)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
}

fn codex_adapter_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<PersistedSettings>>()
        .map(|settings| settings.lock().expect("settings").app.adapters.codex)
        .unwrap_or(false)
}

fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}

fn is_rollout(path: &Path) -> bool {
    path.is_file()
        && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("rollout-"))
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_only_structural_lifecycle_events() {
        let started = parse_monitor_event(
            r#"{"timestamp":"2026-08-14T05:00:00.000Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1","private":"ignored"}}"#,
            None,
        )
        .unwrap();
        let progress = parse_monitor_event(
            r#"{"timestamp":"2026-08-14T05:00:01.000Z","type":"response_item","payload":{"type":"custom_tool_call_output","output":"private"}}"#,
            Some("turn-1"),
        )
        .unwrap();
        let completed = parse_monitor_event(
            r#"{"timestamp":"2026-08-14T05:00:02.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","last_agent_message":"private"}}"#,
            Some("turn-1"),
        )
        .unwrap();

        assert_eq!(started.kind, MonitorEventKind::Started);
        assert_eq!(progress.kind, MonitorEventKind::Progress);
        assert_eq!(completed.kind, MonitorEventKind::Completed);
        assert_eq!(progress.turn_id.as_deref(), Some("turn-1"));
    }

    #[test]
    fn snapshot_keeps_only_the_latest_turn_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("rollout-2026-08-14T05-00-00-019ffeb6-0cb1-7673-8fda-42b6efaf3343.jsonl");
        let mut file = File::create(&path).unwrap();
        for line in [
            r#"{"timestamp":"2026-08-14T05:00:00.000Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}"#,
            r#"{"timestamp":"2026-08-14T05:00:01.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1"}}"#,
            r#"{"timestamp":"2026-08-14T05:01:00.000Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-2"}}"#,
            r#"{"timestamp":"2026-08-14T05:01:01.000Z","type":"response_item","payload":{"type":"custom_tool_call_output","output":"private"}}"#,
        ] {
            writeln!(file, "{line}").unwrap();
        }
        drop(file);

        let (_, active_turn, snapshot) = scan_snapshot(&path).unwrap();
        assert_eq!(active_turn.as_deref(), Some("turn-2"));
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].kind, MonitorEventKind::Started);
        assert_eq!(snapshot[1].kind, MonitorEventKind::Progress);
    }

    #[test]
    fn extracts_thread_id_from_rollout_filename() {
        let path =
            Path::new("rollout-2026-08-14T05-00-00-019ffeb6-0cb1-7673-8fda-42b6efaf3343.jsonl");
        assert_eq!(
            thread_id_from_path(path).as_deref(),
            Some("019ffeb6-0cb1-7673-8fda-42b6efaf3343")
        );
    }

    #[test]
    fn reads_safe_fallback_metadata_from_user_rollout() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("rollout-2026-08-14T05-00-00-019ffeb6-0cb1-7673-8fda-42b6efaf3343.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-08-14T05:00:00.000Z","type":"session_meta","payload":{"id":"019ffeb6-0cb1-7673-8fda-42b6efaf3343","cwd":"E:\\workspace\\springcat-ai","thread_source":"user"}}
"#,
        )
        .unwrap();

        let metadata = metadata_from_rollout(&path).unwrap();
        assert_eq!(metadata.id, "019ffeb6-0cb1-7673-8fda-42b6efaf3343");
        assert_eq!(
            metadata.workspace.as_deref(),
            Some("E:\\workspace\\springcat-ai")
        );
        assert_eq!(metadata.title, None);
    }
}
