use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use crate::domain::APP_DATA_DIR_NAME;

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_DATA_DIR_NAME)
}

static ACTIVE_CACHE_DIR: OnceLock<RwLock<PathBuf>> = OnceLock::new();

fn active_cache_dir() -> &'static RwLock<PathBuf> {
    ACTIVE_CACHE_DIR.get_or_init(|| RwLock::new(data_dir()))
}

pub fn cache_dir() -> PathBuf {
    active_cache_dir().read().expect("cache directory").clone()
}

pub fn cache_dir_from_setting(value: Option<&str>) -> Result<PathBuf, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(data_dir());
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("缓存目录必须使用绝对路径。".into());
    }
    Ok(path)
}

pub fn configure_cache_dir(value: Option<&str>) -> Result<PathBuf, String> {
    let path = cache_dir_from_setting(value)?;
    ensure_cache_dirs_at(&path).map_err(|err| format!("无法创建缓存目录：{err}"))?;
    *active_cache_dir().write().expect("cache directory") = path.clone();
    Ok(path)
}

pub fn inbox_dir() -> PathBuf {
    cache_dir().join("inbox")
}

pub fn failed_dir() -> PathBuf {
    cache_dir().join("inbox-failed")
}

pub fn db_path() -> PathBuf {
    cache_dir().join("tasks.sqlite")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn log_dir() -> PathBuf {
    cache_dir().join("logs")
}

pub fn bridge_name() -> &'static str {
    if cfg!(windows) {
        "springcat-bridge.exe"
    } else {
        "springcat-bridge"
    }
}

pub fn installed_bridge_path() -> PathBuf {
    data_dir().join("bin").join(bridge_name())
}

/// Best-effort location of the hook binary Codex / Cursor / Grok / Gemini should call.
pub fn resolve_bridge() -> Option<PathBuf> {
    let name = bridge_name();
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
            if let Some(workspace) = dir.ancestors().nth(3) {
                candidates.push(workspace.join("bridge/target/release").join(name));
                candidates.push(workspace.join("bridge/target/debug").join(name));
            }
        }
    }
    candidates.push(installed_bridge_path());
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join(name));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub fn ensure_dirs() -> std::io::Result<()> {
    fs::create_dir_all(data_dir().join("bin"))?;
    ensure_cache_dirs_at(&cache_dir())?;
    Ok(())
}

pub fn ensure_cache_dirs_at(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("inbox"))?;
    fs::create_dir_all(root.join("inbox-failed"))?;
    fs::create_dir_all(root.join("logs"))?;
    Ok(())
}

pub fn same_directory(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_setting_defaults_and_rejects_relative_paths() {
        assert_eq!(cache_dir_from_setting(None).unwrap(), data_dir());
        assert!(cache_dir_from_setting(Some("relative/cache")).is_err());
        let absolute = std::env::temp_dir().join("springcat-cache");
        assert_eq!(
            cache_dir_from_setting(Some(&absolute.display().to_string())).unwrap(),
            absolute
        );
    }
}
