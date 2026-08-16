use serde::{Deserialize, Serialize};

use super::task_item::{TaskItem, TaskStatus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SurfaceState {
    Idle,
    Working {
        task: TaskItem,
    },
    Waiting {
        task: TaskItem,
    },
    Failed {
        task: TaskItem,
    },
    Completed {
        task: TaskItem,
        unread: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        merged_count: Option<usize>,
    },
}

fn latest(tasks: &[TaskItem]) -> TaskItem {
    tasks
        .iter()
        .max_by(|a, b| {
            a.updated_at
                .cmp(&b.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        })
        .cloned()
        .expect("latest() requires a non-empty slice")
}

/// Keep in sync with src/domain/surface-state.ts
/// Priority: running > waiting > failed > completed-unread > idle
pub fn derive_surface_state(tasks: &[TaskItem]) -> SurfaceState {
    let running: Vec<_> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Running)
        .cloned()
        .collect();
    if !running.is_empty() {
        return SurfaceState::Working {
            task: latest(&running),
        };
    }

    let waiting: Vec<_> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Waiting)
        .cloned()
        .collect();
    if !waiting.is_empty() {
        return SurfaceState::Waiting {
            task: latest(&waiting),
        };
    }

    let failed: Vec<_> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Failed)
        .cloned()
        .collect();
    if !failed.is_empty() {
        return SurfaceState::Failed {
            task: latest(&failed),
        };
    }

    let completed_unread: Vec<_> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Completed && task.unread)
        .cloned()
        .collect();
    if completed_unread.len() == 1 {
        return SurfaceState::Completed {
            task: completed_unread[0].clone(),
            unread: true,
            merged_count: None,
        };
    }
    if completed_unread.len() > 1 {
        return SurfaceState::Completed {
            task: latest(&completed_unread),
            unread: true,
            merged_count: Some(completed_unread.len()),
        };
    }

    SurfaceState::Idle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskSource;

    fn item(id: &str, status: TaskStatus, unread: bool, updated_at: &str) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            source: TaskSource::Codex,
            title: id.to_string(),
            summary: None,
            status,
            started_at: None,
            updated_at: updated_at.to_string(),
            completed_at: None,
            unread,
            action: None,
        }
    }

    #[test]
    fn empty_is_idle() {
        assert_eq!(derive_surface_state(&[]), SurfaceState::Idle);
    }

    #[test]
    fn running_beats_pending_historical_notifications() {
        let state = derive_surface_state(&[
            item("a", TaskStatus::Running, false, "2026-08-13T04:00:00.000Z"),
            item("b", TaskStatus::Completed, true, "2026-08-13T05:00:00.000Z"),
            item("c", TaskStatus::Failed, true, "2026-08-13T06:00:00.000Z"),
            item("d", TaskStatus::Waiting, true, "2026-08-13T03:00:00.000Z"),
        ]);
        match state {
            SurfaceState::Working { task } => assert_eq!(task.id, "a"),
            other => panic!("expected working, got {other:?}"),
        }
    }

    #[test]
    fn merges_unread_completions() {
        let state = derive_surface_state(&[
            item(
                "c1",
                TaskStatus::Completed,
                true,
                "2026-08-13T04:00:00.000Z",
            ),
            item(
                "c2",
                TaskStatus::Completed,
                true,
                "2026-08-13T04:02:00.000Z",
            ),
            item(
                "c3",
                TaskStatus::Completed,
                true,
                "2026-08-13T04:01:00.000Z",
            ),
        ]);
        match state {
            SurfaceState::Completed {
                task,
                unread,
                merged_count,
            } => {
                assert!(unread);
                assert_eq!(merged_count, Some(3));
                assert_eq!(task.id, "c2");
            }
            other => panic!("expected completed, got {other:?}"),
        }
    }
}
