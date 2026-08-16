//! Edge docking for top / left / right. No bottom snap.

use crate::domain::DockSide;

pub const SNAP_THRESHOLD: f64 = 60.0;
pub const EDGE_MARGIN: f64 = 4.0;
pub const ICON_SIZE: f64 = 44.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLayout {
    Collapsed,
    PinnedCollapsed,
    Peek,
    PinnedPeek,
    Expanded,
    PinnedExpanded,
}

impl PanelLayout {
    pub fn parse(value: &str) -> Self {
        match value {
            "pinned-collapsed" => Self::PinnedCollapsed,
            "peek" => Self::Peek,
            "pinned-peek" => Self::PinnedPeek,
            "expanded" => Self::Expanded,
            "pinned-expanded" => Self::PinnedExpanded,
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
        }
    }

    pub fn is_pinned(self) -> bool {
        matches!(
            self,
            Self::PinnedCollapsed | Self::PinnedPeek | Self::PinnedExpanded
        )
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
    (360.0, 448.0)
}

pub fn dynamic_island_pinned_expanded_size(_side: DockSide) -> (f64, f64) {
    (520.0, 448.0)
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
        let along = along_preserving_center(DockSide::Top, icon, 360.0, 448.0);
        let (x, y) = docked_position(desk(), DockSide::Top, along, 360.0, 448.0);
        assert!(
            x > 1400.0,
            "expanded panel should stay on the right, got x={x}"
        );
        assert_eq!(y, EDGE_MARGIN);
    }

    #[test]
    fn left_and_right_layouts_keep_the_same_edge_gap() {
        for (width, height) in [
            (ICON_SIZE, ICON_SIZE),
            (268.0, 48.0),
            (360.0, 48.0),
            (360.0, 448.0),
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
        let along = along_preserving_center(DockSide::Top, icon, 360.0, 448.0);
        let (x, _) = docked_position(desk(), DockSide::Top, along, 360.0, 448.0);
        let icon_center = icon.x + icon.w * 0.5;
        let panel_center = x + 180.0;
        assert!((panel_center - icon_center).abs() < 1.0);
        assert!(
            x > 700.0,
            "must not jump back to the left origin, got x={x}"
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
    fn dynamic_island_mode_lengthens_pinned_peek_and_expanded_layouts() {
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
            (520.0, 448.0)
        );
    }
}
