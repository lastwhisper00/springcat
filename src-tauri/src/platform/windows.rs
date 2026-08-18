//! Windows work-area, taskbar, and z-order helpers.

use tauri::{LogicalPosition, LogicalSize, Runtime, WebviewWindow};

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;

static PINNED_TOP_GUARD: AtomicBool = AtomicBool::new(false);

/// Keep WebView2's backing surface at the full drawer height. The outer HWND
/// clips this surface to the current animated panel height, so expanding the
/// drawer reveals already-rendered pixels instead of reallocating WebView2's
/// transparent composition surface on every `WM_SIZE`.
const WEBVIEW_SURFACE_HEIGHT: f64 = 448.0;

/// Only the final orb-sized window keeps the previous wide WebView allocation.
/// Intermediate contractions (for example drawer -> pill) must resize the
/// surface width as well, otherwise a right-anchored parent exposes the right
/// slice of the old surface while the orb remains clipped off its left edge.
const COLLAPSED_SURFACE_SIZE: f64 = 48.0;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparedSurface {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
static PREPARED_SURFACE: Mutex<Option<PreparedSurface>> = Mutex::new(None);

#[cfg(target_os = "windows")]
static WEBVIEW_SURFACE_HOST: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
struct PendingSurfacePosition {
    host: isize,
    screen_x: i32,
    screen_y: i32,
    width: i32,
    height: i32,
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
        // every `WM_SIZE`. SpringCat deliberately keeps a full-height backing
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
        if let Some(surface) = PENDING_SURFACE_POSITION
            .lock()
            .expect("pending WebView surface position")
            .take()
        {
            let surface_host =
                windows::Win32::Foundation::HWND(surface.host as *mut core::ffi::c_void);
            let _ = unsafe {
                SetWindowPos(
                    surface_host,
                    None,
                    surface.screen_x - position.x,
                    surface.screen_y - position.y,
                    surface.width,
                    surface.height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            };
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
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW.0 as isize);
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

/// Move and resize in one native operation so left/right dock animations have
/// identical anchoring. Other platforms use the regular Tauri fallback.
pub fn set_window_bounds<R: Runtime>(
    window: &WebviewWindow<R>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOZORDER,
        };

        let scale = window.scale_factor()?;
        let current = window.outer_size()?.to_logical::<f64>(scale);
        let prepare_surface_first =
            surface_must_grow_first(current.width, current.height, width, height);
        let surface_contracts = surface_must_contract(current.width, current.height, width, height);
        let preserve_surface = surface_contracts && is_collapsed_surface_target(width, height);
        let pinned = PINNED_TOP_GUARD.load(Ordering::Relaxed);
        let insert_after = pinned.then_some(HWND_TOPMOST);
        let flags = if pinned {
            SWP_NOACTIVATE
        } else {
            SWP_NOACTIVATE | SWP_NOZORDER
        };
        let target = physical_surface_bounds(scale, x, y, width, height);
        let surface_prepared = take_prepared_surface(target);
        // A prepared opening surface and a contracting surface already contain
        // the pixels that must be visible after the parent moves. Reposition
        // that existing surface inside the parent's pending native transaction
        // and keep its allocation intact. Resizing WebView2 here produces a
        // one-frame transparent composition surface at the end of closing.
        if surface_prepared || preserve_surface {
            position_window_over_existing_surface(window, target, insert_after, flags)?;
            return Ok(());
        }
        // A drawer-to-pill fold still contracts the native parent, but unlike
        // the final pill-to-orb close it must also trim the WebView width. Put
        // that resized surface at the future parent origin first, then commit
        // both child and parent coordinates through the same native frame.
        if surface_contracts {
            let current_position = window.outer_position()?;
            let (offset_x, offset_y) =
                aligned_surface_offset(current_position.x, current_position.y, target.x, target.y);
            set_webview_surface(window, width, height, offset_x, offset_y)?;
            position_window_over_existing_surface(window, target, insert_after, flags)?;
            return Ok(());
        }
        // Discrete UI expansion normally arrives with a surface that has
        // already painted at the target coordinates. Keep this immediate grow
        // as a fallback for native-only callers that did not prepaint.
        if prepare_surface_first {
            set_webview_surface(window, width, height, 0, 0)?;
        }
        let result = unsafe {
            SetWindowPos(
                window.hwnd()?,
                insert_after,
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                (width * scale).round() as i32,
                (height * scale).round() as i32,
                flags,
            )
        };
        if result.is_ok() {
            // Keep the native HWND rectangular and let the transparent WebView/CSS
            // draw the shape. SetWindowRgn is a 1-bit clip on Windows; at high DPI
            // it leaves stair-stepped backing-surface pixels around the breathing orb.
            return Ok(());
        }
    }

    window.set_size(LogicalSize::new(width, height))?;
    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}

/// Resize the WebView2 surface while the parent HWND still has its collapsed
/// bounds. The child host is shifted by the future parent delta, so the current
/// 48 px clip continues to show the exact same screen-space orb while WebView2
/// gets a compositor frame to render the full target layout.
pub fn prepare_window_bounds<R: Runtime>(
    window: &WebviewWindow<R>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let scale = window.scale_factor()?;
        let current = window.outer_position()?;
        let target = physical_surface_bounds(scale, x, y, width, height);
        let (offset_x, offset_y) = aligned_surface_offset(current.x, current.y, target.x, target.y);
        set_webview_surface(window, width, height, offset_x, offset_y)?;
        *PREPARED_SURFACE.lock().expect("prepared WebView surface") = Some(target);
    }

    Ok(())
}

