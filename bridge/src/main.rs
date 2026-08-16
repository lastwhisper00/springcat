//! springcat-bridge: stdin JSON → atomic inbox file.
//! No HTTP. The desktop app watches the configured SpringCat cache inbox.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const APP_DATA_DIR_NAME: &str = "springcat-ai";

fn main() {
    if let Err(err) = run() {
        eprintln!("springcat-bridge: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("emit") {
        return Err(
            "usage: springcat-bridge emit --source <codex|cursor|grok-cli|gemini-cli> [--event <type>]".into(),
        );
    }

    let requested_source = flag(&args, "--source").unwrap_or_else(|| "unknown".to_string());
    let event_type = flag(&args, "--event");

    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|err| err.to_string())?;
    // Some Windows hook runners prepend a UTF-8 BOM to redirected stdin.
    let stdin = stdin.trim().trim_start_matches('\u{feff}').to_string();

    let vendor_payload = if stdin.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&stdin).map_err(|err| format!("stdin json: {err}"))?
    };
    let source = actual_source(&requested_source, &vendor_payload);
    let mut payload = lifecycle_payload(&vendor_payload)?;

    let object = payload
        .as_object_mut()
        .ok_or_else(|| "stdin json must be an object".to_string())?;
    if let Some(title) = lifecycle_title(&source, &vendor_payload) {
        object.insert("title".to_string(), serde_json::Value::String(title));
    }
    object.insert(
        "source".to_string(),
        serde_json::Value::String(source.clone()),
    );
    if let Some(event_type) = event_type {
        object.insert("type".to_string(), serde_json::Value::String(event_type));
    }
    object
        .entry("schemaVersion".to_string())
        .or_insert_with(|| serde_json::json!(1));
    if !object.contains_key("eventId") {
        let event_id = stable_event_id(object).unwrap_or_else(|| unique_id("evt"));
        object.insert("eventId".into(), serde_json::Value::String(event_id));
    }
    if !object.contains_key("occurredAt") {
        object.insert("occurredAt".into(), serde_json::Value::String(now_iso()));
    }

    let hook_event_name = object
        .get("hook_event_name")
        .or_else(|| object.get("hookEventName"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();

    if should_ignore_grok_session_stop(&source, &hook_event_name, object) {
        println!("{{}}");
        return Ok(());
    }

    let inbox = cache_dir().join("inbox");
    fs::create_dir_all(&inbox).map_err(|err| err.to_string())?;
    let name = format!("{}-{}.json", millis(), unique_id("f"));
    let dest = inbox.join(&name);
    let tmp = inbox.join(format!("{name}.tmp"));
    let bytes = serde_json::to_vec_pretty(&payload).map_err(|err| err.to_string())?;
    fs::write(&tmp, bytes).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &dest).map_err(|err| err.to_string())?;
    eprintln!("springcat-bridge: wrote {}", dest.display());
    if matches!(
        hook_event_name.as_str(),
        "UserPromptSubmit" | "user_prompt_submit" | "beforeSubmitPrompt"
    ) {
        println!("{}", r#"{"continue":true}"#);
    } else {
        println!("{{}}");
    }
    Ok(())
}

/// Keep only lifecycle identity and status metadata. Prompts, responses, tool
/// arguments/results, and transcript paths never enter SpringCat's inbox. A
/// short title may be derived separately, but the source text is still dropped.
fn lifecycle_payload(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let input = input
        .as_object()
        .ok_or_else(|| "stdin json must be an object".to_string())?;
    let mut output = serde_json::Map::new();
    for key in [
        "hook_event_name",
        "hookEventName",
        "status",
        "reason",
        "session_id",
        "sessionId",
        "thread_id",
        "thread-id",
        "threadId",
        "conversation_id",
        "conversationId",
        "generation_id",
        "generationId",
        "task_id",
        "taskId",
        "cwd",
        "workspace_path",
        "workspacePath",
        "workspace",
        "workspace_roots",
        "workspaceRoot",
        "timestamp",
        "project_name",
        "projectName",
        "project",
    ] {
        if let Some(value) = input.get(key) {
            output.insert(key.to_string(), value.clone());
        }
    }
    Ok(serde_json::Value::Object(output))
}

fn lifecycle_title(source: &str, input: &serde_json::Value) -> Option<String> {
    if source == "cursor" {
        let conversation_id = ["conversation_id", "conversationId"]
            .iter()
            .find_map(|key| input.get(*key).and_then(serde_json::Value::as_str));
        if let Some(title) = conversation_id.and_then(cursor_conversation_title) {
            return Some(title);
        }
    }

    ["title", "task_title", "taskTitle"]
        .iter()
        .find_map(|key| input.get(*key).and_then(serde_json::Value::as_str))
        .and_then(compact_title)
        .or_else(|| {
            ["prompt", "user_prompt", "userPrompt"]
                .iter()
                .find_map(|key| input.get(*key).and_then(serde_json::Value::as_str))
                .and_then(compact_title)
        })
}

fn cursor_conversation_title(conversation_id: &str) -> Option<String> {
    let db_path = dirs::config_dir()?
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    let connection = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let key = format!("composerData:{conversation_id}");
    let raw: String = connection
        .query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            [&key],
            |row| row.get(0),
        )
        .ok()?;
    title_from_composer_json(&raw)
}

fn title_from_composer_json(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .and_then(compact_title)
}

fn compact_title(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .collect();
    let first_line = cleaned.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() || is_placeholder_title(first_line) {
        return None;
    }
    Some(first_line.chars().take(80).collect())
}

fn is_placeholder_title(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "new chat" | "new conversation" | "untitled" | "未命名" | "未命名任务"
    )
}

