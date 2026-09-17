//! Windows work-area, taskbar, and z-order helpers.

use crate::domain::DockSide;
#[cfg(not(target_os = "windows"))]
use tauri::LogicalSize;
use tauri::{LogicalPosition, Runtime, WebviewWindow};

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;

static PINNED_TOP_GUARD: AtomicBool = AtomicBool::new(false);

/// One canvas for every shape. Its center is the resting orb's fixed anchor;
/// the small outer HWND clips it without changing the WebView viewport.
/// Two maximum capsule widths leave room to grow toward either dock edge.
const WEBVIEW_SURFACE_WIDTH: f64 = 1040.0;
const WEBVIEW_SURFACE_HEIGHT: f64 = 420.0;
const WEBVIEW_ORB_ANCHOR: f64 = WEBVIEW_SURFACE_WIDTH / 2.0;
const OUTER_ORB_INSET: f64 = 24.0;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
struct FixedSurface {
    parent: isize,
    host: isize,
    width: i32,
    height: i32,
    logical_height: f64,
}

#[cfg(target_os = "windows")]
static WEBVIEW_SURFACE: Mutex<Option<FixedSurface>> = Mutex::new(None);

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
struct PendingSurfacePosition {
    host: isize,
    screen_x: i32,
    screen_y: i32,
}

#[cfg(target_os = "windows")]
static PENDING_SURFACE_POSITION: Mutex<Option<PendingSurfacePosition>> = Mutex::new(None);

#[cfg(target_os = "windows")]
static PINNED_TOP_WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
const OVERLAY_SUBCLASS_ID: usize = 0x5350_4341;

#[cfg(target_os = "windows")]
const PINNED_TOP_WATCHDOG_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

#[cfg(target_os = "windows")]
unsafe extern "system" fn overlay_subclass_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::DefSubclassProc;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WINDOWPOS, WM_SIZE, WM_WINDOWPOSCHANGING,
    };

    if message == WM_SIZE {
        // Wry's parent subclass normally resizes WebView2 to the outer HWND on
        // every `WM_SIZE`. SpringCat deliberately keeps a fixed backing
        // surface and clips it with the outer window, so forwarding this
        // message would shrink and regrow the surface once per animation frame.
        return windows::Win32::Foundation::LRESULT(0);
    }

    if message == WM_WINDOWPOSCHANGING {
        let position = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
        if PINNED_TOP_GUARD.load(Ordering::Relaxed) && position.flags.0 & SWP_NOMOVE.0 == 0 {
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() }
                && info.rcWork.top != info.rcMonitor.top
            {
                let width = if position.flags.0 & SWP_NOSIZE.0 != 0 {
                    let mut rect = RECT::default();
                    if unsafe { GetWindowRect(hwnd, &mut rect).is_ok() } {
                        rect.right - rect.left
                    } else {
                        position.cx
                    }
                } else {
                    position.cx
                };
                let centered_x = info.rcMonitor.left
                    + (info.rcMonitor.right - info.rcMonitor.left - width).max(0) / 2;

                // Windows periodically normalizes a centered top overlay to the
                // taskbar work area. Block only that exact system correction;
                // arbitrary coordinates used by the user's drag remain valid.
                if (position.x - centered_x).abs() <= 1 && position.y == info.rcWork.top {
                    position.y = info.rcMonitor.top;
                }
            }
        }

        // A child HWND normally follows its parent only after the parent move
        // has already reached DWM. Rebase the existing WebView host while the
        // parent's WINDOWPOS is still pending, so both coordinates become
        // visible in one composition frame instead of exposing a blank clip in
        // between.
        //
        // Consume the pending rebase ONLY for real moves/resizes. Other
        // SetWindowPos calls on this HWND — most importantly the topmost
        // watchdog, which fires every 100 ms with SWP_NOMOVE | SWP_NOSIZE —
        // also pass through here; letting one of those steal the pending
        // rebase repositioned the WebView host against garbage WINDOWPOS
        // coordinates and left the panel showing a blank slice.
        let carries_position = position.flags.0 & (SWP_NOMOVE.0 | SWP_NOSIZE.0) == 0;
        if carries_position {
            let pending = PENDING_SURFACE_POSITION
                .lock()
                .expect("pending WebView surface position")
                .take();
            if let Some(surface) = pending {
                let surface_host =
                    windows::Win32::Foundation::HWND(surface.host as *mut core::ffi::c_void);
                let result = unsafe {
                    SetWindowPos(
                        surface_host,
                        None,
                        surface.screen_x - position.x,
                        surface.screen_y - position.y,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
                    )
                };
                if let Err(error) = result {
                    tracing::error!(target: "springcat_native", %error, "surface rebase failed; retrying after outer commit");
                    *PENDING_SURFACE_POSITION
                        .lock()
                        .expect("pending WebView surface position") = Some(surface);
                }
            }
        }
    }

    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub fn set_pinned_top_guard(enabled: bool) {
    PINNED_TOP_GUARD.store(enabled, Ordering::Relaxed);
}

