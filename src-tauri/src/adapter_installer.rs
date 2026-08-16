//! Installs SpringCat lifecycle hooks without replacing unrelated user hooks.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use tauri::Manager;

use crate::domain::TaskSource;
use crate::paths;

const HOOK_TIMEOUT_SECONDS: u64 = 5;
const GEMINI_HOOK_TIMEOUT_MILLIS: u64 = 5_000;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInstallStatus {
    pub source: String,
    pub installed: bool,
    pub config_path: String,
    pub bridge_installed: bool,
    pub requires_trust: bool,
    pub message: String,
}

pub fn status(source: TaskSource) -> Result<AdapterInstallStatus, String> {
    if source == TaskSource::WorkBuddy {
        return workbuddy_status();
    }
    ensure_supported(source)?;
    let config_path = config_path(source)?;
    let hooks_installed = if config_path.is_file() {
        let root = read_json(&config_path)?;
        config_has_all_hooks(&root, source)
    } else {
        false
    };
    let bridge_installed = paths::installed_bridge_path().is_file();
    let installed = hooks_installed && bridge_installed;
    let message = if installed && source == TaskSource::Codex {
        "已安装。SpringCat 会直接监听本机 Codex 生命周期；受信任的 hooks 作为额外实时通道。"
    } else if installed && source == TaskSource::GrokCli {
        "已安装。Grok CLI 的新会话会自动发送开始、进度和完成状态。"
    } else if installed && source == TaskSource::GeminiCli {
        "已安装。Gemini CLI 会通过原生 hooks 自动发送开始、进度和完成状态。"
    } else if installed {
        "已安装，后续对话会自动发送开始、进度和完成状态。"
    } else if hooks_installed {
        "hooks 已写入，但 bridge 文件缺失；点击修复即可重新安装。"
    } else {
        "尚未安装。"
    };

    Ok(AdapterInstallStatus {
        source: source_key(source).to_string(),
        installed,
        config_path: config_path.display().to_string(),
        bridge_installed,
        requires_trust: source == TaskSource::Codex,
        message: message.to_string(),
    })
}

pub fn install(app: &tauri::AppHandle, source: TaskSource) -> Result<AdapterInstallStatus, String> {
    if source == TaskSource::WorkBuddy {
        return workbuddy_status();
    }
    ensure_supported(source)?;
    let bridge = ensure_bridge(app)?;
    let config = config_path(source)?;
    install_config_at(&config, &bridge, source)?;
    status(source)
}

pub fn uninstall(source: TaskSource) -> Result<AdapterInstallStatus, String> {
    if source == TaskSource::WorkBuddy {
        return workbuddy_status();
    }
    ensure_supported(source)?;
    let config = config_path(source)?;
    if config.is_file() {
        let mut root = read_json(&config)?;
        let original = root.clone();
        remove_springcat_hooks(&mut root, source)?;
        if root != original {
            write_json_atomic(&config, &root)?;
        }
    }
    status(source)
}

/// Keep an already-installed bridge in sync with the desktop app version.
/// This does not install hooks or opt a disabled adapter back in.
pub fn refresh_bridge_if_installed(app: &tauri::AppHandle) -> Result<(), String> {
    if paths::installed_bridge_path().is_file() {
        ensure_bridge(app)?;
    }
    Ok(())
}

fn ensure_supported(source: TaskSource) -> Result<(), String> {
    match source {
        TaskSource::Codex
        | TaskSource::Cursor
        | TaskSource::GrokCli
        | TaskSource::GeminiCli
        | TaskSource::WorkBuddy => Ok(()),
        _ => Err("这个适配器暂不支持自动安装".to_string()),
    }
}

fn workbuddy_status() -> Result<AdapterInstallStatus, String> {
    let projects =
        crate::workbuddy_monitor::projects_dir().ok_or_else(|| "无法定位用户目录".to_string())?;
    let installed = projects.is_dir();
    Ok(AdapterInstallStatus {
        source: "workbuddy".to_string(),
        installed,
        config_path: projects.display().to_string(),
        bridge_installed: true,
        requires_trust: false,
        message: if installed {
            "已检测到 WorkBuddy 本地会话，SpringCat 会直接监听 JSONL 生命周期。".to_string()
        } else {
            "尚未检测到 WorkBuddy 本地会话目录；请先启动一次 WorkBuddy。".to_string()
        },
    })
}

