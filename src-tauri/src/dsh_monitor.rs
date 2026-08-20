//! Passive DeepSeek Harness Desktop (DSH) lifecycle monitor.
//!
//! DSH stores a per-session project cache at
//! `<data>/dsh-desktop/harness/storages/session_projcache.json`. The harness
//! rewrites that file continuously while a conversation is active, exposing —
//! per session — a short `title`, an `openStep` object while a turn is in
//! flight, `pendingCalls` / `plan` while it is blocked on input, and a
//! monotonically increasing `seq`. SpringCat reads only those structural
//! fields; the compressed transcript under `harness/sessions/...` is never
//! touched, so prompts, reasoning, tool arguments, and results never reach
//! SpringCat's database.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::domain::{TaskEvent, TaskEventType, TaskSource};
use crate::event_collector::{self, CollectorState};
use crate::settings_store::{now_rfc3339, PersistedSettings};

const SYNC_DEBOUNCE: Duration = Duration::from_millis(300);
const SETUP_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const TITLE_LIMIT: usize = 80;
const CACHE_FILE: &str = "session_projcache.json";

pub struct DshMonitorState {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Running,
    Waiting,
    Completed,
}

#[derive(Debug, Clone)]
struct SessionState {
    phase: Phase,
    steps: i64,
    seq: i64,
    progress_emitted: bool,
}

#[derive(Debug, Default)]
struct MonitorCursor {
    initialized: bool,
    sessions: HashMap<String, SessionState>,
}

#[derive(Debug, Clone)]
struct SessionSnapshot {
    id: String,
    title: String,
    steps: i64,
    seq: i64,
    running: bool,
    waiting: bool,
}

#[derive(Debug)]
struct PendingEvent {
    task_id: String,
    event_type: TaskEventType,
    title: String,
    occurred_at: String,
    event_seq: i64,
}

