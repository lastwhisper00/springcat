//! Single work-panel WebView: size, work-area placement, docking.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{
    window::Color, AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Runtime,
    WebviewWindow,
};

use crate::docking::{self, DockTarget, PanelLayout, Rect, SNAP_THRESHOLD};
use crate::domain::DockSide;
use crate::event_collector;
use crate::platform;
use crate::settings_store::{self, MonitorDock, PersistedSettings};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const SETTINGS_WINDOW_LABEL: &str = "settings";
const WEBVIEW_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-blink-features=MiddleClickAutoscroll";

/// The persisted pin flag controls the overlay's placement and pinned layout,
/// not whether the orb can be covered by ordinary application windows. The
/// main window is skipped from the taskbar, so dropping it out of the topmost
/// band would make an unpinned orb appear to vanish with no obvious way back.
fn overlay_native_topmost(_pinned: bool) -> bool {
    true
}

fn dynamic_island_compatible(app: &AppHandle) -> bool {
    app.try_state::<Mutex<PersistedSettings>>()
        .map(|state| {
            state
                .lock()
                .expect("settings")
                .app
                .dynamic_island_compatible
        })
        .unwrap_or(false)
}

/// Last native `Moved` position. `outer_position()` is often stale after `startDragging`.
#[derive(Default)]
pub struct WindowTrack {
    gen: AtomicU64,
    pos: Mutex<Option<(f64, f64)>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockChanged {
    pub side: DockSide,
    pub along: f64,
    pub preview: bool,
    pub x: f64,
    pub y: f64,
}

pub fn setup_main_window(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    window.set_skip_taskbar(true)?;
    platform::windows::configure_overlay_window(&window)?;
    window.set_shadow(false)?;
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    let (on_top, dynamic_island_compatible) = app
        .try_state::<Mutex<PersistedSettings>>()
        .map(|state| {
            let settings = state.lock().expect("settings");
            (
                settings.app.always_on_top,
                settings.app.dynamic_island_compatible,
            )
        })
        .unwrap_or((true, false));
    platform::windows::set_pinned_top_guard(on_top);
    window.set_always_on_top(overlay_native_topmost(on_top))?;
    let (side, along) = current_side_along(app, &window)?;
    let initial_layout = if on_top {
        PanelLayout::PinnedCollapsed
    } else {
        PanelLayout::Collapsed
    };
    let (x, y) = apply_side_layout(
        &window,
        side,
        Some(along),
        initial_layout,
        dynamic_island_compatible,
    )?;
    if let Some((_, scale, _)) = work_rect(&window)? {
        remember_logical(app, x, y, scale);
    }
    watch_moves(app, &window);
    Ok(())
}

fn watch_moves(app: &AppHandle, window: &WebviewWindow) {
    let handle = app.clone();
    window.on_window_event(move |event| {
        let tauri::WindowEvent::Moved(pos) = event else {
            return;
        };
        let Some(track) = handle.try_state::<WindowTrack>() else {
            return;
        };
        *track.pos.lock().expect("pos") = Some((pos.x as f64, pos.y as f64));
        let n = track.gen.fetch_add(1, Ordering::Relaxed) + 1;
        let app = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(320));
            let Some(track) = app.try_state::<WindowTrack>() else {
                return;
            };
            if track.gen.load(Ordering::Relaxed) != n {
                return;
            }
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = settle_window_if_idle(&app2);
            });
        });
    });
}