fn source_key(source: TaskSource) -> &'static str {
    match source {
        TaskSource::Codex => "codex",
        TaskSource::Cursor => "cursor",
        TaskSource::GrokCli => "grok-cli",
        TaskSource::GeminiCli => "gemini-cli",
        TaskSource::WorkBuddy => "workbuddy",
        TaskSource::Unknown => "unknown",
    }
}

fn config_path(source: TaskSource) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户目录".to_string())?;
    match source {
        TaskSource::Codex => Ok(home.join(".codex").join("hooks.json")),
        TaskSource::Cursor => Ok(home.join(".cursor").join("hooks.json")),
        TaskSource::GrokCli => Ok(home.join(".grok").join("hooks").join("springcat.json")),
        TaskSource::GeminiCli => Ok(home.join(".gemini").join("settings.json")),
        _ => Err("这个适配器暂不支持自动安装".to_string()),
    }
}

fn ensure_bridge(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    paths::ensure_dirs().map_err(|err| err.to_string())?;
    let destination = paths::installed_bridge_path();
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("bin").join(paths::bridge_name()))
        .filter(|path| path.is_file());
    let source =
        bundled.or_else(|| paths::resolve_bridge().filter(|candidate| candidate != &destination));

    if let Some(source) = source {
        replace_bridge(&source, &destination)?;
    } else if !destination.is_file() {
        return Err("未找到 springcat-bridge，请先构建或重新安装 SpringCat".to_string());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&destination)
            .map_err(|err| err.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions).map_err(|err| err.to_string())?;
    }

    Ok(destination)
}

fn replace_bridge(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "bridge 安装路径无效".to_string())?;
    let nonce = uuid::Uuid::new_v4();
    let temporary = parent.join(format!(".springcat-bridge-{nonce}.tmp"));
    fs::copy(source, &temporary).map_err(|err| format!("复制 springcat-bridge 失败：{err}"))?;

    if !destination.exists() {
        return fs::rename(&temporary, destination)
            .map_err(|err| format!("安装 springcat-bridge 失败：{err}"));
    }

    let swap = parent.join(format!(".springcat-bridge-{nonce}.swap"));
    fs::rename(destination, &swap)
        .map_err(|err| format!("准备更新 springcat-bridge 失败：{err}"))?;
    if let Err(err) = fs::rename(&temporary, destination) {
        let _ = fs::rename(&swap, destination);
        let _ = fs::remove_file(&temporary);
        return Err(format!("更新 springcat-bridge 失败：{err}"));
    }
    let _ = fs::remove_file(swap);
    Ok(())
}

fn install_config_at(config: &Path, bridge: &Path, source: TaskSource) -> Result<(), String> {
    let mut root = if config.is_file() {
        read_json(config)?
    } else {
        json!({})
    };
    let original = root.clone();
    install_hooks(&mut root, bridge, source)?;
    if root == original {
        Ok(())
    } else {
        write_json_atomic(config, &root)
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|err| format!("{} 不是有效 JSON，已停止修改：{err}", path.display()))?;
    if !value.is_object() {
        return Err(format!("{} 的顶层必须是 JSON 对象", path.display()));
    }
    Ok(value)
}

fn hooks_object(root: &mut Value) -> Result<&mut Map<String, Value>, String> {
    let object = root
        .as_object_mut()
        .ok_or_else(|| "hooks 配置顶层必须是 JSON 对象".to_string())?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    hooks
        .as_object_mut()
        .ok_or_else(|| "hooks 字段必须是 JSON 对象".to_string())
}

