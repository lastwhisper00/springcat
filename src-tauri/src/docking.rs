//! Edge docking for top / left / right. No bottom snap.

use crate::domain::DockSide;

pub const SNAP_THRESHOLD: f64 = 60.0;
pub const EDGE_MARGIN: f64 = 4.0;
/// Native collapsed frame. The visible orb is smaller so hover/ring effects
/// remain inside the transparent HWND instead of being clipped at its edges.
pub const ICON_SIZE: f64 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLayout {
    Collapsed,
    PinnedCollapsed,
    Peek,
    PinnedPeek,
    Expanded,
    PinnedExpanded,
    Widgets,
    PinnedWidgets,
}

impl PanelLayout {
    pub fn parse(value: &str) -> Self {
        match value {
            "pinned-collapsed" => Self::PinnedCollapsed,
            "peek" => Self::Peek,
            "pinned-peek" => Self::PinnedPeek,
            "expanded" => Self::Expanded,
            "pinned-expanded" => Self::PinnedExpanded,
            "widgets" => Self::Widgets,
            "pinned-widgets" => Self::PinnedWidgets,
            _ => Self::Collapsed,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collapsed => "collapsed",
            Self::PinnedCollapsed => "pinned-collapsed",
            Self::Peek => "peek",
            Self::PinnedPeek => "pinned-peek",
            Self::Expanded => "expanded",
            Self::PinnedExpanded => "pinned-expanded",
            Self::Widgets => "widgets",
            Self::PinnedWidgets => "pinned-widgets",
        }
    }

    pub fn is_pinned(self) -> bool {
        matches!(
            self,
            Self::PinnedCollapsed | Self::PinnedPeek | Self::PinnedExpanded | Self::PinnedWidgets
        )
    }