fn settle_window_if_idle(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    let Some((_, scale, _)) = work_rect(&window)? else {
        return Ok(());
    };
    let size = window.outer_size()?.to_logical::<f64>(scale);
    let pinned = platform::windows::pinned_top_guard_enabled();
    if pinned {
        let Some(bounds) = pinned_top_rect(&window)? else {
            return Ok(());
        };
        // Native window dragging does not reliably return a pointer-up event to
        // the WebView. Re-anchor every pinned shape after movement settles, not
        // just the 44 px collapsed orb, while preserving its current layout.
        let layout = if size.height >= 280.0 {
            PanelLayout::PinnedExpanded
        } else if size.width >= 160.0 {
            PanelLayout::PinnedPeek
        } else {
            PanelLayout::PinnedCollapsed
        };
        let (width, _) = docking::size_for(DockSide::Top, layout, dynamic_island_compatible(app));
        let (target_x, target_y) = docking::pinned_top_position(bounds, width);
        let position = window.outer_position()?.to_logical::<f64>(scale);
        if (position.x - target_x).abs() < 0.5 && (position.y - target_y).abs() < 0.5 {
            return Ok(());
        }
        // Native dragging owns the mouse loop on Windows, so WebView pointer-up
        // is not guaranteed to fire. Notify the UI after movement settles and
        // let its single animation path return the pill; moving here would race
        // that path and produce visible jumps.
        let _ = app.emit(
            "pinned-reanchor",
            DockChanged {
                side: DockSide::Top,
                along: target_x,
                preview: false,
                x: target_x,
                y: target_y,
            },
        );
        return Ok(());
    }
    if size.width > 80.0 || size.height > 80.0 {
        return Ok(());
    }
    dock_after_drag(app)?;
    Ok(())
}

fn remember_logical(app: &AppHandle, x: f64, y: f64, scale: f64) {
    if let Some(track) = app.try_state::<WindowTrack>() {
        *track.pos.lock().expect("pos") = Some((x * scale, y * scale));
    }
}

fn live_logical_pos(
    app: &AppHandle,
    window: &WebviewWindow,
    scale: f64,
    at_physical: Option<(f64, f64)>,
) -> tauri::Result<LogicalPosition<f64>> {
    if let Some((x, y)) = at_physical {
        return Ok(PhysicalPosition::new(x, y).to_logical(scale));
    }
    if let Some(track) = app.try_state::<WindowTrack>() {
        if let Some((x, y)) = *track.pos.lock().expect("pos") {
            return Ok(PhysicalPosition::new(x, y).to_logical(scale));
        }
    }
    Ok(window.outer_position()?.to_logical(scale))
}

pub fn work_rect<R: Runtime>(
    window: &WebviewWindow<R>,
) -> tauri::Result<Option<(Rect, f64, String)>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(None);
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let origin = work.position.to_logical::<f64>(scale);
    let area = work.size.to_logical::<f64>(scale);
    let key = monitor
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("{}:{}", origin.x as i64, origin.y as i64));
    Ok(Some((
        Rect {
            x: origin.x,
            y: origin.y,
            w: area.width,
            h: area.height,
        },
        scale,
        key,
    )))
}

/// Bounds used by the centered pinned pill.
///
/// Windows uses the full monitor so an auto-hidden/top taskbar reservation
/// cannot create a visible gap. Other platforms keep using the system work
/// area; on macOS that is the future-safe boundary below the menu bar/notch.
fn pinned_top_rect<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<Option<Rect>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(None);
    };
    let scale = monitor.scale_factor();

    #[cfg(target_os = "windows")]
    let (origin, area) = (
        monitor.position().to_logical::<f64>(scale),
        monitor.size().to_logical::<f64>(scale),
    );

    #[cfg(not(target_os = "windows"))]
    let (origin, area) = {
        let work = monitor.work_area();
        (
            work.position.to_logical::<f64>(scale),
            work.size.to_logical::<f64>(scale),
        )
    };

    Ok(Some(Rect {
        x: origin.x,
        y: origin.y,
        w: area.width,
        h: area.height,
    }))
}

pub fn apply_side_layout<R: Runtime>(
    window: &WebviewWindow<R>,
    side: DockSide,
    along: Option<f64>,
    layout: PanelLayout,
    dynamic_island_compatible: bool,
) -> tauri::Result<(f64, f64)> {
    let Some((work, _, _)) = work_rect(window)? else {
        return Ok((0.0, 0.0));
    };
    let (width, height) = docking::size_for(side, layout, dynamic_island_compatible);
    let along = along.unwrap_or_else(|| docking::default_along(work, side, width, height));
    let (x, y) = if layout.is_pinned() {
        let bounds = pinned_top_rect(window)?.unwrap_or(work);
        docking::pinned_top_position(bounds, width)
    } else {
        docking::docked_position(work, side, along, width, height)
    };
    platform::windows::set_window_bounds(window, x, y, width, height)?;
    Ok((x, y))
}