fn install_hooks(root: &mut Value, bridge: &Path, source: TaskSource) -> Result<(), String> {
    remove_springcat_hooks(root, source)?;
    if source == TaskSource::Cursor {
        root.as_object_mut()
            .expect("validated object")
            .entry("version")
            .or_insert_with(|| json!(1));
    } else if matches!(source, TaskSource::Codex | TaskSource::GrokCli) {
        root.as_object_mut()
            .expect("validated object")
            .entry("description")
            .or_insert_with(|| json!("Lifecycle hooks used by SpringCat task notifications"));
    }

    let hooks = hooks_object(root)?;
    match source {
        TaskSource::Codex => {
            add_codex_hook(
                hooks,
                "UserPromptSubmit",
                command_hook(bridge, source, Some("task.started"), true),
            )?;
            add_codex_hook(
                hooks,
                "PostToolUse",
                command_hook(bridge, source, Some("task.progress"), true),
            )?;
            add_codex_hook(
                hooks,
                "Stop",
                command_hook(bridge, source, Some("task.completed"), true),
            )?;
        }
        TaskSource::Cursor => {
            add_cursor_hook(
                hooks,
                "beforeSubmitPrompt",
                command_hook(bridge, source, Some("task.started"), false),
            )?;
            add_cursor_hook(
                hooks,
                "postToolUse",
                command_hook(bridge, source, Some("task.progress"), false),
            )?;
            add_cursor_hook(
                hooks,
                "afterAgentResponse",
                command_hook(bridge, source, Some("task.progress"), false),
            )?;
            add_cursor_hook(
                hooks,
                "stop",
                command_hook(bridge, source, Some("task.completed"), false),
            )?;
        }
        TaskSource::GrokCli => {
            add_nested_hook(
                hooks,
                "UserPromptSubmit",
                command_hook(bridge, source, Some("task.started"), false),
            )?;
            add_nested_hook(
                hooks,
                "PostToolUse",
                command_hook(bridge, source, Some("task.progress"), false),
            )?;
            add_nested_hook(
                hooks,
                "PostToolUseFailure",
                command_hook(bridge, source, Some("task.progress"), false),
            )?;
            add_nested_hook(
                hooks,
                "Stop",
                command_hook(bridge, source, Some("task.completed"), false),
            )?;
            add_nested_hook(
                hooks,
                "StopFailure",
                command_hook(bridge, source, Some("task.completed"), false),
            )?;
        }
        TaskSource::GeminiCli => {
            add_nested_hook(
                hooks,
                "BeforeAgent",
                gemini_command_hook(bridge, source, "task.started"),
            )?;
            add_nested_hook(
                hooks,
                "AfterTool",
                gemini_command_hook(bridge, source, "task.progress"),
            )?;
            add_nested_hook(
                hooks,
                "AfterAgent",
                gemini_command_hook(bridge, source, "task.completed"),
            )?;
        }
        _ => return Err("这个适配器暂不支持自动安装".to_string()),
    }
    Ok(())
}

fn command_hook(bridge: &Path, source: TaskSource, event: Option<&str>, codex: bool) -> Value {
    let command = hook_command(bridge, source, event);
    if codex {
        let mut handler = json!({
            "type": "command",
            "command": command,
            "timeout": HOOK_TIMEOUT_SECONDS,
            "async": true
        });
        if cfg!(windows) {
            handler.as_object_mut().expect("handler object").insert(
                "commandWindows".to_string(),
                json!(hook_command(bridge, source, event)),
            );
        }
        handler
    } else {
        json!({
            "type": "command",
            "command": command,
            "timeout": HOOK_TIMEOUT_SECONDS
        })
    }
}

fn gemini_command_hook(bridge: &Path, source: TaskSource, event: &str) -> Value {
    json!({
        "type": "command",
        "name": "SpringCat lifecycle",
        "command": hook_command(bridge, source, Some(event)),
        "timeout": GEMINI_HOOK_TIMEOUT_MILLIS
    })
}

fn hook_command(bridge: &Path, source: TaskSource, event: Option<&str>) -> String {
    let escaped = bridge.to_string_lossy().replace('"', "\\\"");
    let mut command = format!("\"{escaped}\" emit --source {}", source_key(source));
    if let Some(event) = event {
        command.push_str(" --event ");
        command.push_str(event);
    }
    command
}

fn add_codex_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    handler: Value,
) -> Result<(), String> {
    let entries = hooks.entry(event).or_insert_with(|| json!([]));
    let entries = entries
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event} 必须是 JSON 数组"))?;
    entries.push(json!({ "hooks": [handler] }));
    Ok(())
}

fn add_nested_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    handler: Value,
) -> Result<(), String> {
    add_codex_hook(hooks, event, handler)
}

fn add_cursor_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    handler: Value,
) -> Result<(), String> {
    let entries = hooks.entry(event).or_insert_with(|| json!([]));
    let entries = entries
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event} 必须是 JSON 数组"))?;
    entries.push(handler);
    Ok(())
}