/// Return the effective runtime pin state. This deliberately differs from the
/// persisted manual setting while a running task temporarily pins the panel.
pub fn pinned_top_guard_enabled() -> bool {
    PINNED_TOP_GUARD.load(Ordering::Relaxed)
}

/// Keep the overlay at the head of the topmost band while it is pinned.
///
/// Third-party desktop bars can also use `WS_EX_TOPMOST` and periodically put
/// themselves back at the head of that band. A one-time `SetWindowPos` during
/// layout changes therefore is not enough: the bar can cover the upper part of
/// an otherwise stationary pill later. Reasserting z-order does not move,
/// resize, activate, or show the window, and is disabled with the pin setting.
#[cfg(target_os = "windows")]
fn start_pinned_top_watchdog(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindow, IsWindowVisible, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE,
    };

    if PINNED_TOP_WATCHDOG_STARTED.swap(true, Ordering::Relaxed) {
        return;
    }

    // `HWND` wraps a raw pointer and cannot be sent between threads directly.
    // Store its stable numeric value and reconstruct the non-owning handle in
    // the watchdog thread; `IsWindow` terminates the loop after destruction.
    let raw_hwnd = hwnd.0 as isize;
    let _ = std::thread::Builder::new()
        .name("springcat-topmost-guard".into())
        .spawn(move || loop {
            std::thread::sleep(PINNED_TOP_WATCHDOG_INTERVAL);
            let hwnd = HWND(raw_hwnd as *mut core::ffi::c_void);
            if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
                break;
            }
            if !PINNED_TOP_GUARD.load(Ordering::Relaxed)
                || !unsafe { IsWindowVisible(hwnd).as_bool() }
            {
                continue;
            }

            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                )
            };
        });
}

/// Mark the frameless overlay as a native tool window. `skip_taskbar` alone
/// does not set this style in Tauri; without it Windows Shell periodically
/// pushes a top-edge window below a taskbar positioned at the top of screen.
pub fn configure_overlay_window<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::SetWindowSubclass;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_TOOLWINDOW,
        };

        let hwnd = window.hwnd()?;
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                style | WS_EX_TOOLWINDOW.0 as isize,
            );
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
            let _ = SetWindowSubclass(hwnd, Some(overlay_subclass_proc), OVERLAY_SUBCLASS_ID, 0);
        }
        start_pinned_top_watchdog(hwnd);
    }

    Ok(())
}

/// Bring an already-open desktop app window to the foreground without sending
/// it a protocol URL or command. This deliberately avoids changing app state.
#[cfg(target_os = "windows")]
pub fn focus_existing_process_window(executable_name: &str, product_name: &str) -> bool {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
        SW_SHOW,
    };

    struct Search<'a> {
        executable_name: &'a str,
        product_name: &'a str,
        preferred: Option<HWND>,
        fallback: Option<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, parameter: LPARAM) -> BOOL {
        let search = unsafe { &mut *(parameter.0 as *mut Search<'_>) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool()
            || !window_belongs_to_process(hwnd, search.executable_name)
        {
            return BOOL(1);
        }

        if search.fallback.is_none() {
            search.fallback = Some(hwnd);
        }
        let title = window_title(hwnd);
        if preferred_product_window(&title, search.product_name) {
            search.preferred = Some(hwnd);
        }
        BOOL(1)
    }

    let mut search = Search {
        executable_name,
        product_name,
        preferred: None,
        fallback: None,
    };
    let _ = unsafe {
        EnumWindows(
            Some(visit),
            LPARAM((&mut search as *mut Search<'_>) as isize),
        )
    };
    let Some(hwnd) = search.preferred.or(search.fallback) else {
        return false;
    };

    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = SetForegroundWindow(hwnd);
    }
    true
}

#[cfg(target_os = "windows")]
fn window_belongs_to_process(hwnd: windows::Win32::Foundation::HWND, expected: &str) -> bool {
    use std::os::windows::ffi::OsStringExt;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id == 0 {
        return false;
    }
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return false;
    };
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    if queried.is_err() {
        return false;
    }
    let path = std::ffi::OsString::from_wide(&buffer[..length as usize]);
    std::path::Path::new(&path)
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(expected))
}

