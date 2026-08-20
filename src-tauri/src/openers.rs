//! Open a task's deepLink, else the source app / workspace folder.

use std::process::Command;

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::domain::{TaskItem, TaskSource};
use crate::event_collector::{self, CollectorState};

pub fn open_task(app: &AppHandle, task_id: &str) -> Result<(), String> {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(());
    };
    let task = collector
        .db
        .lock()
        .expect("db")
        .get_task(task_id)
        .map_err(|err| err)?;
    let Some(task) = task else {
        return Ok(());
    };
    let _ = collector.db.lock().expect("db").mark_read(task_id);
    event_collector::emit_tasks(app);
    open_item(app, &task)
}

pub fn open_latest_actionable(app: &AppHandle) -> Result<(), String> {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(());
    };
    let items = collector.db.lock().expect("db").list_recent()?;
    let latest = items.into_iter().find(|task| {
        matches!(
            task.status,
            crate::domain::TaskStatus::Waiting | crate::domain::TaskStatus::Failed
        ) || task.unread
    });
    if let Some(task) = latest {
        open_task(app, &task.id)?;
    }
    Ok(())
}

pub fn open_item(app: &AppHandle, task: &TaskItem) -> Result<(), String> {
    // Cursor's /agent deeplink opens a separate Agents window. When SpringCat
    // cannot safely select an existing conversation, only focus Cursor itself.
    if task.source == TaskSource::Cursor {
        return open_source(app, task);
    }
    // Marvis exposes an internal /chat/:conversationId route through its
    // pseudo protocol. Route first, then explicitly wake the real AI starter
    // window: a tray-minimized Marvis process keeps a tiny off-screen helper
    // window that otherwise looks like a successful focus operation.
    if task.source == TaskSource::Marvis {
        return open_marvis(app, task);
    }
    if let Some(link) = task
        .action
        .as_ref()
        .and_then(|action| action.deep_link.as_deref())
    {
        if open_link(app, link) {
            return Ok(());
        }
    }
    let generated_link = match task.source {
        TaskSource::Codex => codex_thread_link(&task.id),
        TaskSource::Cursor
        | TaskSource::GrokCli
        | TaskSource::GeminiCli
        | TaskSource::WorkBuddy
        | TaskSource::Marvis
        | TaskSource::DshDesktop
        | TaskSource::Unknown => None,
    };
    if let Some(link) = generated_link {
        if open_link(app, &link) {
            return Ok(());
        }
    }
    open_source(app, task)
}

const MARVIS_ACTIVATION_LINK: &str =
    "marvis://client/windowActive?check_ai_starter=1&from_source=springcat";

fn open_marvis(app: &AppHandle, task: &TaskItem) -> Result<(), String> {
    let conversation_link = task
        .action
        .as_ref()
        .and_then(|action| action.deep_link.clone())
        .or_else(|| crate::marvis_monitor::conversation_link_for_response(&task.id));

    if let Some(link) = conversation_link {
        // ShellExecute only confirms that Windows accepted the URI. Always
        // follow it with Marvis' starter activation flag so a conversation
        // routed while the app is in the tray becomes visible.
        let _ = open_link(app, &link);
    }
    if app
        .opener()
        .open_url(MARVIS_ACTIVATION_LINK, None::<&str>)
        .is_ok()
    {
        return Ok(());
    }

    // Older builds may not register the pseudo protocol. In that case focus
    // a genuine window if possible, then fall back to launching the app.
    if crate::platform::windows::focus_existing_process_window("Marvis.exe", "Marvis") {
        return Ok(());
    }
    if spawn("Marvis") || spawn("Marvis.exe") {
        return Ok(());
    }
    Ok(())
}

fn codex_thread_link(task_id: &str) -> Option<String> {
    uuid::Uuid::parse_str(task_id)
        .ok()
        .map(|thread_id| format!("codex://threads/{thread_id}"))
}