fn actual_source(requested: &str, input: &serde_json::Value) -> String {
    if input.get("hookEventName").is_some() {
        "grok-cli".to_string()
    } else {
        requested.to_string()
    }
}

fn stable_event_id(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let session = [
        "sessionId",
        "session_id",
        "conversation_id",
        "conversationId",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))?;
    let hook = ["hookEventName", "hook_event_name"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))?;
    let timestamp = object
        .get("timestamp")
        .and_then(serde_json::Value::as_str)?;
    let event_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("lifecycle");
    let source = object
        .get("source")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    Some(format!(
        "hook:{source}:{session}:{hook}:{event_type}:{timestamp}"
    ))
}

fn should_ignore_grok_session_stop(
    source: &str,
    hook_event_name: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    if source != "grok-cli" || !hook_event_name.eq_ignore_ascii_case("stop") {
        return false;
    }
    matches!(
        object.get("reason").and_then(serde_json::Value::as_str),
        Some("channel_closed" | "shutdown")
    )
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|item| item == name)
        .and_then(|index| args.get(index + 1).cloned())
}

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(env::temp_dir)
        .join(APP_DATA_DIR_NAME)
}

fn cache_dir() -> PathBuf {
    let default = data_dir();
    let settings_path = default.join("settings.json");
    let Ok(raw) = fs::read_to_string(settings_path) else {
        return default;
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return default;
    };
    cache_dir_from_settings(&settings, default)
}

fn cache_dir_from_settings(settings: &serde_json::Value, default: PathBuf) -> PathBuf {
    let Some(value) = settings
        .get("cacheDirectory")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return default;
    };
    let selected = PathBuf::from(value);
    if selected.is_absolute() {
        selected
    } else {
        default
    }
}

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", millis(), std::process::id())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lifecycle_payload_drops_conversation_and_tool_content() {
        let payload = lifecycle_payload(&json!({
            "hook_event_name": "PostToolUse",
            "session_id": "session-1",
            "cwd": "E:/workspace/app",
            "prompt": "private prompt",
            "last_assistant_message": "private response",
            "tool_input": { "command": "private command" },
            "tool_response": "private output",
            "transcript_path": "C:/private/transcript.jsonl"
        }))
        .unwrap();

        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["hook_event_name"], "PostToolUse");
        assert!(payload.get("prompt").is_none());
        assert!(payload.get("last_assistant_message").is_none());
        assert!(payload.get("tool_input").is_none());
        assert!(payload.get("tool_response").is_none());
        assert!(payload.get("transcript_path").is_none());
    }

    #[test]
    fn lifecycle_title_keeps_only_a_compact_prompt_title() {
        let input = json!({
            "prompt": "Fix the login flow\nDo not change the API.",
            "conversation_id": "missing-conversation"
        });
        assert_eq!(
            lifecycle_title("cursor", &input).as_deref(),
            Some("Fix the login flow")
        );
        let payload = lifecycle_payload(&input).unwrap();
        assert!(payload.get("prompt").is_none());
    }

    #[test]
    fn composer_json_exposes_cursor_generated_name() {
        assert_eq!(
            title_from_composer_json(r#"{"name":"Guanchao backend logging setup"}"#).as_deref(),
            Some("Guanchao backend logging setup")
        );
        assert!(title_from_composer_json(r#"{"name":"New Chat"}"#).is_none());
    }

    #[test]
    fn lifecycle_payload_rejects_non_objects() {
        assert!(lifecycle_payload(&json!(["not", "an", "object"])).is_err());
    }

    #[test]
    fn grok_camel_case_lifecycle_fields_are_preserved_without_content() {
        let payload = lifecycle_payload(&json!({
            "hookEventName": "post_tool_use",
            "sessionId": "grok-1",
            "workspaceRoot": "E:/workspace/app",
            "timestamp": "2026-08-13T12:00:00Z",
            "toolInput": { "command": "private" },
            "toolResult": "private"
        }))
        .unwrap();

        assert_eq!(payload["hookEventName"], "post_tool_use");
        assert_eq!(payload["sessionId"], "grok-1");
        assert_eq!(payload["workspaceRoot"], "E:/workspace/app");
        assert!(payload.get("toolInput").is_none());
        assert!(payload.get("toolResult").is_none());
    }

    #[test]
    fn grok_payload_overrides_cursor_compatibility_source_and_dedupes() {
        let payload = json!({
            "hookEventName": "post_tool_use",
            "sessionId": "grok-1",
            "timestamp": "2026-08-13T12:00:00Z",
            "source": "grok-cli",
            "type": "task.progress"
        });
        assert_eq!(actual_source("cursor", &payload), "grok-cli");
        let object = payload.as_object().unwrap();
        assert_eq!(stable_event_id(object), stable_event_id(object));
        assert!(stable_event_id(object).unwrap().contains("grok-1"));
    }

    #[test]
    fn grok_session_shutdown_stop_is_not_a_second_completion() {
        let payload = json!({ "reason": "channel_closed" });
        assert!(should_ignore_grok_session_stop(
            "grok-cli",
            "stop",
            payload.as_object().unwrap()
        ));
        assert!(!should_ignore_grok_session_stop(
            "grok-cli",
            "stop",
            json!({ "reason": "end_turn" }).as_object().unwrap()
        ));
    }

    #[test]
    fn configured_cache_directory_must_be_absolute() {
        let default = std::env::temp_dir().join("springcat-default");
        let custom = std::env::temp_dir().join("springcat-custom");
        let settings = json!({ "cacheDirectory": custom });
        assert_eq!(cache_dir_from_settings(&settings, default.clone()), custom);
        assert_eq!(
            cache_dir_from_settings(
                &json!({ "cacheDirectory": "relative/cache" }),
                default.clone(),
            ),
            default
        );
    }
}