#[cfg(target_os = "windows")]
fn window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
}

fn preferred_product_window(title: &str, product_name: &str) -> bool {
    title.eq_ignore_ascii_case(product_name)
        || title
            .to_ascii_lowercase()
            .ends_with(&format!(" - {}", product_name.to_ascii_lowercase()))
}

#[cfg(not(target_os = "windows"))]
pub fn focus_existing_process_window(_executable_name: &str, _product_name: &str) -> bool {
    false
}

/// Move without routing through Tauri's Windows work-area normalization. This
/// lets a pinned panel animate all the way to the physical monitor top.
pub fn set_window_position<R: Runtime>(
    window: &WebviewWindow<R>,
    x: f64,
    y: f64,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };

        let scale = window.scale_factor()?;
        // Top app bars (for example MyFinder) can sit above an existing
        // WS_EX_TOPMOST window. Reinsert the pinned overlay at the head of the
        // topmost band whenever it moves so the physical y=0 placement remains
        // fully visible instead of being covered by the reserved work area.
        let pinned = PINNED_TOP_GUARD.load(Ordering::Relaxed);
        let insert_after = pinned.then_some(HWND_TOPMOST);
        let flags = if pinned {
            SWP_NOACTIVATE | SWP_NOSIZE
        } else {
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER
        };
        let result = unsafe {
            SetWindowPos(
                window.hwnd()?,
                insert_after,
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                0,
                0,
                flags,
            )
        };
        if result.is_ok() {
            return Ok(());
        }
    }

    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}

/// Move and resize the outer clip, keeping the WebView viewport unchanged.
/// Other platforms keep the regular Tauri resize behavior.
pub fn set_window_bounds<R: Runtime>(
    window: &WebviewWindow<R>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    side: DockSide,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOZORDER};

        let scale = window.scale_factor()?;
        let surface = ensure_webview_surface(window, height, side)?;
        let pinned = PINNED_TOP_GUARD.load(Ordering::Relaxed);
        let insert_after = pinned.then_some(HWND_TOPMOST);
        let flags = if pinned {
            SWP_NOACTIVATE
        } else {
            SWP_NOACTIVATE | SWP_NOZORDER
        };
        let target = physical_window_bounds(scale, x, y, width, height);
        let offset_x = physical_surface_offset(side, width, scale);
        tracing::info!(
            target: "springcat_native", ?side, x = target.x, y = target.y,
            width = target.width, height = target.height, offset_x,
            backing_width = surface.width, backing_height = surface.height,
            "commit outer clip; backing unchanged"
        );
        return position_window_over_existing_surface(
            window,
            surface.host,
            target,
            offset_x,
            insert_after,
            flags,
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = side;
        window.set_size(LogicalSize::new(width, height))?;
        window.set_position(LogicalPosition::new(x, y))?;
        Ok(())
    }
}

/// Ensure the fixed canvas exists without rebasing a currently visible frame.
/// Once initialized, ordinary orb/pill/drawer preparation does no native work.
pub fn prepare_window_bounds<R: Runtime>(
    window: &WebviewWindow<R>,
    _x: f64,
    _y: f64,
    _width: f64,
    height: f64,
    side: DockSide,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    ensure_webview_surface(window, height, side)?;
    #[cfg(not(target_os = "windows"))]
    let _ = (window, height, side);
    Ok(())
}

fn webview_surface_size(required_height: f64, previous_height: f64) -> (f64, f64) {
    (
        WEBVIEW_SURFACE_WIDTH,
        required_height
            .max(previous_height)
            .max(WEBVIEW_SURFACE_HEIGHT),
    )
}

fn surface_offset(side: DockSide, outer_width: f64) -> f64 {
    let anchor = match side {
        DockSide::Top => outer_width / 2.0,
        DockSide::Left => OUTER_ORB_INSET,
        DockSide::Right => outer_width - OUTER_ORB_INSET,
    };
    anchor - WEBVIEW_ORB_ANCHOR
}

fn physical_surface_offset(side: DockSide, outer_width: f64, scale: f64) -> i32 {
    (surface_offset(side, outer_width) * scale).round() as i32
}

#[cfg(target_os = "windows")]
fn physical_window_bounds(scale: f64, x: f64, y: f64, width: f64, height: f64) -> WindowBounds {
    WindowBounds {
        x: (x * scale).round() as i32,
        y: (y * scale).round() as i32,
        width: (width * scale).round() as i32,
        height: (height * scale).round() as i32,
    }
}

