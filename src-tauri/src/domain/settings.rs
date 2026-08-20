use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresentationMode {
    Work,
    Pet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DockSide {
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterToggles {
    pub codex: bool,
    pub cursor: bool,
    pub grok_cli: bool,
    #[serde(default = "default_enabled")]
    pub gemini_cli: bool,
    #[serde(default = "default_enabled")]
    pub work_buddy: bool,
    #[serde(default = "default_enabled")]
    pub marvis: bool,
    #[serde(default = "default_enabled")]
    pub dsh_desktop: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub presentation_mode: PresentationMode,
    pub dock_side: DockSide,
    #[serde(default)]
    pub dynamic_island_compatible: bool,
    pub always_on_top: bool,
    #[serde(default)]
    pub auto_pin_while_running: bool,
    pub autostart: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted_until: Option<String>,
    pub focus_mode: bool,
    pub history_retention_days: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_directory: Option<String>,
    /// Optional browser executable used only for external HTTP(S) links.
    /// `None` follows the operating system default browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_path: Option<String>,
    pub adapters: AdapterToggles,
}

pub const PET_MODE_IMPLEMENTED: bool = false;

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            presentation_mode: PresentationMode::Work,
            dock_side: DockSide::Top,
            dynamic_island_compatible: false,
            always_on_top: true,
            auto_pin_while_running: false,
            autostart: false,
            muted_until: None,
            focus_mode: false,
            history_retention_days: 7,
            cache_directory: None,
            browser_path: None,
            adapters: AdapterToggles {
                codex: true,
                cursor: true,
                grok_cli: true,
                gemini_cli: true,
                work_buddy: true,
                marvis: true,
                dsh_desktop: true,
            },
        }
    }
}

pub fn normalize_settings(input: Option<AppSettings>) -> AppSettings {
    let mut settings = input.unwrap_or_default();
    if settings.presentation_mode == PresentationMode::Pet && !PET_MODE_IMPLEMENTED {
        settings.presentation_mode = PresentationMode::Work;
    }
    if !matches!(settings.history_retention_days, 0 | 1 | 7 | 30) {
        settings.history_retention_days = 7;
    }
    settings.cache_directory = settings
        .cache_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| Path::new(value).is_absolute())
        .map(str::to_string);
    settings.browser_path = settings
        .browser_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| Path::new(value).is_absolute())
        .filter(|value| Path::new(value).is_file())
        .map(str::to_string);
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_mode_falls_back_to_work() {
        let mut incoming = AppSettings::default();
        incoming.presentation_mode = PresentationMode::Pet;
        incoming.dock_side = DockSide::Left;
        let settings = normalize_settings(Some(incoming));
        assert_eq!(settings.presentation_mode, PresentationMode::Work);
        assert_eq!(settings.dock_side, DockSide::Left);
    }

    #[test]
    fn cache_directory_must_be_absolute() {
        let mut incoming = AppSettings::default();
        incoming.cache_directory = Some(" relative/cache ".into());
        assert_eq!(normalize_settings(Some(incoming)).cache_directory, None);

        let absolute = std::env::temp_dir().join("springcat-cache");
        let mut incoming = AppSettings::default();
        incoming.cache_directory = Some(format!("  {}  ", absolute.display()));
        assert_eq!(
            normalize_settings(Some(incoming)).cache_directory,
            Some(absolute.display().to_string())
        );
    }

    #[test]
    fn old_settings_default_dynamic_island_compatibility_to_off() {
        let value = serde_json::json!({
            "presentationMode": "work",
            "dockSide": "top",
            "alwaysOnTop": true,
            "autostart": false,
            "focusMode": false,
            "historyRetentionDays": 7,
            "adapters": {
                "codex": true,
                "cursor": true,
                "grokCli": true,
                "workBuddy": true
            }
        });
        let settings: AppSettings = serde_json::from_value(value).unwrap();
        assert!(!settings.dynamic_island_compatible);
        assert!(!settings.auto_pin_while_running);
        assert_eq!(settings.browser_path, None);
        assert!(settings.adapters.gemini_cli);
        assert!(settings.adapters.marvis);
        assert!(settings.adapters.dsh_desktop);
    }

    #[test]
    fn browser_override_must_be_an_existing_absolute_file() {
        let mut incoming = AppSettings::default();
        incoming.browser_path = Some("browser.exe".into());
        assert_eq!(normalize_settings(Some(incoming)).browser_path, None);

        let temp = tempfile::NamedTempFile::new().unwrap();
        let mut incoming = AppSettings::default();
        incoming.browser_path = Some(format!("  {}  ", temp.path().display()));
        assert_eq!(
            normalize_settings(Some(incoming)).browser_path,
            Some(temp.path().display().to_string())
        );
    }
}
