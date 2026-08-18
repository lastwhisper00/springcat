mod adapter_installer;
mod adapters;
mod browser;
mod codex_monitor;
mod cursor_metadata;
mod cursor_monitor;
mod docking;
mod domain;
mod event_collector;
mod logging;
mod marvis_monitor;
mod normalizer;
mod openers;
mod paths;
mod platform;
mod repository;
mod settings_store;
mod tray;
mod usage_collector;
mod usage_share;
mod windows;
mod workbuddy_monitor;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;

pub use domain::{
    apply_event_to_task, derive_surface_state, normalize_settings, parse_task_event,
    sanitize_summary, AppSettings, DockSide, PresentationMode, SurfaceState, TaskEvent,
    TaskEventType, TaskItem, TaskSource, TaskStatus, APP_DATA_DIR_NAME, APP_DISPLAY_NAME, APP_NAME,
};

use crate::docking::PanelLayout;
use crate::event_collector::{emit_tasks, CollectorState};
use crate::repository::{DailyUsage, Repository};
use crate::settings_store::PersistedSettings;

#[tauri::command]
fn app_meta() -> serde_json::Value {
    serde_json::json!({
        "name": APP_NAME,
        "displayName": APP_DISPLAY_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "presentationMode": "work",
        "dataDirName": APP_DATA_DIR_NAME
    })
}

#[tauri::command]
fn resize_panel(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    windows::resize_panel(&app, width, height).map_err(|err| err.to_string())
}

#[tauri::command]
fn move_panel(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    windows::move_panel(&app, x, y).map_err(|err| err.to_string())
}

#[tauri::command]
fn place_main_window(app: AppHandle) -> Result<(), String> {
    windows::apply_layout(&app, PanelLayout::Collapsed).map_err(|err| err.to_string())
}