pub fn apply_layout(app: &AppHandle, layout: PanelLayout) -> tauri::Result<()> {
    apply_layout_at(app, layout, None, None)
}

pub fn apply_layout_at(
    app: &AppHandle,
    layout: PanelLayout,
    at_physical: Option<(f64, f64)>,
    dynamic_island_override: Option<bool>,
) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    let Some((work, scale, key)) = work_rect(&window)? else {
        return Ok(());
    };
    let pos = live_logical_pos(app, &window, scale, at_physical)?;
    let size = window.outer_size()?.to_logical::<f64>(scale);
    let win = Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    let side = if layout.is_pinned() {
        DockSide::Top
    } else {
        docking::nearest_side(work, win)
    };
    let along = docking::along_axis(side, win.x, win.y);
    let (x, y) = apply_side_layout(
        &window,
        side,
        Some(along),
        layout,
        dynamic_island_override.unwrap_or_else(|| dynamic_island_compatible(app)),
    )?;
    remember_logical(app, x, y, scale);
    persist_dock(app, &key, side, docking::along_axis(side, x, y));
    let _ = app.emit(
        "dock-changed",
        DockChanged {
            side,
            along: docking::along_axis(side, x, y),
            preview: false,
            x,
            y,
        },
    );
    Ok(())
}

pub fn current_side_along(
    app: &AppHandle,
    window: &WebviewWindow,
) -> tauri::Result<(DockSide, f64)> {
    let Some((work, scale, key)) = work_rect(window)? else {
        return Ok((DockSide::Top, 0.0));
    };
    let settings = app.state::<Mutex<PersistedSettings>>();
    let settings = settings.lock().expect("settings");
    if let Some(saved) = settings.monitor_docks.get(&key) {
        return Ok((saved.side, saved.along));
    }
    let side = settings.app.dock_side;
    let (width, height) = docking::collapsed_size(side);
    let along = docking::default_along(work, side, width, height);
    let _ = scale;
    Ok((side, along))
}

pub fn resize_panel(app: &AppHandle, width: f64, height: f64) -> tauri::Result<()> {
    apply_layout(app, layout_from_size(DockSide::Top, width, height))
}

pub fn move_panel(app: &AppHandle, x: f64, y: f64) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    platform::windows::set_window_position(&window, x, y)?;
    remember_logical(app, x, y, window.scale_factor()?);
    Ok(())
}

fn layout_from_size(_side: DockSide, width: f64, height: f64) -> PanelLayout {
    if height >= 280.0 {
        PanelLayout::Expanded
    } else if width >= 160.0 {
        PanelLayout::Peek
    } else {
        PanelLayout::Collapsed
    }
}

pub fn dock_after_drag(app: &AppHandle) -> tauri::Result<DockChanged> {
    dock_after_drag_at(app, None, true)
}

pub fn dock_after_drag_at(
    app: &AppHandle,
    at_physical: Option<(f64, f64)>,
    relocate: bool,
) -> tauri::Result<DockChanged> {
    let fallback = DockChanged {
        side: DockSide::Top,
        along: 0.0,
        preview: false,
        x: 0.0,
        y: 0.0,
    };
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(fallback);
    };
    let Some((work, scale, key)) = work_rect(&window)? else {
        return Ok(fallback);
    };
    let pos = live_logical_pos(app, &window, scale, at_physical)?;
    let size = window.outer_size()?.to_logical::<f64>(scale);
    let win = Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    let target = docking::snap_target(work, win, true);
    if relocate {
        commit_dock(app, &window, &key, work, target)?;
        remember_logical(app, target.x, target.y, scale);
    } else {
        persist_dock(
            app,
            &key,
            target.side,
            docking::along_axis(target.side, target.x, target.y),
        );
        if size.width > target.width + 1.0 || size.height > target.height + 1.0 {
            platform::windows::set_window_bounds(
                &window,
                pos.x,
                pos.y,
                target.width,
                target.height,
            )?;
        }
        remember_logical(app, pos.x, pos.y, scale);
    }
    let along = docking::along_axis(target.side, target.x, target.y);
    let payload = DockChanged {
        side: target.side,
        along,
        preview: false,
        x: target.x,
        y: target.y,
    };
    let _ = app.emit("dock-changed", &payload);
    Ok(payload)
}

