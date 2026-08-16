//! Notification policy. Keep in sync with src/domain/notification-policy.ts
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::settings::AppSettings;
use super::surface_state::SurfaceState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelLayout {
    Collapsed,
    Peek,
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDecision {
    pub layout: PanelLayout,
    pub auto_hide_ms: Option<u64>,
    pub peek: bool,
}

/// Keep in sync with src/domain/notification-policy.ts
pub fn decide_notification(state: &SurfaceState, settings: &AppSettings) -> NotificationDecision {
    if muted(settings) {
        return NotificationDecision {
            layout: PanelLayout::Collapsed,
            auto_hide_ms: None,
            peek: false,
        };
    }

    match state {
        SurfaceState::Waiting { .. } => NotificationDecision {
            layout: PanelLayout::Peek,
            auto_hide_ms: None,
            peek: true,
        },
        SurfaceState::Failed { .. } => NotificationDecision {
            layout: PanelLayout::Peek,
            auto_hide_ms: None,
            peek: true,
        },
        SurfaceState::Completed { .. } => {
            if settings.focus_mode {
                NotificationDecision {
                    layout: PanelLayout::Collapsed,
                    auto_hide_ms: None,
                    peek: false,
                }
            } else {
                NotificationDecision {
                    layout: PanelLayout::Peek,
                    auto_hide_ms: Some(5000),
                    peek: true,
                }
            }
        }
        SurfaceState::Working { .. } => NotificationDecision {
            layout: PanelLayout::Peek,
            auto_hide_ms: None,
            peek: true,
        },
        SurfaceState::Idle => NotificationDecision {
            layout: PanelLayout::Collapsed,
            auto_hide_ms: None,
            peek: false,
        },
    }
}

fn muted(settings: &AppSettings) -> bool {
    let Some(until) = settings.muted_until.as_deref() else {
        return false;
    };
    match chrono::DateTime::parse_from_rfc3339(until) {
        Ok(deadline) => deadline > chrono::Utc::now(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{TaskItem, TaskSource, TaskStatus};

    fn waiting() -> SurfaceState {
        SurfaceState::Waiting {
            task: TaskItem {
                id: "t".into(),
                source: TaskSource::Codex,
                title: "ask".into(),
                summary: None,
                status: TaskStatus::Waiting,
                started_at: None,
                updated_at: "2026-08-13T04:00:00.000Z".into(),
                completed_at: None,
                unread: true,
                action: None,
            },
        }
    }

    fn working() -> SurfaceState {
        let SurfaceState::Waiting { mut task } = waiting() else {
            unreachable!()
        };
        task.status = TaskStatus::Running;
        task.unread = false;
        SurfaceState::Working { task }
    }

    #[test]
    fn muted_never_peeks() {
        let mut settings = AppSettings::default();
        settings.muted_until = Some("2099-01-01T00:00:00.000Z".into());
        let decision = decide_notification(&waiting(), &settings);
        assert!(!decision.peek);
        assert_eq!(decision.layout, PanelLayout::Collapsed);
    }

    #[test]
    fn working_stays_visible() {
        let decision = decide_notification(&working(), &AppSettings::default());
        assert!(decision.peek);
        assert_eq!(decision.layout, PanelLayout::Peek);
        assert_eq!(decision.auto_hide_ms, None);
    }
}
