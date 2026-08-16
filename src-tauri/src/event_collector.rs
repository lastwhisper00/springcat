//! Inbox file watcher. One event per JSON file. No polling, no HTTP.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::cursor_metadata;
use crate::domain::{derive_surface_state, TaskSource};
use crate::normalizer::{normalize_value, parse_json, NormalizeError};
use crate::paths;
use crate::repository::Repository;
use crate::settings_store::{self, PersistedSettings};

pub struct CollectorState {
    pub db: Mutex<Repository>,
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    paths::ensure_dirs().map_err(|err| err.to_string())?;
    drain_inbox(app);

    let inbox = paths::inbox_dir();
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .map_err(|err| err.to_string())?;
    watcher
        .watch(&inbox, RecursiveMode::NonRecursive)
        .map_err(|err| err.to_string())?;
    app.manage(Mutex::new(watcher));

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-inbox".into())
        .spawn(move || {
            for message in rx {
                match message {
                    Ok(event) => {
                        if !is_create_or_modify(&event.kind) {
                            continue;
                        }
                        for path in event.paths {
                            process_path(&handle, &path);
                        }
                    }
                    Err(err) => tracing::warn!(error = %err, "inbox watcher error"),
                }
            }
        })
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

pub fn drain_inbox(app: &AppHandle) {
    let Ok(entries) = fs::read_dir(paths::inbox_dir()) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    files.sort();
    for path in files {
        process_path(app, &path);
    }
}

pub fn process_path(app: &AppHandle, path: &Path) {
    if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
        return;
    }
    if !path.exists() {
        return;
    }

    let bytes = match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        Ok(_) => return,
        Err(err) => {
            tracing::warn!(error = %err, file = %path.display(), "failed to read inbox file");
            return;
        }
    };

    let value = match parse_json(&bytes) {
        Ok(value) => value,
        Err(NormalizeError::InvalidJson) => {
            isolate_failed(path, "invalid json");
            return;
        }
        Err(_) => {
            isolate_failed(path, "unusable payload");
            return;
        }
    };

    let source_hint = value
        .get("source")
        .and_then(|item| serde_json::from_value::<TaskSource>(item.clone()).ok());

    let mut event = match normalize_value(&value, source_hint) {
        Ok(event) => event,
        Err(err) => {
            isolate_failed(path, &err.to_string());
            return;
        }
    };

    if event.source == TaskSource::Cursor {
        if let Some(title) = cursor_metadata::conversation_title(&event.task_id) {
            event.title = Some(title);
        }
    }

    if !adapter_enabled(app, event.source) {
        tracing::info!(source = ?event.source, "adapter disabled; dropping event");
        let _ = fs::remove_file(path);
        return;
    }

    let Some(repo) = app.try_state::<CollectorState>() else {
        return;
    };
    let applied = {
        let db = repo.db.lock().expect("db");
        match db.apply_event(&event) {
            Ok(applied) => applied,
            Err(err) => {
                tracing::error!(error = %err, event_id = %event.event_id, "repository apply failed");
                isolate_failed(path, "repository error");
                return;
            }
        }
    };

    let _ = fs::remove_file(path);

    let retention = {
        let settings = app.state::<Mutex<PersistedSettings>>();
        let guard = settings.lock().expect("settings");
        guard.app.history_retention_days
    };
    if let Ok(db) = repo.db.lock() {
        let _ = db.purge(retention);
    }

    if applied.is_none() {
        tracing::debug!(event_id = %event.event_id, "duplicate event ignored");
        return;
    }

    emit_tasks(app);
}

pub fn emit_tasks(app: &AppHandle) {
    let Some(repo) = app.try_state::<CollectorState>() else {
        return;
    };
    let items = match repo.db.lock().expect("db").list_recent() {
        Ok(items) => items,
        Err(err) => {
            tracing::error!(error = %err, "list tasks failed");
            return;
        }
    };
    let surface = derive_surface_state(&items);
    let _ = app.emit("tasks-updated", &items);
    let _ = app.emit("surface-updated", &surface);
}

fn adapter_enabled(app: &AppHandle, source: TaskSource) -> bool {
    let settings = app.state::<Mutex<PersistedSettings>>();
    let toggles = settings.lock().expect("settings").app.adapters.clone();
    match source {
        TaskSource::Codex => toggles.codex,
        TaskSource::Cursor => toggles.cursor,
        TaskSource::GrokCli => toggles.grok_cli,
        TaskSource::GeminiCli => toggles.gemini_cli,
        TaskSource::WorkBuddy => toggles.work_buddy,
        TaskSource::Unknown => true,
    }
}

