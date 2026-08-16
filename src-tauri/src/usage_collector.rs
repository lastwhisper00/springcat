//! Local, content-free token usage collection for tools that expose exact counts.
//!
//! Only structural usage fields are read. Prompts, responses, tool arguments,
//! source code, and credentials never leave the vendor files and are not stored.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::domain::TaskSource;
use crate::event_collector::CollectorState;
use crate::repository::UsageRecord;

const CODEX_BOOTSTRAP_DAYS: u64 = 62;

pub struct UsageCollectorState {
    _watcher: Mutex<RecommendedWatcher>,
}

#[derive(Debug, Clone, Default)]
struct UsageFileCursor {
    offset: u64,
    model: Option<String>,
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let watch_roots = usage_watch_roots();
    if watch_roots.is_empty() {
        return Ok(());
    }

    let cursors = Arc::new(Mutex::new(HashMap::<PathBuf, UsageFileCursor>::new()));
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(750)),
    )
    .map_err(|err| err.to_string())?;
    for root in &watch_roots {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|err| err.to_string())?;
    }
    app.manage(UsageCollectorState {
        _watcher: Mutex::new(watcher),
    });

    let handle = app.clone();
    std::thread::Builder::new()
        .name("springcat-usage-collector".into())
        .spawn(move || {
            // The watcher is already active, so writes that arrive during the
            // historical scan are queued and replayed from the saved offsets.
            bootstrap(&handle, &cursors);
            for message in rx {
                match message {
                    Ok(event) if is_create_or_modify(&event.kind) => {
                        for path in event.paths {
                            if usage_source_for_path(&path).is_some() {
                                sync_incremental(&handle, &cursors, &path);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(err) => tracing::warn!(error = %err, "usage watcher error"),
                }
            }
        })
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn bootstrap(app: &AppHandle, cursors: &Arc<Mutex<HashMap<PathBuf, UsageFileCursor>>>) {
    let mut files = Vec::new();
    if let Some(home) = codex_home() {
        for root in [home.join("sessions"), home.join("archived_sessions")] {
            collect_recent_rollouts(&root, &mut files);
        }
    }
    if let Some(log) = grok_home().map(|home| home.join("logs").join("unified.jsonl")) {
        if log.is_file() {
            files.push(log);
        }
    }
    files.sort();
    files.dedup();
    for path in files {
        sync_incremental(app, cursors, &path);
    }
}

fn sync_incremental(
    app: &AppHandle,
    cursors: &Arc<Mutex<HashMap<PathBuf, UsageFileCursor>>>,
    path: &Path,
) {
    let Some(source) = usage_source_for_path(path) else {
        return;
    };
    let cursor = cursors
        .lock()
        .expect("usage cursors")
        .remove(path)
        .unwrap_or_default();
    let (next_cursor, records) = match read_appended_usage(path, source, cursor.clone()) {
        Ok(result) => result,
        Err(err) => {
            tracing::debug!(error = %err, file = %path.display(), "usage read deferred");
            cursors
                .lock()
                .expect("usage cursors")
                .insert(path.to_path_buf(), cursor);
            return;
        }
    };
    cursors
        .lock()
        .expect("usage cursors")
        .insert(path.to_path_buf(), next_cursor);

    let Some(state) = app.try_state::<CollectorState>() else {
        return;
    };
    let mut inserted = 0usize;
    let db = state.db.lock().expect("db");
    for record in records {
        match db.insert_usage(&record) {
            Ok(true) => inserted += 1,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(error = %err, source = ?source, "usage record insert failed")
            }
        }
    }
    drop(db);
    if inserted > 0 {
        tracing::debug!(inserted, source = ?source, "token usage records collected");
        let _ = app.emit("usage-updated", ());
    }
}

fn read_appended_usage(
    path: &Path,
    source: TaskSource,
    mut cursor: UsageFileCursor,
) -> Result<(UsageFileCursor, Vec<UsageRecord>), String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let length = file.metadata().map_err(|err| err.to_string())?.len();
    if length < cursor.offset {
        cursor = UsageFileCursor::default();
    }
    file.seek(SeekFrom::Start(cursor.offset))
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::new();

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
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(model) = model_context(&value, source) {
            cursor.model = Some(model);
        }
        let parsed = match source {
            TaskSource::Codex => parse_codex_usage_value(&value, cursor.model.as_deref()),
            TaskSource::GrokCli => parse_grok_usage_value(&value, cursor.model.as_deref()),
            _ => None,
        };
        if let Some(mut record) = parsed {
            if source == TaskSource::Codex && record.session_id.is_none() {
                if let Some(session_id) = codex_session_id_from_path(path) {
                    record.session_id = Some(session_id.clone());
                    record.external_event_id = format!("codex:{session_id}:{}", record.occurred_at);
                    record.id = record.external_event_id.clone();
                }
            }
            records.push(record);
        }
    }
    Ok((cursor, records))
}