fn safe_task_key(task_id: &str) -> bool {
    !task_id.is_empty()
        && task_id.len() <= 256
        && task_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn open_link(app: &AppHandle, link: &str) -> bool {
    if crate::browser::is_http_url(link) {
        return crate::browser::open_http_url(app, link);
    }
    if link.contains("://") {
        return app.opener().open_url(link, None::<&str>).is_ok();
    }
    let path = std::path::Path::new(link);
    if path.exists() {
        return app.opener().open_path(link, None::<&str>).is_ok();
    }
    app.opener().open_url(link, None::<&str>).is_ok()
}

fn open_source(app: &AppHandle, task: &TaskItem) -> Result<(), String> {
    match task.source {
        TaskSource::Cursor => {
            if crate::platform::windows::focus_existing_process_window("Cursor.exe", "Cursor") {
                return Ok(());
            }
            if spawn("cursor") || spawn("Cursor") {
                return Ok(());
            }
            let _ = app.opener().open_url("cursor://", None::<&str>);
        }
        TaskSource::Codex => {
            if app.opener().open_url("codex://", None::<&str>).is_ok() {
                return Ok(());
            }
            if spawn("codex") {
                return Ok(());
            }
        }
        TaskSource::GrokCli => {
            if safe_task_key(&task.id) && open_grok_session(&task.id) {
                return Ok(());
            }
            if open_grok_terminal(&[]) || spawn("grok") || spawn("grok-cli") {
                return Ok(());
            }
        }
        TaskSource::GeminiCli => {
            if open_gemini_terminal() || spawn("gemini") {
                return Ok(());
            }
        }
        TaskSource::WorkBuddy => {
            if crate::platform::windows::focus_existing_process_window("WorkBuddy.exe", "WorkBuddy")
            {
                return Ok(());
            }
            let _ = app.opener().open_url("workbuddy://", None::<&str>);
        }
        TaskSource::Marvis => return open_marvis(app, task),
        TaskSource::DshDesktop => {
            if crate::platform::windows::focus_existing_process_window(
                "DSH Desktop.exe",
                "DSH Desktop",
            ) {
                return Ok(());
            }
            let _ = app.opener().open_url("dsh-desktop://", None::<&str>);
            let _ = spawn("DSH Desktop");
        }
        TaskSource::Unknown => {}
    }
    Ok(())
}

fn spawn(program: &str) -> bool {
    Command::new(program).spawn().is_ok()
}

fn spawn_with_args(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).spawn().is_ok()
}

fn open_grok_session(task_id: &str) -> bool {
    open_grok_terminal(&["--resume", task_id])
        || spawn_with_args("grok", &["--resume", task_id])
        || spawn_with_args("grok-cli", &["--resume", task_id])
}

#[cfg(windows)]
fn open_gemini_terminal() -> bool {
    Command::new("cmd.exe")
        .args(["/D", "/C", "start", "", "gemini"])
        .spawn()
        .is_ok()
}

#[cfg(not(windows))]
fn open_gemini_terminal() -> bool {
    false
}

#[cfg(windows)]
fn open_grok_terminal(args: &[&str]) -> bool {
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/C", "start", "", "grok"]);
    command.args(args);
    command.spawn().is_ok()
}

#[cfg(not(windows))]
fn open_grok_terminal(_args: &[&str]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_codex_thread_deep_link_from_session_id() {
        assert_eq!(
            codex_thread_link("019ffeb6-0cb1-7673-8fda-42b6efaf3343").as_deref(),
            Some("codex://threads/019ffeb6-0cb1-7673-8fda-42b6efaf3343")
        );
    }

    #[test]
    fn rejects_non_codex_task_ids_as_deep_links() {
        assert_eq!(codex_thread_link("unknown-task"), None);
    }

    #[test]
    fn rejects_unsafe_task_keys() {
        assert!(!safe_task_key("conversation id&mode=new"));
        assert!(!safe_task_key(""));
    }

    #[test]
    fn uses_the_real_marvis_starter_activation_link() {
        assert_eq!(
            MARVIS_ACTIVATION_LINK,
            "marvis://client/windowActive?check_ai_starter=1&from_source=springcat"
        );
    }
}