fn remove_springcat_hooks(root: &mut Value, source: TaskSource) -> Result<(), String> {
    let hooks = hooks_object(root)?;
    let event_names: &[&str] = match source {
        TaskSource::Codex => &["UserPromptSubmit", "PostToolUse", "Stop"],
        TaskSource::Cursor => &[
            "beforeSubmitPrompt",
            "postToolUse",
            "afterAgentResponse",
            "stop",
        ],
        TaskSource::GrokCli => &[
            "UserPromptSubmit",
            "PostToolUse",
            "PostToolUseFailure",
            "Stop",
            "StopFailure",
        ],
        TaskSource::GeminiCli => &["BeforeAgent", "AfterTool", "AfterAgent"],
        _ => return Err("这个适配器暂不支持自动安装".to_string()),
    };

    for event in event_names {
        let Some(value) = hooks.get_mut(*event) else {
            continue;
        };
        let entries = value
            .as_array_mut()
            .ok_or_else(|| format!("hooks.{event} 必须是 JSON 数组"))?;
        if matches!(
            source,
            TaskSource::Codex | TaskSource::GrokCli | TaskSource::GeminiCli
        ) {
            entries.retain_mut(|entry| strip_nested_entry(entry, source));
        } else {
            entries.retain(|entry| !is_springcat_handler(entry, source));
        }
    }
    Ok(())
}

fn strip_nested_entry(entry: &mut Value, source: TaskSource) -> bool {
    if is_springcat_handler(entry, source) {
        return false;
    }
    let Some(handlers) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
        return true;
    };
    let before = handlers.len();
    handlers.retain(|handler| !is_springcat_handler(handler, source));
    before == handlers.len() || !handlers.is_empty()
}

fn is_springcat_handler(value: &Value, source: TaskSource) -> bool {
    ["command", "commandWindows"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .any(|command| {
            command.contains("springcat-bridge")
                && command.contains(&format!("--source {}", source_key(source)))
        })
}

fn config_has_all_hooks(root: &Value, source: TaskSource) -> bool {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let requirements: &[(&str, Option<&str>)] = match source {
        TaskSource::Codex => &[
            ("UserPromptSubmit", Some("task.started")),
            ("PostToolUse", Some("task.progress")),
            ("Stop", Some("task.completed")),
        ],
        TaskSource::Cursor => &[
            ("beforeSubmitPrompt", Some("task.started")),
            ("postToolUse", Some("task.progress")),
            ("afterAgentResponse", Some("task.progress")),
            ("stop", Some("task.completed")),
        ],
        TaskSource::GrokCli => &[
            ("UserPromptSubmit", Some("task.started")),
            ("PostToolUse", Some("task.progress")),
            ("PostToolUseFailure", Some("task.progress")),
            ("Stop", Some("task.completed")),
            ("StopFailure", Some("task.completed")),
        ],
        TaskSource::GeminiCli => &[
            ("BeforeAgent", Some("task.started")),
            ("AfterTool", Some("task.progress")),
            ("AfterAgent", Some("task.completed")),
        ],
        _ => return false,
    };

    requirements.iter().all(|(event, expected_event)| {
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    handler_matches(entry, source, *expected_event)
                        || entry
                            .get("hooks")
                            .and_then(Value::as_array)
                            .is_some_and(|handlers| {
                                handlers.iter().any(|handler| {
                                    handler_matches(handler, source, *expected_event)
                                })
                            })
                })
            })
    })
}

