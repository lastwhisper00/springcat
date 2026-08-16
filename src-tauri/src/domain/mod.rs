pub mod constants;
pub mod notification_policy;
pub mod settings;
pub mod surface_state;
pub mod task_event;
pub mod task_item;

pub use constants::*;
#[allow(unused_imports)]
pub use notification_policy::{decide_notification, NotificationDecision, PanelLayout};
pub use settings::{normalize_settings, AdapterToggles, AppSettings, DockSide, PresentationMode};
pub use surface_state::{derive_surface_state, SurfaceState};
pub use task_event::{parse_task_event, sanitize_summary, TaskEvent, TaskEventType, TaskSource};
pub use task_item::{apply_event_to_task, TaskAction, TaskItem, TaskStatus, UNTITLED_TASK_TITLE};
