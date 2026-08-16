//! System-browser discovery and SpringCat's optional external-link override.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::settings_store::PersistedSettings;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserOption {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    pub system_default_name: String,
    pub system_default_path: Option<String>,
    pub browsers: Vec<BrowserOption>,
}

pub fn browser_info() -> BrowserInfo {
    let system_default = system_default_browser_path();
    let mut paths = known_browser_paths();
    if let Some(path) = &system_default {
        paths.push(path.clone());
    }

    let mut browsers = Vec::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let key = path.display().to_string().replace('/', "\\").to_lowercase();
        if browsers
            .iter()
            .any(|browser: &BrowserOption| browser.path.replace('/', "\\").to_lowercase() == key)
        {
            continue;
        }
        browsers.push(BrowserOption {
            name: browser_display_name(&path),
            path: path.display().to_string(),
        });
    }
    browsers.sort_by(|left, right| left.name.cmp(&right.name));

    BrowserInfo {
        system_default_name: system_default
            .as_deref()
            .map(browser_display_name)
            .unwrap_or_else(|| "系统默认浏览器".to_string()),
        system_default_path: system_default.map(|path| path.display().to_string()),
        browsers,
    }
}

pub fn open_http_url(app: &AppHandle, url: &str) -> bool {
    if !is_http_url(url) {
        return false;
    }
    let preferred = app
        .try_state::<Mutex<PersistedSettings>>()
        .and_then(|state| state.lock().ok()?.app.browser_path.clone());
    if let Some(path) = preferred.filter(|path| Path::new(path).is_file()) {
        if Command::new(path).arg(url).spawn().is_ok() {
            return true;
        }
    }
    app.opener().open_url(url, None::<&str>).is_ok()
}

pub fn is_http_url(value: &str) -> bool {
    value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

fn browser_display_name(path: &Path) -> String {
    let executable = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match executable.as_str() {
        "chrome.exe" | "chrome" => "Google Chrome".to_string(),
        "msedge.exe" | "msedge" => "Microsoft Edge".to_string(),
        "firefox.exe" | "firefox" => "Mozilla Firefox".to_string(),
        "brave.exe" | "brave" => "Brave".to_string(),
        "vivaldi.exe" | "vivaldi" => "Vivaldi".to_string(),
        "launcher.exe"
            if path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("opera") =>
        {
            "Opera".to_string()
        }
        _ => path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("浏览器")
            .to_string(),
    }
}

#[cfg(target_os = "windows")]
fn system_default_browser_path() -> Option<PathBuf> {
    use windows::core::{w, PCWSTR, PWSTR};
    use windows::Win32::UI::Shell::{AssocQueryStringW, ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE};

    let mut length = 0u32;
    unsafe {
        let _ = AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_EXECUTABLE,
            w!("https"),
            PCWSTR::null(),
            None,
            &mut length,
        );
    }
    if length <= 1 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_EXECUTABLE,
            w!("https"),
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
        .ok()
        .ok()?;
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    let path = PathBuf::from(String::from_utf16_lossy(&buffer[..end]));
    path.is_file().then_some(path)
}

#[cfg(not(target_os = "windows"))]
fn system_default_browser_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "windows")]
fn known_browser_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for (variable, relatives) in [
        (
            "ProgramFiles",
            &[
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
                "Mozilla Firefox/firefox.exe",
                "BraveSoftware/Brave-Browser/Application/brave.exe",
                "Vivaldi/Application/vivaldi.exe",
            ][..],
        ),
        (
            "ProgramFiles(x86)",
            &[
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
                "Mozilla Firefox/firefox.exe",
                "BraveSoftware/Brave-Browser/Application/brave.exe",
            ][..],
        ),
        (
            "LOCALAPPDATA",
            &[
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
                "BraveSoftware/Brave-Browser/Application/brave.exe",
                "Vivaldi/Application/vivaldi.exe",
                "Programs/Opera/launcher.exe",
            ][..],
        ),
    ] {
        let Some(base) = std::env::var_os(variable) else {
            continue;
        };
        for relative in relatives {
            paths.push(PathBuf::from(&base).join(relative));
        }
    }
    paths
}

#[cfg(not(target_os = "windows"))]
fn known_browser_paths() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_web_urls_for_browser_override() {
        assert!(is_http_url("https://example.com"));
        assert!(is_http_url("HTTP://example.com"));
        assert!(!is_http_url("codex://threads/123"));
        assert!(!is_http_url("C:/work/index.html"));
    }

    #[test]
    fn gives_known_browsers_friendly_names() {
        assert_eq!(
            browser_display_name(Path::new("C:/Apps/chrome.exe")),
            "Google Chrome"
        );
        assert_eq!(
            browser_display_name(Path::new("C:/Apps/msedge.exe")),
            "Microsoft Edge"
        );
        assert_eq!(
            browser_display_name(Path::new("C:/Apps/firefox.exe")),
            "Mozilla Firefox"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reads_the_windows_default_browser_association() {
        let info = browser_info();
        eprintln!("detected default browser: {info:?}");
        assert!(!info.system_default_name.trim().is_empty());
        if let Some(path) = info.system_default_path {
            assert!(Path::new(&path).is_file());
        }
    }
}