#[cfg(test)]
fn parse_codex_usage(line: &str) -> Option<UsageRecord> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    parse_codex_usage_value(&value, None)
}

fn parse_codex_usage_value(
    value: &serde_json::Value,
    fallback_model: Option<&str>,
) -> Option<UsageRecord> {
    if value.get("type")?.as_str()? != "event_msg" {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    let usage = payload.get("info")?.get("last_token_usage")?;
    let occurred_at = value.get("timestamp")?.as_str()?.to_string();
    let input_tokens = usage_i64(usage, "input_tokens");
    let output_tokens = usage_i64(usage, "output_tokens");
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(input_tokens + output_tokens);
    if total_tokens <= 0 {
        return None;
    }
    let session_id = session_id_from_codex_value(value);
    let external_event_id = format!(
        "codex:{}:{}",
        session_id.as_deref().unwrap_or("unknown"),
        occurred_at
    );
    Some(UsageRecord {
        id: external_event_id.clone(),
        source: TaskSource::Codex,
        external_event_id,
        session_id,
        model: usage_model(value).or_else(|| fallback_model.map(str::to_string)),
        local_date: local_date(&occurred_at)?,
        occurred_at,
        input_tokens,
        cached_input_tokens: usage_i64(usage, "cached_input_tokens"),
        output_tokens,
        reasoning_tokens: usage_i64(usage, "reasoning_output_tokens"),
        total_tokens,
        collection_method: "codex-rollout".into(),
        accuracy: "exact".into(),
    })
}

#[cfg(test)]
fn parse_grok_usage(line: &str) -> Option<UsageRecord> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    parse_grok_usage_value(&value, None)
}

fn parse_grok_usage_value(
    value: &serde_json::Value,
    fallback_model: Option<&str>,
) -> Option<UsageRecord> {
    if value.get("msg")?.as_str()? != "shell.turn.inference_done" {
        return None;
    }
    let context = value.get("ctx")?;
    let occurred_at = value.get("ts")?.as_str()?.to_string();
    let input_tokens = usage_i64(context, "prompt_tokens");
    let output_tokens = usage_i64(context, "completion_tokens");
    let reasoning_tokens = usage_i64(context, "reasoning_tokens");
    let total_tokens = input_tokens + output_tokens;
    if total_tokens <= 0 {
        return None;
    }
    let session_id = value
        .get("sid")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let loop_index = context
        .get("loop_index")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    let external_event_id = format!(
        "grok:{}:{}:{}",
        session_id.as_deref().unwrap_or("unknown"),
        occurred_at,
        loop_index
    );
    Some(UsageRecord {
        id: external_event_id.clone(),
        source: TaskSource::GrokCli,
        external_event_id,
        session_id,
        model: context
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| fallback_model.map(str::to_string)),
        local_date: local_date(&occurred_at)?,
        occurred_at,
        input_tokens,
        cached_input_tokens: usage_i64(context, "cached_prompt_tokens"),
        output_tokens,
        reasoning_tokens,
        total_tokens,
        collection_method: "grok-unified-log".into(),
        accuracy: "exact".into(),
    })
}

fn usage_i64(value: &serde_json::Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0)
        .max(0)
}

fn session_id_from_codex_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("payload")?
        .get("session_id")
        .or_else(|| value.get("payload")?.get("thread_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn usage_model(value: &serde_json::Value) -> Option<String> {
    value
        .get("payload")?
        .get("info")?
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn model_context(value: &serde_json::Value, source: TaskSource) -> Option<String> {
    let model = match source {
        TaskSource::Codex if value.get("type")?.as_str()? == "turn_context" => {
            value.get("payload")?.get("model")
        }
        TaskSource::GrokCli => match value.get("msg")?.as_str()? {
            "model changed" => value.get("ctx")?.get("model"),
            "model catalog: notifying clients" => value.get("ctx")?.get("current_model_id"),
            _ => None,
        },
        _ => None,
    }?;
    model.as_str().map(str::to_string)
}

fn local_date(timestamp: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn usage_watch_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = codex_home() {
        roots.extend(
            [home.join("sessions"), home.join("archived_sessions")]
                .into_iter()
                .filter(|path| path.is_dir()),
        );
    }
    if let Some(log_dir) = grok_home().map(|home| home.join("logs")) {
        if log_dir.is_dir() {
            roots.push(log_dir);
        }
    }
    roots
}

fn collect_recent_rollouts(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recent_rollouts(&path, files);
        } else if is_codex_rollout(&path) && recently_modified(&path) {
            files.push(path);
        }
    }
}

fn recently_modified(path: &Path) -> bool {
    let Ok(modified) = path.metadata().and_then(|metadata| metadata.modified()) else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age <= Duration::from_secs(CODEX_BOOTSTRAP_DAYS * 24 * 60 * 60))
        .unwrap_or(true)
}