fn surface_must_grow_first(
    current_width: f64,
    current_height: f64,
    next_width: f64,
    next_height: f64,
) -> bool {
    next_width > current_width + 0.5 || next_height > current_height + 0.5
}

fn surface_must_contract(
    current_width: f64,
    current_height: f64,
    next_width: f64,
    next_height: f64,
) -> bool {
    next_width + 0.5 < current_width || next_height + 0.5 < current_height
}

fn is_collapsed_surface_target(width: f64, height: f64) -> bool {
    width <= COLLAPSED_SURFACE_SIZE + 0.5 && height <= COLLAPSED_SURFACE_SIZE + 0.5
}

fn webview_surface_size(width: f64, height: f64) -> (f64, f64) {
    (width, height.max(WEBVIEW_SURFACE_HEIGHT))
}

fn aligned_surface_offset(
    current_x: i32,
    current_y: i32,
    target_x: i32,
    target_y: i32,
) -> (i32, i32) {
    (target_x - current_x, target_y - current_y)
}

#[cfg(target_os = "windows")]
fn physical_surface_bounds(scale: f64, x: f64, y: f64, width: f64, height: f64) -> PreparedSurface {
    PreparedSurface {
        x: (x * scale).round() as i32,
        y: (y * scale).round() as i32,
        width: (width * scale).round() as i32,
        height: (height * scale).round() as i32,
    }
}

#[cfg(target_os = "windows")]
fn take_prepared_surface(target: PreparedSurface) -> bool {
    PREPARED_SURFACE
        .lock()
        .expect("prepared WebView surface")
        .take()
        .is_some_and(|prepared| prepared == target)
}