#[cfg(target_os = "windows")]
fn native_surface_error(error: impl std::fmt::Display) -> tauri::Error {
    tracing::error!(target: "springcat_native", %error, "native surface operation failed");
    std::io::Error::other(error.to_string()).into()
}

#[cfg(target_os = "windows")]
fn position_window_over_existing_surface<R: Runtime>(
    window: &WebviewWindow<R>,
    surface_host: isize,
    target: WindowBounds,
    offset_x: i32,
    insert_after: Option<windows::Win32::Foundation::HWND>,
    flags: windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS,
) -> tauri::Result<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };

    let parent = window.hwnd()?;
    // The render anchor is fixed in canvas coordinates. Derive its next
    // screen origin from the requested outer clip, never from stale host
    // coordinates left behind by a previous dock side or drag.
    *PENDING_SURFACE_POSITION
        .lock()
        .expect("pending WebView surface position") = Some(PendingSurfacePosition {
        host: surface_host,
        screen_x: target.x + offset_x,
        screen_y: target.y,
    });

    let result = unsafe {
        SetWindowPos(
            parent,
            insert_after,
            target.x,
            target.y,
            target.width,
            target.height,
            flags,
        )
    };
    let pending = PENDING_SURFACE_POSITION
        .lock()
        .expect("pending WebView surface position")
        .take();
    result.map_err(native_surface_error)?;

    // WM_WINDOWPOSCHANGING normally moves the host before the parent's clip
    // commits. Retry synchronously if that message was omitted or failed.
    if let Some(surface) = pending {
        unsafe {
            SetWindowPos(
                HWND(surface.host as *mut core::ffi::c_void),
                None,
                surface.screen_x - target.x,
                surface.screen_y - target.y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
            )
        }
        .map_err(native_surface_error)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn ensure_webview_surface<R: Runtime>(
    window: &WebviewWindow<R>,
    required_height: f64,
    side: DockSide,
) -> tauri::Result<FixedSurface> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, IsWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
    };

    let scale = window.scale_factor()?;
    let parent_raw = window.hwnd()?.0 as isize;
    let previous = *WEBVIEW_SURFACE.lock().expect("fixed WebView surface");
    let (logical_width, logical_height) = webview_surface_size(
        required_height,
        previous
            .filter(|s| s.parent == parent_raw)
            .map_or(0.0, |s| s.logical_height),
    );
    let physical_width = (logical_width * scale).round() as i32;
    let physical_height = (logical_height * scale).round() as i32;
    if let Some(surface) = previous {
        if surface.parent == parent_raw
            && surface.width == physical_width
            && surface.height == physical_height
            && unsafe { IsWindow(Some(HWND(surface.host as *mut core::ffi::c_void))).as_bool() }
        {
            return Ok(surface);
        }
    }

    let current_width = window.outer_size()?.to_logical::<f64>(scale).width;
    let offset_x = physical_surface_offset(side, current_width, scale);
    // Tauri's dispatcher runs with_webview inline on its UI thread. From a
    // worker it queues the closure; acknowledge actual completion before any
    // outer clip can expose the canvas. The buffered channel cannot block the
    // inline callback, and a bounded receive reports a stalled dispatcher.
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    window.with_webview(move |webview| {
        let initialized = (|| -> windows::core::Result<FixedSurface> {
            let parent = HWND(parent_raw as *mut core::ffi::c_void);
            let surface_host =
                unsafe { FindWindowExW(Some(parent), None, w!("WRY_WEBVIEW"), PCWSTR::null())? };
            let controller = webview.controller();
            let mut current = RECT::default();
            unsafe {
                controller.Bounds(&mut current)?;
            }
            if current.left != 0
                || current.top != 0
                || current.right != physical_width
                || current.bottom != physical_height
            {
                unsafe {
                    controller.SetBounds(RECT {
                        left: 0,
                        top: 0,
                        right: physical_width,
                        bottom: physical_height,
                    })?;
                }
                tracing::info!(
                    target: "springcat_native", physical_width, physical_height, scale,
                    "resize backing: initialization, DPI change, or larger height"
                );
            }
            // The host is sized only with initial allocation/DPI changes. All
            // ordinary commits move it with SWP_NOSIZE.
            unsafe {
                SetWindowPos(
                    surface_host,
                    None,
                    offset_x,
                    0,
                    physical_width,
                    physical_height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
            }
            let surface = FixedSurface {
                parent: parent_raw,
                host: surface_host.0 as isize,
                width: physical_width,
                height: physical_height,
                logical_height,
            };
            *WEBVIEW_SURFACE.lock().expect("fixed WebView surface") = Some(surface);
            Ok(surface)
        })();
        let _ = sender.send(initialized.map_err(|error| error.to_string()));
    })?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .map_err(native_surface_error)?
        .map_err(native_surface_error)
}

