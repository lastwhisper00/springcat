use std::collections::HashMap;
use std::fs;

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
    }
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
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