pub fn harness_dir() -> Option<PathBuf> {
    std::env::var_os("DSH_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|dir| dir.join("dsh-desktop").join("harness")))
}

pub fn cache_path() -> Option<PathBuf> {
    harness_dir().map(|dir| dir.join("storages").join(CACHE_FILE))
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let Some(harness) = harness_dir() else {
        return Ok(());
    };
    if !harness.is_dir() {
        return Ok(());
    }
    let storages = harness.join("storages");

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .map_err(|err| err.to_string())?;
    let initial_watch = if storages.is_dir() {
        storages.clone()
    } else {
        harness.clone()
    };
    watcher
        .watch(&initial_watch, RecursiveMode::NonRecursive)
        .map_err(|err| err.to_string())?;

    let watcher = Arc::new(Mutex::new(watcher));
    app.manage(DshMonitorState {
        _watcher: watcher.clone(),
    });

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-dsh-monitor".into())
        .spawn(move || {
            let mut watched_path = initial_watch;
            let mut watching_storages = watched_path == storages;
            let mut cursor = MonitorCursor::default();
            let cache = harness.join("storages").join(CACHE_FILE);
            let mut dirty = cache.is_file();
            let mut last_sync = Instant::now()
                .checked_sub(SYNC_DEBOUNCE)
                .unwrap_or_else(Instant::now);
            let mut last_setup_check = Instant::now()
                .checked_sub(SETUP_CHECK_INTERVAL)
                .unwrap_or_else(Instant::now);

            loop {
                match rx.recv_timeout(Duration::from_millis(120)) {
                    Ok(Ok(event)) if is_relevant(&event.kind) => {
                        let mut relevant = false;
                        for path in &event.paths {
                            if path.file_name().and_then(|name| name.to_str()) == Some(CACHE_FILE) {
                                relevant = true;
                            }
                            if path == &storages {
                                relevant = true;
                            }
                        }
                        if relevant {
                            dirty = true;
                        }
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => tracing::warn!(error = %err, "DSH watcher error"),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }

                if !watching_storages && last_setup_check.elapsed() >= SETUP_CHECK_INTERVAL {
                    last_setup_check = Instant::now();
                    if storages.is_dir() {
                        let mut guard = watcher.lock().expect("DSH watcher");
                        if guard.watch(&storages, RecursiveMode::NonRecursive).is_ok() {
                            let _ = guard.unwatch(&watched_path);
                            watched_path = storages.clone();
                            watching_storages = true;
                            dirty = true;
                        }
                    }
                }

                if dirty && cache.is_file() && last_sync.elapsed() >= SYNC_DEBOUNCE {
                    match sync(&handle, &cache, &mut cursor) {
                        Ok(()) => dirty = false,
                        Err(err) => {
                            tracing::debug!(error = %err, "DSH session cache read deferred");
                        }
                    }
                    last_sync = Instant::now();
                }
            }
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn sync(app: &AppHandle, cache: &Path, cursor: &mut MonitorCursor) -> Result<(), String> {
    let bytes = fs::read(cache).map_err(|err| err.to_string())?;
    let root: Value = serde_json::from_slice(&bytes).map_err(|err| err.to_string())?;

    let snapshots = read_sessions(&root);
    let now = now_rfc3339();
    let mut events = Vec::new();

    for snapshot in snapshots {
        plan_events(
            &snapshot,
            cursor.sessions.get(&snapshot.id),
            &now,
            &mut events,
        );
        cursor.sessions.insert(
            snapshot.id.clone(),
            SessionState {
                phase: phase_of(&snapshot),
                steps: snapshot.steps,
                seq: snapshot.seq,
                progress_emitted: cursor
                    .sessions
                    .get(&snapshot.id)
                    .is_some_and(|state| state.progress_emitted)
                    || events.iter().any(|event| {
                        event.task_id == snapshot.id && event.event_type == TaskEventType::Progress
                    }),
            },
        );
    }
    cursor.initialized = true;

    if !events.is_empty() {
        apply_events(app, events);
    }
    Ok(())
}

fn read_sessions(root: &Value) -> Vec<SessionSnapshot> {
    let Some(sessions) = root
        .get("tables")
        .and_then(|tables| tables.get("sessions"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    sessions
        .iter()
        .filter_map(|(id, value)| parse_session(id, value))
        .collect()
}

fn parse_session(id: &str, value: &Value) -> Option<SessionSnapshot> {
    let object = value.as_object()?;
    let rows = object.get("rows")?.as_object()?;

    let mut seq = 0i64;
    let mut steps = 0i64;
    let mut running = false;
    let mut waiting = false;
    let mut raw_title: Option<String> = None;

    for (key, row) in rows {
        seq = seq.max(row.get("seq").and_then(Value::as_i64).unwrap_or(0));
        let Some(val) = row.get("val") else { continue };
        match key.as_str() {
            "sessionStats" => {
                steps = val.get("steps").and_then(Value::as_i64).unwrap_or(0);
                running = val.get("openStep").is_some_and(|open| open.is_object());
                if val
                    .get("pendingCalls")
                    .and_then(Value::as_object)
                    .is_some_and(|calls| !calls.is_empty())
                {
                    waiting = true;
                }
            }
            "title" => {
                raw_title = val.as_str().map(str::to_string);
            }
            "plan" => {
                if val.get("active").and_then(Value::as_bool).unwrap_or(false) {
                    waiting = true;
                }
                if val.get("wanted").is_some_and(|wanted| !wanted.is_null()) {
                    waiting = true;
                }
            }
            _ => {}
        }
    }

    // A placeholder row with no lifecycle activity yet is noise, not a task.
    if seq == 0 && steps == 0 && !running && !waiting {
        return None;
    }

    let title = raw_title
        .as_deref()
        .and_then(|value| compact_text(value, TITLE_LIMIT))
        .unwrap_or_else(|| "DSH 任务".to_string());

    Some(SessionSnapshot {
        id: id.to_string(),
        title,
        steps,
        seq,
        running,
        waiting,
    })
}

fn phase_of(snapshot: &SessionSnapshot) -> Phase {
    if snapshot.running {
        Phase::Running
    } else if snapshot.waiting {
        Phase::Waiting
    } else {
        Phase::Completed
    }
}

fn plan_events(
    snapshot: &SessionSnapshot,
    previous: Option<&SessionState>,
    now: &str,
    events: &mut Vec<PendingEvent>,
) {
    let phase = phase_of(snapshot);
    let Some(previous) = previous else {
        match phase {
            Phase::Running => push(events, snapshot, TaskEventType::Started, now),
            Phase::Waiting => push(events, snapshot, TaskEventType::Waiting, now),
            Phase::Completed => {}
        }
        return;
    };

    if snapshot.seq <= previous.seq {
        return;
    }

    let event_type = match (previous.phase, phase) {
        (Phase::Running, Phase::Completed) => Some(TaskEventType::Completed),
        (Phase::Running, Phase::Waiting) => Some(TaskEventType::Waiting),
        (Phase::Running, Phase::Running)
            if snapshot.steps > previous.steps && !previous.progress_emitted =>
        {
            Some(TaskEventType::Progress)
        }
        (Phase::Waiting, Phase::Running) => Some(TaskEventType::Progress),
        (Phase::Waiting, Phase::Completed) => Some(TaskEventType::Completed),
        (Phase::Completed, Phase::Running) => Some(TaskEventType::Started),
        (Phase::Completed, Phase::Waiting) => Some(TaskEventType::Waiting),
        (Phase::Running, Phase::Running)
        | (Phase::Waiting, Phase::Waiting)
        | (Phase::Completed, Phase::Completed) => None,
    };

    if let Some(event_type) = event_type {
        push(events, snapshot, event_type, now);
    }
}

fn push(
    events: &mut Vec<PendingEvent>,
    snapshot: &SessionSnapshot,
    event_type: TaskEventType,
    now: &str,
) {
    events.push(PendingEvent {
        task_id: snapshot.id.clone(),
        event_type,
        title: snapshot.title.clone(),
        occurred_at: now.to_string(),
        event_seq: snapshot.seq,
    });
}

fn apply_events(app: &AppHandle, events: Vec<PendingEvent>) {
    if !dsh_adapter_enabled(app) {
        return;
    }
    let Some(collector) = app.try_state::<CollectorState>() else {
        return;
    };
    let db = collector.db.lock().expect("db");
    let mut changed = false;

    for event in events {
        let session_id = event.task_id.clone();
        let event_id = format!(
            "dsh-desktop:{session_id}:{}:{}",
            event.event_seq,
            event_type_key(event.event_type)
        );
        let task = TaskEvent {
            schema_version: 1,
            event_id,
            source: TaskSource::DshDesktop,
            event_type: event.event_type,
            task_id: event.task_id.clone(),
            session_id: Some(session_id),
            parent_task_id: None,
            project_name: None,
            workspace_path: None,
            title: Some(event.title),
            summary: None,
            occurred_at: event.occurred_at,
            deep_link: None,
            raw: None,
        };
        match db.apply_event(&task) {
            Ok(Some(_)) => changed = true,
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, event_id = %task.event_id, "DSH event insert failed")
            }
        }
    }
    drop(db);

    if changed {
        event_collector::emit_tasks(app);
    }
}

fn event_type_key(event_type: TaskEventType) -> &'static str {
    match event_type {
        TaskEventType::Started => "started",
        TaskEventType::Progress => "progress",
        TaskEventType::Waiting => "waiting",
        TaskEventType::Completed => "completed",
        TaskEventType::Failed => "failed",
        TaskEventType::Cancelled => "cancelled",
    }
}

fn dsh_adapter_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<PersistedSettings>>()
        .map(|settings| settings.lock().expect("settings").app.adapters.dsh_desktop)
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

fn is_relevant(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session(
        id: &str,
        seq: i64,
        steps: i64,
        open: bool,
        pending: bool,
        title: Option<&str>,
    ) -> SessionSnapshot {
        SessionSnapshot {
            id: id.to_string(),
            title: title
                .map(str::to_string)
                .unwrap_or_else(|| "DSH 任务".to_string()),
            steps,
            seq,
            running: open,
            waiting: pending,
        }
    }

    #[test]
    fn parses_a_live_session_from_the_project_cache() {
        let root = json!({
            "tables": {
                "sessions": {
                    "session-1": {
                        "identity": { "createdAt": 1787049235440i64, "cwd": "E:\\workspace\\app" },
                        "rows": {
                            "sessionStats": { "ver": 1, "seq": 101, "val": {
                                "turns": 4, "steps": 130, "lastTurn": 4,
                                "openStep": { "turn": 4, "step": 10, "startTime": 1787050897378i64 },
                                "pendingCalls": {}
                            }},
                            "title": { "ver": 1, "seq": 101, "val": "优化监听" },
                            "plan": { "ver": 1, "seq": 101, "val": { "active": false, "wanted": null } }
                        }
                    }
                }
            }
        });
        let sessions = read_sessions(&root);
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].running);
        assert!(!sessions[0].waiting);
        assert_eq!(sessions[0].steps, 130);
        assert_eq!(sessions[0].seq, 101);
        assert_eq!(sessions[0].title, "优化监听");
    }

    #[test]
    fn treats_pending_calls_or_active_plan_as_waiting() {
        let pending = json!({
            "rows": {
                "sessionStats": { "seq": 5, "val": { "steps": 3, "openStep": null, "pendingCalls": { "call-1": {} } }},
                "title": { "seq": 5, "val": "等待确认" }
            }
        });
        let plan = json!({
            "rows": {
                "sessionStats": { "seq": 5, "val": { "steps": 3, "openStep": null, "pendingCalls": {} }},
                "title": { "seq": 5, "val": "计划确认" },
                "plan": { "seq": 5, "val": { "active": true, "wanted": null } }
            }
        });
        let pending_session = parse_session("s-pending", &pending).unwrap();
        let plan_session = parse_session("s-plan", &plan).unwrap();
        assert_eq!(phase_of(&pending_session), Phase::Waiting);
        assert_eq!(phase_of(&plan_session), Phase::Waiting);
    }

    #[test]
    fn ignores_placeholder_sessions_without_activity() {
        let placeholder = json!({
            "rows": {
                "sessionStats": { "seq": 0, "val": { "steps": 0, "openStep": null, "pendingCalls": {} }},
                "title": { "seq": 0, "val": null }
            }
        });
        assert!(parse_session("blank", &placeholder).is_none());
    }

    #[test]
    fn transitions_lifecycle_across_syncs() {
        let _started = session("s1", 10, 1, true, false, Some("做图标"));
        let previous = SessionState {
            phase: Phase::Running,
            steps: 1,
            seq: 10,
            progress_emitted: false,
        };

        // More steps while still running → one Progress, then never again.
        let mut events = Vec::new();
        plan_events(
            &session("s1", 11, 5, true, false, Some("做图标")),
            Some(&previous),
            "now",
            &mut events,
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, TaskEventType::Progress);

        let mut events = Vec::new();
        plan_events(
            &session("s1", 12, 9, true, false, Some("做图标")),
            Some(&SessionState {
                phase: Phase::Running,
                steps: 5,
                seq: 11,
                progress_emitted: true,
            }),
            "now",
            &mut events,
        );
        assert!(events.is_empty());

        // Running → Completed.
        let mut events = Vec::new();
        plan_events(
            &session("s1", 13, 12, false, false, Some("做图标")),
            Some(&SessionState {
                phase: Phase::Running,
                steps: 9,
                seq: 12,
                progress_emitted: true,
            }),
            "now",
            &mut events,
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, TaskEventType::Completed);

        // A new turn in the same session restarts it.
        let mut events = Vec::new();
        plan_events(
            &session("s1", 20, 13, true, false, Some("继续优化")),
            Some(&SessionState {
                phase: Phase::Completed,
                steps: 12,
                seq: 13,
                progress_emitted: true,
            }),
            "now",
            &mut events,
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, TaskEventType::Started);
    }
}
