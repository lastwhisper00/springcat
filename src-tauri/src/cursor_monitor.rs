//! Cursor lifecycle reconciliation.
//!
//! Cursor hooks remain the realtime source of started/progress/completed
//! events. Some background or resumed generations can emit `postToolUse`
//! after an earlier `stop` without emitting a second terminal hook. This
//! monitor watches Cursor's own state database and repairs only terminal
//! states whose checkpoint is at least as new as SpringCat's last event.

use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use crate::cursor_metadata::{self, ConversationState};
use crate::domain::{TaskEvent, TaskEventType, TaskItem, TaskSource, TaskStatus};
use crate::event_collector::{self, CollectorState};

const RECONCILE_DEBOUNCE: Duration = Duration::from_millis(500);

pub struct CursorMonitorState {
    _watcher: Mutex<RecommendedWatcher>,
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let Some(state_db) = cursor_metadata::state_db_path() else {
        return Ok(());
    };
    let Some(root) = state_db.parent() else {
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
        .watch(root, RecursiveMode::NonRecursive)
        .map_err(|err| err.to_string())?;
    app.manage(CursorMonitorState {
        _watcher: Mutex::new(watcher),
    });

    // Watching begins before reconciliation so a Cursor write racing with
    // startup is queued and handled again after the initial snapshot.
    reconcile(app)?;

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-cursor-monitor".into())
        .spawn(move || loop {
            let message = match rx.recv() {
                Ok(message) => message,
                Err(_) => break,
            };
            match message {
                Ok(event) if event_touches_state_db(&event.kind, &event.paths) => {}
                Ok(_) => continue,
                Err(err) => {
                    tracing::warn!(error = %err, "Cursor state watcher error");
                    continue;
                }
            }

            // SQLite commonly produces a short burst of database/WAL events.
            // Wait for the burst to settle before reading a consistent state.
            loop {
                match rx.recv_timeout(RECONCILE_DEBOUNCE) {
                    Ok(Ok(event)) if event_touches_state_db(&event.kind, &event.paths) => continue,
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        tracing::warn!(error = %err, "Cursor state watcher error")
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            if let Err(err) = reconcile(&handle) {
                tracing::warn!(error = %err, "Cursor lifecycle reconciliation failed");
            }
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn event_touches_state_db(kind: &EventKind, paths: &[std::path::PathBuf]) -> bool {
    if !matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    ) {
        return false;
    }
    paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "state.vscdb" || name.starts_with("state.vscdb-"))
    })
}

fn reconcile(app: &AppHandle) -> Result<usize, String> {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(0);
    };
    let tasks = collector
        .db
        .lock()
        .expect("db")
        .list_active_for_source(TaskSource::Cursor)?;

    let events: Vec<TaskEvent> = tasks
        .iter()
        .filter_map(|task| {
            let state = cursor_metadata::conversation_state(&task.id)?;
            terminal_event(task, &state)
        })
        .collect();
    if events.is_empty() {
        return Ok(0);
    }

    let mut changed = 0;
    {
        let db = collector.db.lock().expect("db");
        for event in &events {
            if db.apply_event(event)?.is_some() {
                changed += 1;
            }
        }
    }
    if changed > 0 {
        tracing::info!(changed, "reconciled Cursor terminal lifecycle state");
        event_collector::emit_tasks(app);
    }
    Ok(changed)
}

fn terminal_event(task: &TaskItem, state: &ConversationState) -> Option<TaskEvent> {
    if task.source != TaskSource::Cursor
        || !matches!(task.status, TaskStatus::Running | TaskStatus::Waiting)
    {
        return None;
    }
    let (event_type, terminal_key) = match state.status.as_str() {
        "completed" => (TaskEventType::Completed, "completed"),
        "aborted" => (TaskEventType::Cancelled, "aborted"),
        _ => return None,
    };

    let task_updated_at = DateTime::parse_from_rfc3339(&task.updated_at)
        .ok()?
        .timestamp_millis();
    if state.updated_at_millis < task_updated_at {
        return None;
    }
    let occurred_at = DateTime::<Utc>::from_timestamp_millis(state.updated_at_millis)?
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let version = state
        .generation_id
        .as_deref()
        .unwrap_or(occurred_at.as_str());

    Some(TaskEvent {
        schema_version: 1,
        event_id: format!("cursor-state:{}:{version}:{terminal_key}", task.id),
        source: TaskSource::Cursor,
        event_type,
        task_id: task.id.clone(),
        session_id: Some(task.id.clone()),
        parent_task_id: None,
        project_name: None,
        workspace_path: None,
        title: None,
        summary: None,
        occurred_at,
        deep_link: None,
        raw: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_task(updated_at: &str) -> TaskItem {
        TaskItem {
            id: "cursor-conversation".into(),
            source: TaskSource::Cursor,
            title: "Cursor task".into(),
            summary: None,
            status: TaskStatus::Running,
            started_at: Some("2026-08-14T09:00:00.000Z".into()),
            updated_at: updated_at.into(),
            completed_at: None,
            unread: false,
            action: None,
        }
    }

    #[test]
    fn completed_checkpoint_repairs_a_missed_stop() {
        let state = ConversationState {
            status: "completed".into(),
            updated_at_millis: 1_786_698_311_030,
            generation_id: Some("generation-2".into()),
        };
        let event = terminal_event(&running_task("2026-08-14T09:05:07.181Z"), &state).unwrap();

        assert_eq!(event.event_type, TaskEventType::Completed);
        assert_eq!(event.occurred_at, "2026-08-14T09:05:11.030Z");
        assert!(event.event_id.contains("generation-2"));
    }

    #[test]
    fn stale_completed_snapshot_cannot_close_a_new_turn() {
        let state = ConversationState {
            status: "completed".into(),
            updated_at_millis: 1_786_698_311_030,
            generation_id: Some("old-generation".into()),
        };

        assert!(terminal_event(&running_task("2026-08-14T09:06:00.000Z"), &state).is_none());
    }

    #[test]
    fn aborted_cursor_generation_becomes_cancelled() {
        let state = ConversationState {
            status: "aborted".into(),
            updated_at_millis: 1_786_698_311_030,
            generation_id: None,
        };
        let event = terminal_event(&running_task("2026-08-14T09:05:07.181Z"), &state).unwrap();

        assert_eq!(event.event_type, TaskEventType::Cancelled);
    }
}
