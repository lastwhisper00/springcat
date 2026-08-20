use std::collections::HashMap;
use std::fs;

use chrono::TimeZone;
use serde::{Deserialize, Serialize};

use crate::domain::{normalize_settings, AdapterToggles, AppSettings, DockSide};
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorDock {
    pub side: DockSide,
    pub along: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSettings {
    #[serde(flatten)]
    pub app: AppSettings,
    #[serde(default)]
    pub monitor_docks: HashMap<String, MonitorDock>,
    #[serde(default = "default_double_click")]
    pub double_click_action: String,
}

fn default_double_click() -> String {
    "open-latest".to_string()
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            app: AppSettings::default(),
            monitor_docks: HashMap::new(),
            double_click_action: default_double_click(),
        }
    }
}

pub fn load() -> PersistedSettings {
    let path = paths::settings_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return PersistedSettings::default();
    };
    let mut parsed: PersistedSettings =
        serde_json::from_str(&raw).unwrap_or_else(|_| PersistedSettings::default());
    parsed.app = normalize_settings(Some(parsed.app));
    if parsed.double_click_action != "open-latest" && parsed.double_click_action != "none" {
        parsed.double_click_action = default_double_click();
    }
    parsed
}

pub fn save(settings: &PersistedSettings) {
    let path = paths::settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}

#[allow(dead_code)]
pub fn default_adapters() -> AdapterToggles {
    AdapterToggles {
        codex: true,
        cursor: true,
        grok_cli: true,
        gemini_cli: true,
        work_buddy: true,
        marvis: true,
        dsh_desktop: true,
    }
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Normalize vendor timestamps onto the UTC RFC3339 form used by the task list.
/// Timezone-aware values are converted as-is. Naive values are local wall time.
pub fn occurred_at_rfc3339(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return now_rfc3339();
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return parsed
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    }
    let Some(naive) = parse_naive_datetime(trimmed) else {
        return trimmed.to_string();
    };
    chrono::Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|local| {
            local
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .unwrap_or_else(|| trimmed.to_string())
}

fn parse_naive_datetime(raw: &str) -> Option<chrono::NaiveDateTime> {
    const FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ];
    FORMATS
        .iter()
        .find_map(|format| chrono::NaiveDateTime::parse_from_str(raw, format).ok())
}

pub fn is_muted(settings: &AppSettings) -> bool {
    let Some(until) = settings.muted_until.as_deref() else {
        return false;
    };
    match chrono::DateTime::parse_from_rfc3339(until) {
        Ok(deadline) => deadline > chrono::Utc::now(),
        Err(_) => false,
    }
}

pub fn expire_mute_if_needed(settings: &mut PersistedSettings) -> bool {
    if !is_muted(&settings.app) && settings.app.muted_until.is_some() {
        settings.app.muted_until = None;
        save(settings);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_utc_rfc3339_stable() {
        assert_eq!(
            occurred_at_rfc3339("2026-08-18T09:38:00.000Z"),
            "2026-08-18T09:38:00.000Z"
        );
        assert_eq!(
            occurred_at_rfc3339("2026-08-18T17:38:00+08:00"),
            "2026-08-18T09:38:00.000Z"
        );
    }

    #[test]
    fn treats_naive_marvis_timestamps_as_local_wall_time() {
        let utc = occurred_at_rfc3339("2026-08-18T17:38:00.000000");
        let parsed = chrono::DateTime::parse_from_rfc3339(&utc).unwrap();
        assert!(utc.ends_with('Z'));
        assert_eq!(
            parsed
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            "2026-08-18T17:38:00"
        );
        assert_eq!(occurred_at_rfc3339(&utc), utc);
    }
}
