use serde::{Deserialize, Serialize};

use super::task_event::{sanitize_summary, TaskEvent, TaskEventType, TaskSource};

pub const UNTITLED_TASK_TITLE: &str = "未命名任务";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Running,
    Waiting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAction {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub source: TaskSource,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub unread: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<TaskAction>,
}

fn status_from_event(event_type: TaskEventType) -> TaskStatus {
    match event_type {
        TaskEventType::Started | TaskEventType::Progress => TaskStatus::Running,
        TaskEventType::Waiting => TaskStatus::Waiting,
        TaskEventType::Completed => TaskStatus::Completed,
        TaskEventType::Failed => TaskStatus::Failed,
        TaskEventType::Cancelled => TaskStatus::Cancelled,
    }
}

fn default_title(event: &TaskEvent) -> String {
    event
        .title
        .as_ref()
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| UNTITLED_TASK_TITLE.to_string())
}

fn action_for(event: &TaskEvent, status: TaskStatus) -> Option<TaskAction> {
    let label = match status {
        TaskStatus::Completed => "查看结果",
        TaskStatus::Failed => "查看原因",
        TaskStatus::Waiting => "去处理",
        _ if event.deep_link.is_some() => "打开来源",
        _ => return None,
    };
    Some(TaskAction {
        label: label.to_string(),
        deep_link: event.deep_link.clone(),
    })
}

pub fn apply_event_to_task(existing: Option<&TaskItem>, event: &TaskEvent) -> TaskItem {
    let status = status_from_event(event.event_type);
    let title = event
        .title
        .as_ref()
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .or_else(|| existing.map(|item| item.title.clone()))
        .unwrap_or_else(|| default_title(event));
    let summary = sanitize_summary(event.summary.as_deref())
        .or_else(|| existing.and_then(|item| item.summary.clone()));
    let started_at = if event.event_type == TaskEventType::Started {
        Some(event.occurred_at.clone())
    } else {
        existing
            .and_then(|item| item.started_at.clone())
            .or_else(|| {
                if status == TaskStatus::Running {
                    Some(event.occurred_at.clone())
                } else {
                    None
                }
            })
    };
    let completed_at = matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    )
    .then(|| event.occurred_at.clone());
    let unread = matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Waiting
    );

    TaskItem {
        id: event.task_id.clone(),
        source: event.source,
        title,
        summary,
        status,
        started_at,
        updated_at: event.occurred_at.clone(),
        completed_at,
        unread,
        action: action_for(event, status).or_else(|| existing.and_then(|item| item.action.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::parse_task_event;
    use serde_json::json;

    #[test]
    fn maps_started_then_completed() {
        let started = apply_event_to_task(
            None,
            &parse_task_event(&json!({
                "schemaVersion": 1,
                "eventId": "e1",
                "source": "cursor",
                "type": "task.started",
                "taskId": "t9",
                "title": "refactor",
                "occurredAt": "2026-08-13T04:00:00.000Z"
            }))
            .unwrap(),
        );
        let completed = apply_event_to_task(
            Some(&started),
            &parse_task_event(&json!({
                "schemaVersion": 1,
                "eventId": "e2",
                "source": "cursor",
                "type": "task.completed",
                "taskId": "t9",
                "occurredAt": "2026-08-13T04:01:00.000Z"
            }))
            .unwrap(),
        );

        assert_eq!(started.status, TaskStatus::Running);
        assert_eq!(completed.status, TaskStatus::Completed);
        assert!(completed.unread);
        assert_eq!(
            completed.started_at.as_deref(),
            Some("2026-08-13T04:00:00.000Z")
        );
    }
}
