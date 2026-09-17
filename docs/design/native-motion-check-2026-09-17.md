# Windows overlay repaint correction

## Actual desktop path

The earlier header-text correction affected the shared Svelte component, but did not remove Windows WebView2 viewport resizing. `prepare_window_bounds` and the drawer-to-capsule commit still called `SetBounds` with different widths. Browser preview checks do not exercise those native calls.

The Windows overlay now uses one 1040 × 420 logical-pixel backing canvas, centered on an orb anchor at x=520. The outer window retains its actual 48px orb, capsule, or 440 × 420 list size. Native child positioning uses `SWP_NOSIZE`. Initialization, DPI changes, or increased canvas height can allocate a new backing; ordinary open/close operations reuse it.

The backend advertises `fixedOverlaySurface` through `app_meta`. Only that native mode uses the corresponding fixed-anchor CSS. This handshake prevents a new frontend from assuming the fixed canvas while an older development backend is still running.

## Desktop verification

Used the Windows computer-use tool against the actual `springcat-ai.exe` window, rather than the localhost demonstration. Temporarily exposed the development window in the taskbar for selection, then restored the original tool-window and skip-taskbar settings and restarted the final build.

- Clicked the real capsule to open the list, folded it, reopened it, and closed through capsule to orb.
- Captured intermediate and settled native window states, including the final 48 × 48 orb window.
- Dragged the expanded title bar and restored its original location; it remained 440 × 420.
- Native diagnostics during this run recorded 13 outer-window commits, one backing allocation at startup, and no native surface errors. The observed system scale was 2×, so the backing was 2080 × 840 physical pixels throughout.
- Frontend: 107 tests, type checks, and build passed. Rust: 12 window/platform tests and 12 docking tests passed.
- Settings component files and global styles remained byte-for-byte unchanged against the pre-redesign baseline.

These results establish that repeated native viewport resizing was removed and desktop interaction paths work. Final perceptual confirmation of any remaining flicker is requested from the user on the restored production-style tool window; isolated screenshots are not proof that every presented frame is flawless.