#[cfg(target_os = "windows")]
fn position_window_over_existing_surface<R: Runtime>(
    window: &WebviewWindow<R>,
    target: PreparedSurface,
    insert_after: Option<windows::Win32::Foundation::HWND>,
    flags: windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS,
) -> tauri::Result<()> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, IsWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
    };

    let parent = window.hwnd()?;
    let surface_host_raw = WEBVIEW_SURFACE_HOST.load(Ordering::Acquire);
    let controller_host = HWND(surface_host_raw as *mut core::ffi::c_void);
    unsafe {
        let mut surface_rect = RECT::default();
        let has_surface = surface_host_raw != 0
            && IsWindow(Some(controller_host)).as_bool()
            && GetWindowRect(controller_host, &mut surface_rect).is_ok();

        // Keep the already-painted surface at the same screen-space origin.
        // For a prepared opening that origin is the target origin, producing
        // offset (0, 0). For closing it becomes a negative child offset and the
        // smaller parent simply clips the old surface around the resting orb.
        let pending_surface = has_surface.then_some(PendingSurfacePosition {
            host: surface_host_raw,
            screen_x: surface_rect.left,
            screen_y: surface_rect.top,
            width: surface_rect.right - surface_rect.left,
            height: surface_rect.bottom - surface_rect.top,
        });
        *PENDING_SURFACE_POSITION
            .lock()
            .expect("pending WebView surface position") = pending_surface;

        let result = SetWindowPos(
            parent,
            insert_after,
            target.x,
            target.y,
            target.width,
            target.height,
            flags,
        );

        if result.is_ok() {
            // WM_WINDOWPOSCHANGING normally consumes the pending position. Keep
            // a synchronous fallback for unusual window styles that suppress
            // that message.
            if let Some(surface) = PENDING_SURFACE_POSITION
                .lock()
                .expect("pending WebView surface position")
                .take()
            {
                let _ = SetWindowPos(
                    controller_host,
                    None,
                    surface.screen_x - target.x,
                    surface.screen_y - target.y,
                    surface.width,
                    surface.height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            return Ok(());
        }
        PENDING_SURFACE_POSITION
            .lock()
            .expect("pending WebView surface position")
            .take();
    }

    window.set_size(LogicalSize::new(
        target.width as f64 / window.scale_factor()?,
        target.height as f64 / window.scale_factor()?,
    ))?;
    window.set_position(LogicalPosition::new(
        target.x as f64 / window.scale_factor()?,
        target.y as f64 / window.scale_factor()?,
    ))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_webview_surface<R: Runtime>(
    window: &WebviewWindow<R>,
    width: f64,
    height: f64,
    offset_x: i32,
    offset_y: i32,
) -> tauri::Result<()> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

    let scale = window.scale_factor()?;
    let (surface_width, surface_height) = webview_surface_size(width, height);
    let physical_width = (surface_width * scale).round() as i32;
    let physical_height = (surface_height * scale).round() as i32;

    window.with_webview(move |webview| unsafe {
        let controller = webview.controller();
        let _ = controller.SetBounds(RECT {
            left: 0,
            top: 0,
            right: physical_width,
            bottom: physical_height,
        });

        // Wry resizes this controller host asynchronously from its `WM_SIZE`
        // handler. Override it synchronously before DWM commits the parent
        // frame, keeping the transparent backing surface fully allocated.
        let mut controller_host = HWND::default();
        if controller.ParentWindow(&mut controller_host).is_ok() {
            WEBVIEW_SURFACE_HOST.store(controller_host.0 as isize, Ordering::Release);
            let _ = SetWindowPos(
                controller_host,
                None,
                offset_x,
                offset_y,
                physical_width,
                physical_height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        aligned_surface_offset, is_collapsed_surface_target, pinned_top_guard_enabled,
        preferred_product_window, set_pinned_top_guard, surface_must_contract,
        surface_must_grow_first, webview_surface_size,
    };

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
    fn keeps_the_webview_surface_ready_while_the_native_window_clips_it() {
        assert_eq!(webview_surface_size(360.0, 48.0), (360.0, 448.0));
        assert_eq!(webview_surface_size(360.0, 448.0), (360.0, 448.0));
    }

    #[test]
    fn grows_the_webview_surface_before_exposing_a_larger_parent() {
        assert!(surface_must_grow_first(48.0, 48.0, 360.0, 448.0));
        assert!(surface_must_grow_first(360.0, 48.0, 520.0, 48.0));
        assert!(!surface_must_grow_first(360.0, 448.0, 48.0, 48.0));
        assert!(!surface_must_grow_first(360.0, 448.0, 360.0, 48.0));
    }

    #[test]
    fn preserves_the_existing_surface_when_the_parent_contracts() {
        assert!(surface_must_contract(360.0, 448.0, 48.0, 48.0));
        assert!(surface_must_contract(360.0, 448.0, 360.0, 48.0));
        assert!(!surface_must_contract(48.0, 48.0, 360.0, 448.0));
        assert!(!surface_must_contract(360.0, 48.0, 360.0, 48.0));
    }

    #[test]
    fn preserves_a_wide_surface_only_for_the_final_orb() {
        assert!(is_collapsed_surface_target(48.0, 48.0));
        assert!(!is_collapsed_surface_target(268.0, 48.0));
        assert!(!is_collapsed_surface_target(48.0, 448.0));
    }

    #[test]
    fn aligns_a_prepainted_surface_with_its_future_screen_origin() {
        assert_eq!(aligned_surface_offset(1100, 56, 788, 56), (-312, 0));
        assert_eq!(aligned_surface_offset(56, 400, 56, 120), (0, -280));
    }
}
