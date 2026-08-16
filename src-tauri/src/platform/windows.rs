//! Windows work-area, taskbar, and z-order helpers.

use tauri::{LogicalPosition, LogicalSize, Runtime, WebviewWindow};

use std::sync::atomic::{AtomicBool, Ordering};

static PINNED_TOP_GUARD: AtomicBool = AtomicBool::new(false);

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
        GetWindowRect, SWP_NOMOVE, SWP_NOSIZE, WINDOWPOS, WM_WINDOWPOSCHANGING,
    };

    if message == WM_WINDOWPOSCHANGING && PINNED_TOP_GUARD.load(Ordering::Relaxed) {
        let position = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
        if position.flags.0 & SWP_NOMOVE.0 == 0 {
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
        let pinned = PINNED_TOP_GUARD.load(Ordering::Relaxed);
        let insert_after = pinned.then_some(HWND_TOPMOST);
        let flags = if pinned {
            SWP_NOACTIVATE
        } else {
            SWP_NOACTIVATE | SWP_NOZORDER
        };
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

#[cfg(test)]
mod tests {
    use super::{pinned_top_guard_enabled, preferred_product_window, set_pinned_top_guard};

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
}