    pub fn is_widgets(self) -> bool {
        matches!(self, Self::Widgets | Self::PinnedWidgets)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockTarget {
    pub side: DockSide,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn collapsed_size(_side: DockSide) -> (f64, f64) {
    (ICON_SIZE, ICON_SIZE)
}

pub fn peek_size(_side: DockSide) -> (f64, f64) {
    (268.0, 48.0)
}

pub fn pinned_peek_size(_side: DockSide) -> (f64, f64) {
    (360.0, 48.0)
}

pub fn dynamic_island_pinned_peek_size(_side: DockSide) -> (f64, f64) {
    (520.0, 48.0)
}

pub fn expanded_size(_side: DockSide) -> (f64, f64) {
    (440.0, 420.0)
}

pub fn dynamic_island_pinned_expanded_size(_side: DockSide) -> (f64, f64) {
    (440.0, 420.0)
}

pub fn size_for(
    side: DockSide,
    layout: PanelLayout,
    dynamic_island_compatible: bool,
) -> (f64, f64) {
    match layout {
        PanelLayout::Collapsed => collapsed_size(side),
        PanelLayout::PinnedCollapsed => collapsed_size(side),
        PanelLayout::Peek => peek_size(side),
        PanelLayout::PinnedPeek if dynamic_island_compatible => {
            dynamic_island_pinned_peek_size(side)
        }
        PanelLayout::PinnedPeek => pinned_peek_size(side),
        PanelLayout::Expanded => expanded_size(side),
        PanelLayout::PinnedExpanded if dynamic_island_compatible => {
            dynamic_island_pinned_expanded_size(side)
        }
        PanelLayout::PinnedExpanded => expanded_size(side),
        // Widget layouts are placed at the top by the window geometry layer.
        PanelLayout::Widgets | PanelLayout::PinnedWidgets => (800.0, 260.0),
    }
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max.max(min))
}

pub fn docked_position(
    work: Rect,
    side: DockSide,
    along: f64,
    win_w: f64,
    win_h: f64,
) -> (f64, f64) {
    let margin = EDGE_MARGIN;
    match side {
        DockSide::Top => {
            let x = clamp(along, work.x, work.x + work.w - win_w);
            (x, work.y + margin)
        }
        DockSide::Left => {
            let y = clamp(along, work.y, work.y + work.h - win_h);
            (work.x + margin, y)
        }
        DockSide::Right => {
            let y = clamp(along, work.y, work.y + work.h - win_h);
            (work.x + work.w - win_w - margin, y)
        }
    }
}

/// Center a pinned pill on the supplied platform-specific top boundary.
pub fn pinned_top_position(bounds: Rect, win_w: f64) -> (f64, f64) {
    let x = bounds.x + (bounds.w - win_w).max(0.0) * 0.5;
    (x, bounds.y)
}

pub fn along_axis(side: DockSide, x: f64, y: f64) -> f64 {
    match side {
        DockSide::Top => x,
        DockSide::Left | DockSide::Right => y,
    }
}

/// Keep the current window center on the dock axis when the size changes.
pub fn along_preserving_center(side: DockSide, win: Rect, new_w: f64, new_h: f64) -> f64 {
    match side {
        DockSide::Top => win.x + win.w * 0.5 - new_w * 0.5,
        DockSide::Left | DockSide::Right => win.y + win.h * 0.5 - new_h * 0.5,
    }
}

/// Preserve the orb's screen-space anchor while its native window changes
/// shape. A top-docked orb lives at the horizontal center of the capsule, but
/// side-docked orbs live in the top row of the panel rather than its vertical
/// center, so their y coordinate must remain fixed during expansion.
pub fn along_preserving_orb(side: DockSide, win: Rect, new_w: f64, new_h: f64) -> f64 {
    match side {
        DockSide::Top => along_preserving_center(side, win, new_w, new_h),
        DockSide::Left | DockSide::Right => along_axis(side, win.x, win.y),
    }
}

pub fn default_along(work: Rect, side: DockSide, win_w: f64, _win_h: f64) -> f64 {
    let margin = EDGE_MARGIN;
    match side {
        DockSide::Top => work.x + work.w - win_w - margin,
        DockSide::Left | DockSide::Right => work.y + margin,
    }
}

/// Distances from the window center to top / left / right work-area edges.
pub fn edge_distances(work: Rect, win: Rect) -> (f64, f64, f64) {
    let cx = win.x + win.w * 0.5;
    let cy = win.y + win.h * 0.5;
    let dist_top = (cy - work.y).abs();
    let dist_left = (cx - work.x).abs();
    let dist_right = ((work.x + work.w) - cx).abs();
    (dist_top, dist_left, dist_right)
}

pub fn nearest_side(work: Rect, win: Rect) -> DockSide {
    let (dist_top, dist_left, dist_right) = edge_distances(work, win);
    let mut best = (dist_top, DockSide::Top);
    if dist_left < best.0 {
        best = (dist_left, DockSide::Left);
    }
    if dist_right < best.0 {
        best = (dist_right, DockSide::Right);
    }
    best.1
}

pub fn preview_side(work: Rect, win: Rect, threshold: f64) -> Option<DockSide> {
    let (dist_top, dist_left, dist_right) = edge_distances(work, win);
    let mut best = (dist_top, DockSide::Top);
    if dist_left < best.0 {
        best = (dist_left, DockSide::Left);
    }
    if dist_right < best.0 {
        best = (dist_right, DockSide::Right);
    }
    (best.0 <= threshold).then_some(best.1)
}

pub fn snap_target(work: Rect, win: Rect, force: bool) -> DockTarget {
    let side = if force {
        nearest_side(work, win)
    } else {
        preview_side(work, win, SNAP_THRESHOLD).unwrap_or_else(|| nearest_side(work, win))
    };
    let (width, height) = collapsed_size(side);
    let along = along_axis(side, win.x, win.y);
    let (x, y) = docked_position(work, side, along, width, height);
    DockTarget {
        side,
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1040.0,
        }
    }

    #[test]
    fn snaps_right_when_close_to_right_edge() {
        let win = Rect {
            x: 1860.0,
            y: 200.0,
            w: 360.0,
            h: 48.0,
        };
        let target = snap_target(desk(), win, true);
        assert_eq!(target.side, DockSide::Right);
        assert_eq!(target.width, ICON_SIZE);
    }

    #[test]
    fn snaps_left_when_close_to_left_edge() {
        let win = Rect {
            x: 20.0,
            y: 300.0,
            w: 360.0,
            h: 48.0,
        };
        let target = snap_target(desk(), win, true);
        assert_eq!(target.side, DockSide::Left);
    }

    #[test]
    fn snaps_top_near_top_edge() {
        let win = Rect {
            x: 800.0,
            y: 10.0,
            w: 360.0,
            h: 48.0,
        };
        let target = snap_target(desk(), win, true);
        assert_eq!(target.side, DockSide::Top);
        assert_eq!(target.y, EDGE_MARGIN);
    }

    #[test]
    fn never_snaps_to_bottom() {
        let win = Rect {
            x: 800.0,
            y: 990.0,
            w: 360.0,
            h: 48.0,
        };
        let target = snap_target(desk(), win, true);
        assert_eq!(target.side, DockSide::Right);
        assert_ne!(format!("{:?}", target.side), "Bottom");
    }

    #[test]
    fn expand_keeps_horizontal_center_on_top() {
        let icon = Rect {
            x: 1800.0,
            y: 4.0,
            w: ICON_SIZE,
            h: ICON_SIZE,
        };
        let (width, height) = expanded_size(DockSide::Top);
        let along = along_preserving_center(DockSide::Top, icon, width, height);
        let (x, y) = docked_position(desk(), DockSide::Top, along, width, height);
        assert_eq!(x + width, desk().x + desk().w);
        assert_eq!(y, EDGE_MARGIN);
    }

    #[test]
    fn left_and_right_layouts_keep_the_same_edge_gap() {
        for (width, height) in [
            (ICON_SIZE, ICON_SIZE),
            (268.0, 48.0),
            (360.0, 48.0),
            expanded_size(DockSide::Left),
        ] {
            let (left_x, _) = docked_position(desk(), DockSide::Left, 200.0, width, height);
            let (right_x, _) = docked_position(desk(), DockSide::Right, 200.0, width, height);
            let right_gap = desk().x + desk().w - (right_x + width);

            assert_eq!(left_x - desk().x, EDGE_MARGIN);
            assert_eq!(right_gap, EDGE_MARGIN);
        }
    }

    #[test]
    fn expand_stays_with_moved_icon_not_left_origin() {
        let icon = Rect {
            x: 920.0,
            y: 4.0,
            w: ICON_SIZE,
            h: ICON_SIZE,
        };
        let (width, height) = expanded_size(DockSide::Top);
        let along = along_preserving_center(DockSide::Top, icon, width, height);
        let (x, _) = docked_position(desk(), DockSide::Top, along, width, height);
        let icon_center = icon.x + icon.w * 0.5;
        let panel_center = x + width * 0.5;
        assert!((panel_center - icon_center).abs() < 1.0);
        assert!(
            x > 600.0,
            "must not jump back to the left origin, got x={x}"
        );
    }

    #[test]
    fn top_expand_and_collapse_share_the_same_screen_center() {
        let icon = Rect {
            x: 920.0,
            y: EDGE_MARGIN,
            w: ICON_SIZE,
            h: ICON_SIZE,
        };
        let expanded_width = expanded_size(DockSide::Top).0;
        let expanded_x = along_preserving_center(
            DockSide::Top,
            icon,
            expanded_width,
            expanded_size(DockSide::Top).1,
        );
        let expanded = Rect {
            x: expanded_x,
            y: EDGE_MARGIN,
            w: expanded_width,
            h: expanded_size(DockSide::Top).1,
        };
        let collapsed_x = along_preserving_center(DockSide::Top, expanded, ICON_SIZE, ICON_SIZE);

        assert!((collapsed_x - icon.x).abs() < 1.0);
        assert!((expanded.x + expanded.w * 0.5 - (icon.x + icon.w * 0.5)).abs() < 1.0);
    }

    #[test]
    fn side_expand_keeps_the_orb_in_the_same_top_row() {
        let icon = Rect {
            x: EDGE_MARGIN,
            y: 320.0,
            w: ICON_SIZE,
            h: ICON_SIZE,
        };

        assert_eq!(
            along_preserving_orb(DockSide::Left, icon, 440.0, 420.0),
            icon.y
        );
        assert_eq!(
            along_preserving_orb(DockSide::Right, icon, 440.0, 420.0),
            icon.y
        );
    }

    #[test]
    fn pinned_pill_is_centered_on_supplied_top_without_a_gap() {
        let bounds = Rect {
            x: -1920.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        let (x, y) = pinned_top_position(bounds, 360.0);

        assert_eq!(x, -1140.0);
        assert_eq!(y, 0.0);
        assert_eq!(x + 180.0, bounds.x + bounds.w * 0.5);
    }

    #[test]
    fn dynamic_island_mode_lengthens_only_the_pinned_peek() {
        assert_eq!(
            size_for(DockSide::Top, PanelLayout::PinnedPeek, false),
            (360.0, 48.0)
        );
        assert_eq!(
            size_for(DockSide::Top, PanelLayout::PinnedPeek, true),
            (520.0, 48.0)
        );
        assert_eq!(
            size_for(DockSide::Top, PanelLayout::PinnedExpanded, true),
            (440.0, 420.0)
        );
    }

    #[test]
    fn expanded_layouts_share_the_same_content_area_on_every_dock() {
        for side in [DockSide::Top, DockSide::Left, DockSide::Right] {
            for layout in [PanelLayout::Expanded, PanelLayout::PinnedExpanded] {
                for compatible in [false, true] {
                    assert_eq!(size_for(side, layout, compatible), (440.0, 420.0));
                }
            }
        }
    }

    #[test]
    fn widget_layout_tokens_preserve_page_and_pin_identity() {
        for (token, pinned) in [("widgets", false), ("pinned-widgets", true)] {
            let layout = PanelLayout::parse(token);
            assert_eq!(layout.as_str(), token);
            assert_eq!(layout.is_pinned(), pinned);
            assert!(layout.is_widgets());
            for compatible in [false, true] {
                assert_eq!(size_for(DockSide::Top, layout, compatible), (800.0, 260.0));
            }
        }
        assert!(!PanelLayout::Expanded.is_widgets());
        assert!(!PanelLayout::PinnedExpanded.is_widgets());
    }

    #[test]
    fn unpinned_widgets_preserve_the_previous_horizontal_center_at_the_work_area_top() {
        let previous = Rect {
            x: 740.0,
            y: 180.0,
            w: 440.0,
            h: 420.0,
        };
        let (width, height) = size_for(DockSide::Top, PanelLayout::Widgets, false);
        let along = along_preserving_orb(DockSide::Top, previous, width, height);
        let (x, y) = docked_position(desk(), DockSide::Top, along, width, height);
        assert_eq!(x + width * 0.5, previous.x + previous.w * 0.5);
        assert_eq!(y, desk().y + EDGE_MARGIN);
    }

    #[test]
    fn pinned_widgets_use_the_physical_top_center_without_shrinking() {
        let bounds = Rect { x: -1920.0, y: 0.0, w: 1920.0, h: 1080.0 };
        let (width, height) = size_for(DockSide::Top, PanelLayout::PinnedWidgets, false);
        let (x, y) = pinned_top_position(bounds, width);
        assert_eq!((width, height), (800.0, 260.0));
        assert_eq!(x + width * 0.5, bounds.x + bounds.w * 0.5);
        assert_eq!(y, bounds.y);
    }
}