pub fn preview_dock(app: &AppHandle) -> tauri::Result<Option<DockSide>> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(None);
    };
    let Some((work, scale, _)) = work_rect(&window)? else {
        return Ok(None);
    };
    let pos = live_logical_pos(app, &window, scale, None)?;
    let size = window.outer_size()?.to_logical::<f64>(scale);
    let win = Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    Ok(docking::preview_side(work, win, SNAP_THRESHOLD))
}

/// Return the centered safe-top target used by the pin animation.
pub fn top_pin_target(
    app: &AppHandle,
    layout: PanelLayout,
    dynamic_island_override: Option<bool>,
) -> tauri::Result<DockChanged> {
    let fallback = DockChanged {
        side: DockSide::Top,
        along: 0.0,
        preview: false,
        x: 0.0,
        y: 0.0,
    };
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(fallback);
    };
    let Some(bounds) = pinned_top_rect(&window)? else {
        return Ok(fallback);
    };
    let compatible = dynamic_island_override.unwrap_or_else(|| dynamic_island_compatible(app));
    let (width, _) = docking::size_for(DockSide::Top, layout, compatible);
    let (x, y) = docking::pinned_top_position(bounds, width);
    Ok(DockChanged {
        side: DockSide::Top,
        along: x,
        preview: false,
        x,
        y,
    })
}

/// Resize a pinned panel around the monitor's top-center anchor without
/// consulting mutable settings. Used by the UI's synchronized width animation.
pub fn resize_pinned_panel(app: &AppHandle, width: f64, height: f64) -> tauri::Result<DockChanged> {
    let fallback = DockChanged {
        side: DockSide::Top,
        along: 0.0,
        preview: false,
        x: 0.0,
        y: 0.0,
    };
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(fallback);
    };
    let Some(bounds) = pinned_top_rect(&window)? else {
        return Ok(fallback);
    };
    if !width.is_finite()
        || !height.is_finite()
        || width < docking::ICON_SIZE
        || height < docking::ICON_SIZE
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid pinned panel size",
        )
        .into());
    }
    let width = width.min(bounds.w.max(docking::ICON_SIZE));
    let height = height.min(bounds.h.max(docking::ICON_SIZE));
    let (x, y) = docking::pinned_top_position(bounds, width);
    platform::windows::set_window_bounds(&window, x, y, width, height)?;
    remember_logical(app, x, y, window.scale_factor()?);
    Ok(DockChanged {
        side: DockSide::Top,
        along: x,
        preview: false,
        x,
        y,
    })
}

fn persist_dock(app: &AppHandle, key: &str, side: DockSide, along: f64) {
    if let Some(state) = app.try_state::<Mutex<PersistedSettings>>() {
        let mut settings = state.lock().expect("settings");
        settings.app.dock_side = side;
        settings
            .monitor_docks
            .insert(key.to_string(), MonitorDock { side, along });
        settings_store::save(&settings);
    }
}

fn commit_dock(
    app: &AppHandle,
    window: &WebviewWindow,
    key: &str,
    work: Rect,
    target: DockTarget,
) -> tauri::Result<()> {
    platform::windows::set_window_bounds(window, target.x, target.y, target.width, target.height)?;
    let along = docking::along_axis(target.side, target.x, target.y);
    let _ = work;
    persist_dock(app, key, target.side, along);
    Ok(())
}