fn handler_matches(value: &Value, source: TaskSource, expected_event: Option<&str>) -> bool {
    ["command", "commandWindows"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .any(|command| {
            is_springcat_handler(value, source)
                && expected_event.is_none_or(|event| command.contains(&format!("--event {event}")))
        })
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} 没有父目录", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    bytes.push(b'\n');

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("hooks.json");
    let nonce = uuid::Uuid::new_v4();
    let temporary = parent.join(format!(".{file_name}.springcat-{nonce}.tmp"));
    fs::write(&temporary, bytes).map_err(|err| err.to_string())?;

    if !path.exists() {
        return fs::rename(&temporary, path).map_err(|err| err.to_string());
    }

    let backup = parent.join(format!("{file_name}.springcat.bak"));
    if !backup.exists() {
        fs::copy(path, &backup).map_err(|err| format!("备份 hooks 配置失败：{err}"))?;
    }

    let swap = parent.join(format!(".{file_name}.springcat-{nonce}.swap"));
    fs::rename(path, &swap).map_err(|err| format!("准备替换 hooks 配置失败：{err}"))?;
    if let Err(err) = fs::rename(&temporary, path) {
        let _ = fs::rename(&swap, path);
        let _ = fs::remove_file(&temporary);
        return Err(format!("写入 hooks 配置失败：{err}"));
    }
    let _ = fs::remove_file(swap);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_install_and_remove_preserve_unrelated_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("hooks.json");
        let bridge = temp.path().join("springcat-bridge.exe");
        fs::write(&bridge, b"bridge").unwrap();
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "hooks": {
                    "PostToolUse": [{
                        "matcher": "Bash",
                        "hooks": [{ "type": "command", "command": "audit.exe" }]
                    }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        install_config_at(&config, &bridge, TaskSource::Codex).unwrap();
        let mut installed = read_json(&config).unwrap();
        assert!(config_has_all_hooks(&installed, TaskSource::Codex));
        assert_eq!(
            installed["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
            "audit.exe"
        );

        remove_springcat_hooks(&mut installed, TaskSource::Codex).unwrap();
        assert!(!config_has_all_hooks(&installed, TaskSource::Codex));
        assert_eq!(
            installed["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
            "audit.exe"
        );
    }

    #[test]
    fn cursor_install_maps_only_start_progress_and_completed() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("hooks.json");
        let bridge = temp.path().join("springcat-bridge.exe");
        fs::write(&bridge, b"bridge").unwrap();

        install_config_at(&config, &bridge, TaskSource::Cursor).unwrap();
        let installed = read_json(&config).unwrap();
        assert!(config_has_all_hooks(&installed, TaskSource::Cursor));
        let stop = installed["hooks"]["stop"][0]["command"].as_str().unwrap();
        assert!(stop.contains("--source cursor"));
        assert!(stop.contains("--event task.completed"));
    }

    #[test]
    fn grok_install_uses_global_native_hooks_and_real_event_names() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("springcat.json");
        let bridge = temp.path().join("springcat-bridge.exe");
        fs::write(&bridge, b"bridge").unwrap();

        install_config_at(&config, &bridge, TaskSource::GrokCli).unwrap();
        let installed = read_json(&config).unwrap();
        assert!(config_has_all_hooks(&installed, TaskSource::GrokCli));
        assert!(
            installed["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains("--source grok-cli --event task.started")
        );
        assert!(installed["hooks"]["StopFailure"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("--event task.completed"));
    }

    #[test]
    fn gemini_install_preserves_user_settings_and_uses_millisecond_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("settings.json");
        let bridge = temp.path().join("springcat-bridge.exe");
        fs::write(&bridge, b"bridge").unwrap();
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "security": { "auth": { "selectedType": "oauth-personal" } },
                "hooks": {
                    "AfterTool": [{
                        "matcher": "run_shell_command",
                        "hooks": [{ "type": "command", "command": "audit.exe" }]
                    }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        install_config_at(&config, &bridge, TaskSource::GeminiCli).unwrap();
        let installed = read_json(&config).unwrap();

        assert!(config_has_all_hooks(&installed, TaskSource::GeminiCli));
        assert_eq!(
            installed["security"]["auth"]["selectedType"],
            "oauth-personal"
        );
        assert_eq!(
            installed["hooks"]["AfterTool"][0]["hooks"][0]["command"],
            "audit.exe"
        );
        let handler = &installed["hooks"]["BeforeAgent"][0]["hooks"][0];
        assert_eq!(handler["timeout"], GEMINI_HOOK_TIMEOUT_MILLIS);
        assert!(handler["command"]
            .as_str()
            .unwrap()
            .contains("--source gemini-cli --event task.started"));

        let mut removed = installed.clone();
        remove_springcat_hooks(&mut removed, TaskSource::GeminiCli).unwrap();
        assert!(!config_has_all_hooks(&removed, TaskSource::GeminiCli));
        assert_eq!(removed["security"], installed["security"]);
        assert_eq!(
            removed["hooks"]["AfterTool"][0]["hooks"][0]["command"],
            "audit.exe"
        );
    }

    #[test]
    fn replace_bridge_upgrades_an_existing_binary() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("bundled-bridge.exe");
        let destination = temp.path().join("springcat-bridge.exe");
        fs::write(&source, b"new bridge").unwrap();
        fs::write(&destination, b"old bridge").unwrap();

        replace_bridge(&source, &destination).unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"new bridge");
    }
}