fn usage_source_for_path(path: &Path) -> Option<TaskSource> {
    if is_codex_rollout(path) {
        Some(TaskSource::Codex)
    } else if path.file_name().and_then(|value| value.to_str()) == Some("unified.jsonl")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("logs")
    {
        Some(TaskSource::GrokCli)
    } else {
        None
    }
}

fn is_codex_rollout(path: &Path) -> bool {
    path.is_file()
        && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("rollout-"))
}

fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let candidate = stem.get(stem.len().checked_sub(36)?..)?;
    (candidate.matches('-').count() == 4).then(|| candidate.to_string())
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}

fn grok_home() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".grok")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_last_request_usage() {
        let record = parse_codex_usage(
            r#"{"timestamp":"2026-08-14T09:18:18.827Z","type":"event_msg","payload":{"type":"token_count","session_id":"thread-1","info":{"last_token_usage":{"input_tokens":70504,"cached_input_tokens":66432,"output_tokens":3612,"reasoning_output_tokens":3380,"total_tokens":74116}}}}"#,
        )
        .unwrap();
        assert_eq!(record.source, TaskSource::Codex);
        assert_eq!(record.input_tokens, 70_504);
        assert_eq!(record.cached_input_tokens, 66_432);
        assert_eq!(record.output_tokens, 3_612);
        assert_eq!(record.reasoning_tokens, 3_380);
        assert_eq!(record.total_tokens, 74_116);
        assert_eq!(record.local_date, "2026-08-14");
    }

    #[test]
    fn carries_codex_turn_model_into_usage_records() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("rollout-2026-08-14T09-00-00-019ffeb6-0cb1-7673-8fda-42b6efaf3343.jsonl");
        fs::write(
            &path,
            concat!(
                r#"{"timestamp":"2026-08-14T09:00:00.000Z","type":"turn_context","payload":{"model":"gpt-5.6-terra"}}"#,
                "\n",
                r#"{"timestamp":"2026-08-14T09:00:01.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}}"#,
                "\n"
            ),
        )
        .unwrap();

        let (_, records) =
            read_appended_usage(&path, TaskSource::Codex, UsageFileCursor::default()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model.as_deref(), Some("gpt-5.6-terra"));
    }

    #[test]
    fn ignores_codex_cumulative_usage_without_last_request() {
        assert!(parse_codex_usage(
            r#"{"timestamp":"2026-08-14T09:18:18.827Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":74116}}}}"#,
        )
        .is_none());
    }

    #[test]
    fn parses_grok_inference_usage() {
        let record = parse_grok_usage(
            r#"{"ts":"2026-08-11T08:02:56.029Z","sid":"session-1","msg":"shell.turn.inference_done","ctx":{"loop_index":4,"prompt_tokens":53829,"cached_prompt_tokens":43904,"completion_tokens":321,"reasoning_tokens":33}}"#,
        )
        .unwrap();
        assert_eq!(record.source, TaskSource::GrokCli);
        assert_eq!(record.input_tokens, 53_829);
        assert_eq!(record.cached_input_tokens, 43_904);
        assert_eq!(record.output_tokens, 321);
        assert_eq!(record.reasoning_tokens, 33);
        assert_eq!(record.total_tokens, 54_150);
        assert!(record.external_event_id.contains("session-1"));
    }

    #[test]
    fn carries_grok_model_change_into_usage_records() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("unified.jsonl");
        fs::write(
            &path,
            concat!(
                r#"{"ts":"2026-08-11T08:00:00.000Z","msg":"model changed","ctx":{"model":"grok-4.5"}}"#,
                "\n",
                r#"{"ts":"2026-08-11T08:02:56.029Z","sid":"session-1","msg":"shell.turn.inference_done","ctx":{"loop_index":4,"prompt_tokens":100,"completion_tokens":10}}"#,
                "\n"
            ),
        )
        .unwrap();

        let (_, records) =
            read_appended_usage(&path, TaskSource::GrokCli, UsageFileCursor::default()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model.as_deref(), Some("grok-4.5"));
    }

    #[test]
    fn carries_grok_catalog_model_into_usage_records() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("unified.jsonl");
        fs::write(
            &path,
            concat!(
                r#"{"ts":"2026-08-11T08:00:00.000Z","msg":"model catalog: notifying clients","ctx":{"model_count":1,"current_model_id":"grok-4.5"}}"#,
                "\n",
                r#"{"ts":"2026-08-11T08:02:56.029Z","sid":"session-1","msg":"shell.turn.inference_done","ctx":{"loop_index":4,"prompt_tokens":100,"completion_tokens":10}}"#,
                "\n"
            ),
        )
        .unwrap();

        let (_, records) =
            read_appended_usage(&path, TaskSource::GrokCli, UsageFileCursor::default()).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model.as_deref(), Some("grok-4.5"));
    }

    #[test]
    fn ignores_unrelated_grok_log_entries() {
        assert!(parse_grok_usage(
            r#"{"ts":"2026-08-11T08:02:56.029Z","msg":"shell.turn.started","ctx":{}}"#,
        )
        .is_none());
    }
}