pub fn set_dock_side(app: &AppHandle, side: DockSide) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    let Some((work, scale, key)) = work_rect(&window)? else {
        return Ok(());
    };
    let pinned = platform::windows::pinned_top_guard_enabled();
    let side = if pinned { DockSide::Top } else { side };
    let layout = if pinned {
        PanelLayout::PinnedCollapsed
    } else {
        PanelLayout::Collapsed
    };
    let (width, height) = docking::collapsed_size(side);
    let along = docking::default_along(work, side, width, height);
    let (x, y) = apply_side_layout(
        &window,
        side,
        Some(along),
        layout,
        dynamic_island_compatible(app),
    )?;
    remember_logical(app, x, y, scale);
    if let Some(state) = app.try_state::<Mutex<PersistedSettings>>() {
        let mut settings = state.lock().expect("settings");
        settings.app.dock_side = side;
        settings
            .monitor_docks
            .insert(key, MonitorDock { side, along });
        settings_store::save(&settings);
    }
    let _ = app.emit(
        "dock-changed",
        DockChanged {
            side,
            along,
            preview: false,
            x,
            y,
        },
    );
    Ok(())
}

pub fn toggle_visible(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    if window.is_visible()? {
        window.hide()?;
    } else {
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

pub fn show_and_focus(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    window.show()?;
    window.set_focus()?;
    Ok(())
}

pub fn set_always_on_top(app: &AppHandle, on_top: bool) -> tauri::Result<()> {
    if on_top {
        set_panel_pinned(app, true)?;
    } else if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        // Turning off the manual preference does not necessarily mean the
        // effective pin is off: a running task may still own a temporary pin.
        // Keep the guard unchanged until the UI resolves that policy and calls
        // `set_panel_pinned`, while preserving the overlay's regular topmost
        // visibility contract.
        window.set_always_on_top(overlay_native_topmost(false))?;
    }
    if let Some(state) = app.try_state::<Mutex<PersistedSettings>>() {
        let mut settings = state.lock().expect("settings");
        settings.app.always_on_top = on_top;
        settings_store::save(&settings);
    }
    Ok(())
}

/// Synchronize the effective panel pin without changing the persisted manual
/// preference. Automatic running-task pinning uses this path.
pub fn set_panel_pinned(app: &AppHandle, pinned: bool) -> tauri::Result<()> {
    platform::windows::set_pinned_top_guard(pinned);
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    window.set_always_on_top(overlay_native_topmost(pinned))?;
    Ok(())
}

pub fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        existing.show()?;
        existing.set_focus()?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html#settings".into()),
    )
    .title("SpringCat 设置")
    .inner_size(960.0, 700.0)
    .min_inner_size(820.0, 620.0)
    .resizable(true)
    .decorations(true)
    .additional_browser_args(WEBVIEW_BROWSER_ARGS)
    .skip_taskbar(false)
    .always_on_top(false)
    .visible(true)
    .build()?;
    Ok(())
}

pub fn expand_and_focus(app: &AppHandle) {
    let _ = show_and_focus(app);
    let pinned = platform::windows::pinned_top_guard_enabled();
    let layout = if pinned {
        PanelLayout::PinnedExpanded
    } else {
        PanelLayout::Expanded
    };
    let _ = apply_layout(app, layout);
    let _ = app.emit("panel-layout", PanelLayout::Expanded.as_str());
    event_collector::emit_tasks(app);
}

#[cfg(test)]
mod tests {
    use super::{layout_from_size, overlay_native_topmost};
    use crate::docking::PanelLayout;
    use crate::domain::DockSide;

    #[test]
    fn expanded_from_large_height() {
        assert_eq!(
            layout_from_size(DockSide::Top, 420.0, 520.0),
            PanelLayout::Expanded
        );
    }

    #[test]
    fn unpinned_overlay_remains_visible_above_regular_windows() {
        assert!(overlay_native_topmost(false));
        assert!(overlay_native_topmost(true));
    }
}
