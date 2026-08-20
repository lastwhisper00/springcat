//! System tray for the single work-panel window.

use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

use crate::settings_store::{self, PersistedSettings};
use crate::windows;

pub struct TrayMenu {
    pub menu: Menu<Wry>,
    pub dynamic_island: CheckMenuItem<Wry>,
    pub pin: CheckMenuItem<Wry>,
    pub focus: CheckMenuItem<Wry>,
    pub mute: CheckMenuItem<Wry>,
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let settings = app
        .try_state::<Mutex<PersistedSettings>>()
        .map(|state| state.lock().expect("settings").clone())
        .unwrap_or_default();

    let view = MenuItem::with_id(app, "view-tasks", "查看所有任务", true, None::<&str>)?;
    let mark_read = MenuItem::with_id(app, "mark-all-read", "全部标为已读", true, None::<&str>)?;
    let mute = CheckMenuItem::with_id(
        app,
        "mute",
        "静音 1 小时",
        true,
        settings_store::is_muted(&settings.app),
        None::<&str>,
    )?;
    let focus = CheckMenuItem::with_id(
        app,
        "focus",
        "专注模式",
        true,
        settings.app.focus_mode,
        None::<&str>,
    )?;
    let dynamic_island = CheckMenuItem::with_id(
        app,
        "dynamic-island",
        "兼容灵动岛",
        true,
        settings.app.dynamic_island_compatible,
        None::<&str>,
    )?;
    let pin = CheckMenuItem::with_id(
        app,
        "pin",
        "置顶",
        true,
        settings.app.always_on_top,
        None::<&str>,
    )?;
    let pet = MenuItem::with_id(
        app,
        "pet-mode",
        "切换宠物模式（即将推出）",
        false,
        None::<&str>,
    )?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &view,
            &mark_read,
            &mute,
            &focus,
            &dynamic_island,
            &pin,
            &pet,
            &settings_item,
            &sep,
            &quit,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("window icon is required for the tray");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("SpringCat")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = windows::toggle_visible(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayMenu {
        menu,
        dynamic_island,
        pin,
        focus,
        mute,
    });

    Ok(())
}

/// Native context menu on the work panel.
///
/// A second always-on-top WebView (plus `eval` / `location.replace`) can
/// deadlock WebView2 on Windows. That freeze also kills tray Quit, so the
/// panel reuses the tray `Menu` on the main window instead.
///
/// `popup_menu` must run on the window thread. Invoke handlers may already be
/// on that thread, so post the popup from a worker and return immediately —
/// waiting on the UI thread here would deadlock the whole process.
pub fn popup_context_menu(app: &AppHandle) -> tauri::Result<()> {
    let app = app.clone();
    std::thread::spawn(move || {
        let posted = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(stale) = posted.get_webview_window(windows::PANEL_MENU_WINDOW_LABEL) {
                let _ = stale.destroy();
            }
            let Some(main) = posted.get_webview_window(windows::MAIN_WINDOW_LABEL) else {
                return;
            };
            let Some(tray) = posted.try_state::<TrayMenu>() else {
                return;
            };
            let _ = main.popup_menu(&tray.menu);
        });
    });
    Ok(())
}

pub fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "quit" => app.exit(0),
        "view-tasks" => windows::expand_and_focus(app),
        "mark-all-read" => {
            let _ = crate::mark_all_read(app.clone());
        }
        "pin" => {
            let next = toggle_always_on_top(app);
            if let Some(tray) = app.try_state::<TrayMenu>() {
                let _ = tray.pin.set_checked(next);
            }
            let _ = app.emit("settings-changed", ());
        }
        "dynamic-island" => {
            let next = toggle_dynamic_island(app);
            if let Some(tray) = app.try_state::<TrayMenu>() {
                let _ = tray.dynamic_island.set_checked(next);
            }
            let _ = app.emit("settings-changed", ());
        }
        "mute" => {
            let muted = toggle_mute(app);
            if let Some(tray) = app.try_state::<TrayMenu>() {
                let _ = tray.mute.set_checked(muted);
            }
            let _ = app.emit("settings-changed", ());
        }
        "focus" => {
            let next = toggle_focus(app);
            if let Some(tray) = app.try_state::<TrayMenu>() {
                let _ = tray.focus.set_checked(next);
            }
            let _ = app.emit("settings-changed", ());
        }
        "settings" => {
            let _ = windows::open_settings_window(app);
        }
        _ => {}
    }
}

fn toggle_always_on_top(app: &AppHandle) -> bool {
    let next = {
        let state = app.state::<Mutex<PersistedSettings>>();
        let mut settings = state.lock().expect("settings");
        settings.app.always_on_top = !settings.app.always_on_top;
        settings_store::save(&settings);
        settings.app.always_on_top
    };
    let _ = windows::set_always_on_top(app, next);
    next
}

fn toggle_dynamic_island(app: &AppHandle) -> bool {
    let state = app.state::<Mutex<PersistedSettings>>();
    let mut settings = state.lock().expect("settings");
    settings.app.dynamic_island_compatible = !settings.app.dynamic_island_compatible;
    settings_store::save(&settings);
    settings.app.dynamic_island_compatible
}

fn toggle_focus(app: &AppHandle) -> bool {
    let state = app.state::<Mutex<PersistedSettings>>();
    let mut settings = state.lock().expect("settings");
    settings.app.focus_mode = !settings.app.focus_mode;
    settings_store::save(&settings);
    settings.app.focus_mode
}

pub fn toggle_mute(app: &AppHandle) -> bool {
    let state = app.state::<Mutex<PersistedSettings>>();
    let mut settings = state.lock().expect("settings");
    if settings_store::is_muted(&settings.app) {
        settings.app.muted_until = None;
        settings_store::save(&settings);
        false
    } else {
        let until = chrono::Utc::now() + chrono::Duration::hours(1);
        settings.app.muted_until = Some(until.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        settings_store::save(&settings);
        true
    }
}

pub fn sync_checks(app: &AppHandle) {
    let Some(tray) = app.try_state::<TrayMenu>() else {
        return;
    };
    let settings = app.state::<Mutex<PersistedSettings>>();
    let settings = settings.lock().expect("settings");
    let _ = tray
        .dynamic_island
        .set_checked(settings.app.dynamic_island_compatible);
    let _ = tray.pin.set_checked(settings.app.always_on_top);
    let _ = tray.focus.set_checked(settings.app.focus_mode);
    let _ = tray
        .mute
        .set_checked(settings_store::is_muted(&settings.app));
}