#[tauri::command]
fn prepare_panel_layout(
    app: AppHandle,
    layout: String,
    x: Option<f64>,
    y: Option<f64>,
    dynamic_island_compatible: Option<bool>,
) -> Result<(), String> {
    let at = match (x, y) {
        (Some(px), Some(py)) => Some((px, py)),
        _ => None,
    };
    windows::prepare_layout_surface(
        &app,
        PanelLayout::parse(&layout),
        at,
        dynamic_island_compatible,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn apply_panel_layout(
    app: AppHandle,
    layout: String,
    x: Option<f64>,
    y: Option<f64>,
    dynamic_island_compatible: Option<bool>,
) -> Result<(), String> {
    let at = match (x, y) {
        (Some(px), Some(py)) => Some((px, py)),
        _ => None,
    };
    windows::apply_layout_at(
        &app,
        PanelLayout::parse(&layout),
        at,
        dynamic_island_compatible,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn resize_pinned_panel(
    app: AppHandle,
    width: f64,
    height: f64,
) -> Result<windows::DockChanged, String> {
    windows::resize_pinned_panel(&app, width, height).map_err(|err| err.to_string())
}

#[tauri::command]
fn resize_panel_frame(
    app: AppHandle,
    width: f64,
    height: f64,
    pinned: bool,
) -> Result<windows::DockChanged, String> {
    windows::resize_panel_frame(&app, width, height, pinned).map_err(|err| err.to_string())
}

#[tauri::command]
fn dock_after_drag(
    app: AppHandle,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<windows::DockChanged, String> {
    let at = match (x, y) {
        (Some(px), Some(py)) => Some((px, py)),
        _ => None,
    };
    windows::dock_after_drag_at(&app, at, false).map_err(|err| err.to_string())
}

#[tauri::command]
fn preview_dock(app: AppHandle) -> Result<Option<DockSide>, String> {
    windows::preview_dock(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn top_pin_target(
    app: AppHandle,
    layout: String,
    dynamic_island_compatible: Option<bool>,
) -> Result<windows::DockChanged, String> {
    windows::top_pin_target(&app, PanelLayout::parse(&layout), dynamic_island_compatible)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_panel_pinned(app: AppHandle, pinned: bool) -> Result<(), String> {
    windows::set_panel_pinned(&app, pinned).map_err(|err| err.to_string())
}

#[tauri::command]
fn get_settings(app: AppHandle) -> PersistedSettings {
    event_collector::current_settings(&app)
}

#[tauri::command]
fn browser_info() -> browser::BrowserInfo {
    browser::browser_info()
}

#[tauri::command]
fn save_usage_share_image(file_name: String, bytes: Vec<u8>) -> Result<String, String> {
    usage_share::save_png(&file_name, &bytes)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPatch {
    presentation_mode: Option<PresentationMode>,
    dock_side: Option<DockSide>,
    dynamic_island_compatible: Option<bool>,
    always_on_top: Option<bool>,
    auto_pin_while_running: Option<bool>,
    autostart: Option<bool>,
    muted_until: Option<Option<String>>,
    focus_mode: Option<bool>,
    history_retention_days: Option<u32>,
    cache_directory: Option<String>,
    browser_path: Option<String>,
    adapters: Option<domain::AdapterToggles>,
    double_click_action: Option<String>,
}

#[tauri::command]
fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<PersistedSettings, String> {
    let prepared_cache_directory = match patch.cache_directory.as_deref() {
        Some(value) => Some(prepare_cache_directory(&app, value)?),
        None => None,
    };
    let next = {
        let state = app.state::<Mutex<PersistedSettings>>();
        let mut settings = state.lock().expect("settings");
        if let Some(mode) = patch.presentation_mode {
            settings.app.presentation_mode = mode;
        }
        if let Some(side) = patch.dock_side {
            settings.app.dock_side = side;
        }
        if let Some(compatible) = patch.dynamic_island_compatible {
            settings.app.dynamic_island_compatible = compatible;
        }
        if let Some(on_top) = patch.always_on_top {
            settings.app.always_on_top = on_top;
            if on_top {
                settings.app.dock_side = DockSide::Top;
            }
        }
        if let Some(auto_pin) = patch.auto_pin_while_running {
            settings.app.auto_pin_while_running = auto_pin;
        }
        if let Some(autostart) = patch.autostart {
            settings.app.autostart = autostart;
        }
        if let Some(muted) = patch.muted_until {
            settings.app.muted_until = muted;
        }
        if let Some(focus) = patch.focus_mode {
            settings.app.focus_mode = focus;
        }
        if let Some(days) = patch.history_retention_days {
            settings.app.history_retention_days = days;
        }
        if let Some(cache_directory) = prepared_cache_directory {
            settings.app.cache_directory = cache_directory;
        }
        if let Some(browser_path) = patch.browser_path {
            settings.app.browser_path = if browser_path.trim().is_empty() {
                None
            } else {
                Some(browser_path)
            };
        }
        if let Some(adapters) = patch.adapters {
            settings.app.adapters = adapters;
        }
        if let Some(action) = patch.double_click_action {
            settings.double_click_action = action;
        }
        settings.app = normalize_settings(Some(settings.app.clone()));
        if settings.app.always_on_top {
            settings.app.dock_side = DockSide::Top;
        }
        settings_store::save(&settings);
        settings.clone()
    };

    if patch.dock_side.is_some() {
        let _ = windows::set_dock_side(&app, next.app.dock_side);
    }
    if let Some(on_top) = patch.always_on_top {
        let _ = windows::set_always_on_top(&app, on_top);
    }
    if let Some(autostart) = patch.autostart {
        set_autostart(&app, autostart);
    }
    tray::sync_checks(&app);
    let _ = app.emit("settings-changed", ());
    Ok(next)
}

fn prepare_cache_directory(app: &AppHandle, value: &str) -> Result<Option<String>, String> {
    let target = paths::cache_dir_from_setting(Some(value))?;
    paths::ensure_cache_dirs_at(&target).map_err(|err| format!("无法创建缓存目录：{err}"))?;

    let active = paths::cache_dir();
    if !paths::same_directory(&active, &target) {
        let collector = app
            .try_state::<CollectorState>()
            .ok_or_else(|| "任务缓存尚未初始化。".to_string())?;
        collector
            .db
            .lock()
            .expect("db")
            .copy_all_to(&target.join("tasks.sqlite"))?;
    }

    if paths::same_directory(&target, &paths::data_dir()) {
        Ok(None)
    } else {
        Ok(Some(target.display().to_string()))
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    default_directory: String,
    active_directory: String,
    configured_directory: Option<String>,
    restart_required: bool,
}

#[tauri::command]
fn storage_info(app: AppHandle) -> StorageInfo {
    let configured_directory = {
        let settings = app.state::<Mutex<PersistedSettings>>();
        let configured = settings
            .lock()
            .expect("settings")
            .app
            .cache_directory
            .clone();
        configured
    };
    let desired = paths::cache_dir_from_setting(configured_directory.as_deref())
        .unwrap_or_else(|_| paths::data_dir());
    let active = paths::cache_dir();
    StorageInfo {
        default_directory: paths::data_dir().display().to_string(),
        active_directory: active.display().to_string(),
        configured_directory,
        restart_required: !paths::same_directory(&active, &desired),
    }
}

#[tauri::command]
fn list_tasks(app: AppHandle) -> Result<Vec<TaskItem>, String> {
    let collector = match app.try_state::<CollectorState>() {
        Some(collector) => collector,
        None => return Ok(Vec::new()),
    };
    let items = collector.db.lock().expect("db").list_recent()?;
    Ok(items)
}

#[tauri::command]
fn list_usage_month(app: AppHandle, month: String) -> Result<Vec<DailyUsage>, String> {
    let collector = match app.try_state::<CollectorState>() {
        Some(collector) => collector,
        None => return Ok(Vec::new()),
    };
    let result = collector.db.lock().expect("db").list_usage_month(&month);
    result
}

#[tauri::command]
fn mark_read(app: AppHandle, task_id: String) -> Result<(), String> {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(());
    };
    collector.db.lock().expect("db").mark_read(&task_id)?;
    emit_tasks(&app);
    Ok(())
}

#[tauri::command]
fn mark_all_read(app: AppHandle) -> Result<(), String> {
    let Some(collector) = app.try_state::<CollectorState>() else {
        return Ok(());
    };
    collector.db.lock().expect("db").mark_all_read()?;
    emit_tasks(&app);
    Ok(())
}

#[tauri::command]
fn open_task(app: AppHandle, task_id: String) -> Result<(), String> {
    openers::open_task(&app, &task_id)
}

#[tauri::command]
fn open_latest(app: AppHandle) -> Result<(), String> {
    openers::open_latest_actionable(&app)
}

#[tauri::command]
fn mute_hour(app: AppHandle) -> Result<PersistedSettings, String> {
    let _ = tray::toggle_mute(&app);
    tray::sync_checks(&app);
    Ok(event_collector::current_settings(&app))
}

#[tauri::command]
fn set_focus(app: AppHandle, enabled: bool) -> Result<PersistedSettings, String> {
    {
        let state = app.state::<Mutex<PersistedSettings>>();
        let mut settings = state.lock().expect("settings");
        settings.app.focus_mode = enabled;
        settings_store::save(&settings);
    }
    tray::sync_checks(&app);
    Ok(event_collector::current_settings(&app))
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    windows::open_settings_window(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn popup_panel_menu(app: AppHandle) -> Result<(), String> {
    windows::open_panel_menu(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn panel_menu_action(app: AppHandle, action: String) -> Result<(), String> {
    windows::hide_panel_menu(&app).map_err(|err| err.to_string())?;
    tray::handle_menu(&app, &action);
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn restart_app(app: AppHandle) {
    app.restart();
}

#[tauri::command]
fn adapter_bind_info() -> event_collector::AdapterBindInfo {
    event_collector::adapter_bind_info()
}

fn parse_adapter_source(source: &str) -> Result<TaskSource, String> {
    match source {
        "codex" => Ok(TaskSource::Codex),
        "cursor" => Ok(TaskSource::Cursor),
        "grok-cli" => Ok(TaskSource::GrokCli),
        "gemini-cli" => Ok(TaskSource::GeminiCli),
        "workbuddy" => Ok(TaskSource::WorkBuddy),
        "marvis" => Ok(TaskSource::Marvis),
        _ => Err("unknown source".into()),
    }
}

fn set_adapter_enabled(app: &AppHandle, source: TaskSource, enabled: bool) {
    let state = app.state::<Mutex<PersistedSettings>>();
    let mut settings = state.lock().expect("settings");
    match source {
        TaskSource::Codex => settings.app.adapters.codex = enabled,
        TaskSource::Cursor => settings.app.adapters.cursor = enabled,
        TaskSource::GrokCli => settings.app.adapters.grok_cli = enabled,
        TaskSource::GeminiCli => settings.app.adapters.gemini_cli = enabled,
        TaskSource::WorkBuddy => settings.app.adapters.work_buddy = enabled,
        TaskSource::Marvis => settings.app.adapters.marvis = enabled,
        TaskSource::Unknown => {}
    }
    settings_store::save(&settings);
}

#[tauri::command]
fn adapter_install_status(
    source: String,
) -> Result<adapter_installer::AdapterInstallStatus, String> {
    adapter_installer::status(parse_adapter_source(&source)?)
}

#[tauri::command]
fn install_adapter(
    app: AppHandle,
    source: String,
) -> Result<adapter_installer::AdapterInstallStatus, String> {
    let parsed = parse_adapter_source(&source)?;
    let status = adapter_installer::install(&app, parsed)?;
    set_adapter_enabled(&app, parsed, true);
    Ok(status)
}

#[tauri::command]
fn uninstall_adapter(
    app: AppHandle,
    source: String,
) -> Result<adapter_installer::AdapterInstallStatus, String> {
    let parsed = parse_adapter_source(&source)?;
    let status = adapter_installer::uninstall(parsed)?;
    set_adapter_enabled(&app, parsed, false);
    Ok(status)
}

#[tauri::command]
fn emit_adapter_test(
    app: AppHandle,
    source: String,
) -> Result<event_collector::AdapterTestResult, String> {
    let parsed = parse_adapter_source(&source)?;
    let result = event_collector::emit_adapter_test(&app, parsed)?;
    if result.ok {
        let _ = windows::show_and_focus(&app);
        let layout = if platform::windows::pinned_top_guard_enabled() {
            PanelLayout::PinnedPeek
        } else {
            PanelLayout::Peek
        };
        let _ = windows::apply_layout(&app, layout);
        let _ = app.emit("panel-layout", PanelLayout::Peek.as_str());
    }
    Ok(result)
}

fn set_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    if let Err(err) = result {
        tracing::warn!(error = %err, "autostart update failed");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            app_meta,
            resize_panel,
            move_panel,
            place_main_window,
            prepare_panel_layout,
            apply_panel_layout,
            resize_pinned_panel,
            resize_panel_frame,
            dock_after_drag,
            preview_dock,
            top_pin_target,
            set_panel_pinned,
            get_settings,
            browser_info,
            save_usage_share_image,
            update_settings,
            storage_info,
            list_tasks,
            list_usage_month,
            mark_read,
            mark_all_read,
            open_task,
            open_latest,
            mute_hour,
            set_focus,
            open_settings,
            popup_panel_menu,
            panel_menu_action,
            quit_app,
            restart_app,
            adapter_bind_info,
            adapter_install_status,
            install_adapter,
            uninstall_adapter,
            emit_adapter_test
        ])
        .setup(|app| {
            let settings = settings_store::load();
            if let Err(err) = paths::configure_cache_dir(settings.app.cache_directory.as_deref()) {
                eprintln!("SpringCat cache directory unavailable, using default: {err}");
                paths::configure_cache_dir(None)?;
            }
            paths::ensure_dirs().map_err(|err| err.to_string())?;
            if let Some(guard) = logging::init() {
                app.manage(guard);
            }
            if let Err(err) = adapter_installer::refresh_bridge_if_installed(app.handle()) {
                tracing::warn!(error = %err, "installed bridge refresh failed");
            }
            for (enabled, source) in [
                (settings.app.adapters.codex, TaskSource::Codex),
                (settings.app.adapters.cursor, TaskSource::Cursor),
                (settings.app.adapters.grok_cli, TaskSource::GrokCli),
                (settings.app.adapters.gemini_cli, TaskSource::GeminiCli),
                (settings.app.adapters.work_buddy, TaskSource::WorkBuddy),
                (settings.app.adapters.marvis, TaskSource::Marvis),
            ] {
                if enabled {
                    if let Err(err) = adapter_installer::install(app.handle(), source) {
                        tracing::warn!(source = ?source, error = %err, "enabled adapter auto-bind failed");
                    }
                }
            }
            if settings.app.autostart {
                set_autostart(app.handle(), true);
            }
            let repo = Repository::open(&paths::db_path()).map_err(|err| err.to_string())?;
            let _ = repo.purge(settings.app.history_retention_days);
            if settings.app.adapters.cursor {
                match cursor_metadata::backfill_untitled(&repo) {
                    Ok(changed) if changed > 0 => {
                        tracing::info!(changed, "backfilled Cursor conversation titles");
                    }
                    Ok(_) => {}
                    Err(err) => tracing::warn!(error = %err, "Cursor title backfill failed"),
                }
            }
            app.manage(Mutex::new(settings));
            app.manage(windows::WindowTrack::default());
            app.manage(CollectorState {
                db: Mutex::new(repo),
            });
            windows::setup_main_window(app.handle())?;
            tray::setup_tray(app)?;
            event_collector::start(app.handle())?;
            if let Err(err) = codex_monitor::start(app.handle()) {
                tracing::warn!(error = %err, "Codex fallback monitor failed to start");
            }
            if let Err(err) = cursor_monitor::start(app.handle()) {
                tracing::warn!(error = %err, "Cursor state monitor failed to start");
            }
            if let Err(err) = workbuddy_monitor::start(app.handle()) {
                tracing::warn!(error = %err, "WorkBuddy monitor failed to start");
            }
            if let Err(err) = marvis_monitor::start(app.handle()) {
                tracing::warn!(error = %err, "Marvis monitor failed to start");
            }
            if let Err(err) = usage_collector::start(app.handle()) {
                tracing::warn!(error = %err, "token usage collector failed to start");
            }
            emit_tasks(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running springcat-ai");
}