#[cfg(test)]
mod tests {
    use super::{
        physical_surface_offset, pinned_top_guard_enabled, preferred_product_window,
        set_pinned_top_guard, surface_offset, webview_surface_size, WEBVIEW_ORB_ANCHOR,
        WEBVIEW_SURFACE_WIDTH,
    };
    use crate::domain::DockSide;

    #[test]
    fn recognizes_primary_cursor_window_titles() {
        assert!(preferred_product_window("guanchaoV3 - Cursor", "Cursor"));
        assert!(preferred_product_window("Cursor", "Cursor"));
        assert!(!preferred_product_window(
            "New account creation request",
            "Cursor"
        ));
    }

    #[test]
    fn tracks_the_effective_runtime_pin_independently() {
        set_pinned_top_guard(false);
        assert!(!pinned_top_guard_enabled());
        set_pinned_top_guard(true);
        assert!(pinned_top_guard_enabled());
        set_pinned_top_guard(false);
    }

    #[test]
    fn all_shapes_reuse_one_backing_canvas() {
        for height in [48.0, 120.0, 280.0, 420.0] {
            assert_eq!(webview_surface_size(height, 0.0), (1040.0, 420.0));
            assert_eq!(webview_surface_size(height, 420.0), (1040.0, 420.0));
        }
        // A future larger drawer may grow the canvas, but folding never shrinks it.
        assert_eq!(webview_surface_size(560.0, 420.0), (1040.0, 560.0));
        assert_eq!(webview_surface_size(48.0, 560.0), (1040.0, 560.0));
    }

    #[test]
    fn collapsed_orb_keeps_the_same_canvas_anchor_when_dragged_between_edges() {
        for side in [DockSide::Top, DockSide::Left, DockSide::Right] {
            assert_eq!(surface_offset(side, 48.0), -496.0);
            assert_eq!(WEBVIEW_ORB_ANCHOR + surface_offset(side, 48.0), 24.0);
        }
    }

    #[test]
    fn settled_cards_fit_the_outer_clip_on_every_dock_and_shape() {
        for side in [DockSide::Top, DockSide::Left, DockSide::Right] {
            for width in [48.0, 268.0, 360.0, 440.0, 520.0] {
                // These are WorkPanel's fixed-native canvas coordinates.
                let card_left = match side {
                    DockSide::Top => WEBVIEW_ORB_ANCHOR - width / 2.0,
                    DockSide::Left => WEBVIEW_ORB_ANCHOR - 24.0,
                    DockSide::Right => WEBVIEW_ORB_ANCHOR + 24.0 - width,
                };
                assert!(card_left >= 0.0);
                assert!(card_left + width <= WEBVIEW_SURFACE_WIDTH);
                let native_left = card_left + surface_offset(side, width);
                assert_eq!(native_left, 0.0);
                assert_eq!(native_left + width, width);
            }
        }
    }

    #[test]
    fn shape_changes_preserve_the_orb_screen_anchor() {
        let screen_anchor = 1280.0;
        for side in [DockSide::Top, DockSide::Left, DockSide::Right] {
            for width in [48.0, 268.0, 360.0, 440.0, 520.0] {
                let outer_x = match side {
                    DockSide::Top => screen_anchor - width / 2.0,
                    DockSide::Left => screen_anchor - 24.0,
                    DockSide::Right => screen_anchor - width + 24.0,
                };
                let orb_screen_x = outer_x + surface_offset(side, width) + WEBVIEW_ORB_ANCHOR;
                assert_eq!(orb_screen_x, screen_anchor);
            }
        }
    }

    #[test]
    fn dpi_rounding_keeps_the_card_and_orb_within_one_physical_pixel() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            for side in [DockSide::Top, DockSide::Left, DockSide::Right] {
                for width in [48.0, 268.0, 311.5, 360.0, 440.0, 520.0] {
                    let offset = physical_surface_offset(side, width, scale) as f64;
                    let card_left = -surface_offset(side, width) * scale;
                    assert!((card_left + offset).abs() <= 0.5);
                    let expected_orb = match side {
                        DockSide::Top => width / 2.0,
                        DockSide::Left => 24.0,
                        DockSide::Right => width - 24.0,
                    } * scale;
                    assert!((WEBVIEW_ORB_ANCHOR * scale + offset - expected_orb).abs() <= 0.5);
                }
            }
        }
    }
}
