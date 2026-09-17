//! ZCode lifecycle reconciliation.
//!
//! ZCode hooks are the realtime source of lifecycle events. If ZCode exits
//! before its `Stop` hook is delivered, SpringCat would otherwise persist the
//! last started/progress event as running forever. This monitor treats the
//! ZCode process lifetime as the fallback terminal signal.

use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use tauri::{AppHandle, Manager};

use crate::domain::{TaskEvent, TaskEventType, TaskItem, TaskSource, TaskStatus};
use crate::event_collector::{self, CollectorState};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const PROCESS_EXIT_GRACE_SECONDS: i64 = 30;

pub fn start(app: &AppHandle) -> Result<(), String> {
    // Reconcile once before the initial UI snapshot so stale tasks left by a
    // previous ZCode process disappear immediately after SpringCat starts.
    if let Err(err) = reconcile(app) {
        tracing::warn!(error = %err, "initial ZCode lifecycle reconciliation failed");
    }

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-zcode-monitor".into())
        .spawn(move || loop {
            std::thread::sleep(RECONCILE_INTERVAL);
            if let Err(err) = reconcile(&handle) {
                tracing::warn!(error = %err, "ZCode lifecycle reconciliation failed");
            }
        })
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn reconcile(app: &AppHandle) -> Result<usize, String> {
    if zcode_process_running()? {
        return Ok(0);
    }
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(0);
    };
    let tasks = collector
        .db
        .lock()
        .expect("db")
        .list_active_for_source(TaskSource::Zcode)?;
    let now = Utc::now();
    let events: Vec<TaskEvent> = tasks
        .iter()
        .filter_map(|task| process_exit_event(task, now))
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
        tracing::info!(changed, "cancelled stale ZCode tasks after process exit");
        event_collector::emit_tasks(app);
    }
    Ok(changed)
}

fn process_exit_event(task: &TaskItem, now: DateTime<Utc>) -> Option<TaskEvent> {
    if task.source != TaskSource::Zcode
        || !matches!(task.status, TaskStatus::Running | TaskStatus::Waiting)
    {
        return None;
    }
    let updated_at = DateTime::parse_from_rfc3339(&task.updated_at).ok()?;
    if now.signed_duration_since(updated_at).num_seconds() < PROCESS_EXIT_GRACE_SECONDS {
        return None;
    }
    let occurred_at = now.to_rfc3339_opts(SecondsFormat::Millis, true);
    Some(TaskEvent {
        schema_version: 1,
        event_id: format!("zcode-process-exit:{}:{}", task.id, task.updated_at),
        source: TaskSource::Zcode,
        event_type: TaskEventType::Cancelled,
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

#[cfg(target_os = "windows")]
fn zcode_process_running() -> Result<bool, String> {
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|err| err.to_string())?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = false;
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = std::ffi::OsString::from_wide(&entry.szExeFile[..length]);
            if name.to_string_lossy().eq_ignore_ascii_case("ZCode.exe") {
                found = true;
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    Ok(found)
}

#[cfg(target_os = "macos")]
fn zcode_process_running() -> Result<bool, String> {
    let status = std::process::Command::new("/usr/bin/pgrep")
        .args(["-x", "ZCode"])
        .status()
        .map_err(|err| err.to_string())?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        code => Err(format!("pgrep exited with status {code:?}")),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn zcode_process_running() -> Result<bool, String> {
    Err("ZCode process reconciliation is unsupported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(source: TaskSource, status: TaskStatus, updated_at: &str) -> TaskItem {
        TaskItem {
            id: "zcode-session".into(),
            source,
            title: "ZCode task".into(),
            summary: None,
            status,
            started_at: Some(updated_at.into()),
            updated_at: updated_at.into(),
            completed_at: None,
            unread: false,
            action: None,
        }
    }

    #[test]
    fn stale_active_zcode_task_is_cancelled_after_process_exit() {
        let now = DateTime::parse_from_rfc3339("2026-09-15T14:00:31.000Z")
            .unwrap()
            .with_timezone(&Utc);
        let event = process_exit_event(
            &task(
                TaskSource::Zcode,
                TaskStatus::Running,
                "2026-09-15T14:00:00.000Z",
            ),
            now,
        )
        .unwrap();

        assert_eq!(event.event_type, TaskEventType::Cancelled);
        assert_eq!(event.task_id, "zcode-session");
    }

    #[test]
    fn recent_zcode_hook_gets_a_grace_period() {
        let now = DateTime::parse_from_rfc3339("2026-09-15T14:00:29.000Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(process_exit_event(
            &task(
                TaskSource::Zcode,
                TaskStatus::Waiting,
                "2026-09-15T14:00:00.000Z",
            ),
            now,
        )
        .is_none());
    }

    #[test]
    fn other_sources_are_never_reconciled_by_zcode_process_state() {
        let now = DateTime::parse_from_rfc3339("2026-09-15T14:10:00.000Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(process_exit_event(
            &task(
                TaskSource::Codex,
                TaskStatus::Running,
                "2026-09-15T14:00:00.000Z",
            ),
            now,
        )
        .is_none());
    }
}
