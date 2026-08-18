//! macOS menu-bar window helpers.

use tauri::{Runtime, WebviewWindow};

/// Keep the SpringCat overlay in the same native window band as menu-bar
/// status items. Tauri's regular `always_on_top` maps to AppKit's floating
/// level, which is still below the menu bar and cannot display a window in the
/// notch row even when its coordinates start at the physical screen top.
pub fn configure_overlay_window<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    #[cfg(not(target_os = "macos"))]
    let _ = window;

    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};

        // Tauri exposes NSWindow only with its macOS private API feature. This
        // function is called on the main thread during setup and immediately
        // after any Tauri call that can reset the native level.
        let ns_window = unsafe { &*(window.ns_window()? as *const NSWindow) };
        ns_window.setLevel(NSStatusWindowLevel);
        ns_window.setCollectionBehavior(
            ns_window.collectionBehavior()
                | NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
    }

    Ok(())
}