fn isolate_failed(path: &Path, reason: &str) {
    tracing::warn!(file = %path.display(), reason = %crate::logging::redact(reason), "isolating inbox file");
    let dest = paths::failed_dir().join(
        path.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("event.json")),
    );
    if fs::rename(path, &dest).is_err() {
        let _ = fs::copy(path, dest);
        let _ = fs::remove_file(path);
    }
}

pub fn current_settings(app: &AppHandle) -> PersistedSettings {
    let state = app.state::<Mutex<PersistedSettings>>();
    let mut settings = state.lock().expect("settings");
    settings_store::expire_mute_if_needed(&mut settings);
    settings.clone()
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterBindInfo {
    pub inbox_dir: String,
    pub bridge_path: String,
    pub bridge_found: bool,
}

pub fn adapter_bind_info() -> AdapterBindInfo {
    let found = paths::resolve_bridge();
    let fallback = paths::installed_bridge_path();
    AdapterBindInfo {
        inbox_dir: paths::inbox_dir().display().to_string(),
        bridge_path: found.as_ref().unwrap_or(&fallback).display().to_string(),
        bridge_found: found.is_some(),
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterTestResult {
    pub ok: bool,
    pub via_bridge: bool,
    pub message: String,
}

pub fn emit_adapter_test(app: &AppHandle, source: TaskSource) -> Result<AdapterTestResult, String> {
    if source == TaskSource::Unknown {
        return Err("unknown source".into());
    }
    if !adapter_enabled(app, source) {
        return Ok(AdapterTestResult {
            ok: false,
            via_bridge: false,
            message: "这个适配器已关闭，请先打开开关。".into(),
        });
    }

    let payload = test_payload(source);
    let via_bridge = spawn_bridge_emit(source, &payload);
    if !via_bridge {
        write_inbox(&payload)?;
    }
    drain_inbox(app);

    let message = if source == TaskSource::WorkBuddy {
        "已生成 WorkBuddy 测试完成提醒，请查看桌面圆球。".to_string()
    } else if via_bridge {
        "已通过 springcat-bridge 发出测试任务。请看桌面圆球。".to_string()
    } else {
        "已写入 inbox 测试任务（未找到 springcat-bridge）。面板通路正常，但工具 hook 还需要按指令完成绑定。".to_string()
    };
    Ok(AdapterTestResult {
        ok: true,
        via_bridge,
        message,
    })
}

fn test_payload(source: TaskSource) -> serde_json::Value {
    let source_key = match source {
        TaskSource::Codex => "codex",
        TaskSource::Cursor => "cursor",
        TaskSource::GrokCli => "grok-cli",
        TaskSource::GeminiCli => "gemini-cli",
        TaskSource::WorkBuddy => "workbuddy",
        TaskSource::Unknown => "unknown",
    };
    let id = uuid::Uuid::new_v4().to_string();
    serde_json::json!({
        "schemaVersion": 1,
        "source": source_key,
        "type": "agent-turn-complete",
        "eventId": format!("bind-test-{id}"),
        "taskId": format!("bind-test-{id}"),
        "thread_id": format!("bind-test-{id}"),
        "title": "SpringCat 绑定测试",
        "last-assistant-message": "如果你在桌面圆球上看到这条，说明已经接通。",
        "summary": "如果你在桌面圆球上看到这条，说明已经接通。"
    })
}

fn spawn_bridge_emit(source: TaskSource, payload: &serde_json::Value) -> bool {
    let Some(bridge) = paths::resolve_bridge() else {
        return false;
    };
    let source_key = match source {
        TaskSource::Codex => "codex",
        TaskSource::Cursor => "cursor",
        TaskSource::GrokCli => "grok-cli",
        TaskSource::GeminiCli => "gemini-cli",
        TaskSource::WorkBuddy => return false,
        TaskSource::Unknown => return false,
    };
    let Ok(mut child) = std::process::Command::new(&bridge)
        .args(["emit", "--source", source_key, "--event", "task.completed"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    let Ok(bytes) = serde_json::to_vec(payload) else {
        return false;
    };
    if std::io::Write::write_all(&mut stdin, &bytes).is_err() {
        return false;
    }
    drop(stdin);
    child.wait().map(|status| status.success()).unwrap_or(false)
}

fn write_inbox(payload: &serde_json::Value) -> Result<(), String> {
    paths::ensure_dirs().map_err(|err| err.to_string())?;
    let inbox = paths::inbox_dir();
    let name = format!("{}-bind-test.json", uuid::Uuid::new_v4());
    let dest = inbox.join(&name);
    let tmp = inbox.join(format!("{name}.tmp"));
    let bytes = serde_json::to_vec_pretty(payload).map_err(|err| err.to_string())?;
    fs::write(&tmp, bytes).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &dest).map_err(|err| err.to_string())?;
    Ok(())
}
