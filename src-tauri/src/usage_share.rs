use std::fs;
use std::path::{Path, PathBuf};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const MAX_SHARE_BYTES: usize = 12 * 1024 * 1024;

pub fn save_png(file_name: &str, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < PNG_SIGNATURE.len() || !bytes.starts_with(PNG_SIGNATURE) {
        return Err("分享图片不是有效的 PNG 文件。".into());
    }
    if bytes.len() > MAX_SHARE_BYTES {
        return Err("分享图片过大，无法保存。".into());
    }
    let file_name = normalize_file_name(file_name)?;
    let directory = dirs::download_dir().unwrap_or_else(crate::paths::data_dir);
    fs::create_dir_all(&directory).map_err(|err| format!("无法创建下载目录：{err}"))?;
    let target = available_path(&directory, &file_name);
    fs::write(&target, bytes).map_err(|err| format!("无法保存分享图片：{err}"))?;
    Ok(target.display().to_string())
}

fn normalize_file_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 180 {
        return Err("分享图片文件名无效。".into());
    }
    let file_name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "分享图片文件名无效。".to_string())?;
    if file_name != value || !file_name.to_ascii_lowercase().ends_with(".png") {
        return Err("分享图片文件名必须是纯 PNG 文件名。".into());
    }
    if file_name
        .chars()
        .any(|value| matches!(value, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
        return Err("分享图片文件名包含无效字符。".into());
    }
    Ok(file_name.to_string())
}

fn available_path(directory: &Path, file_name: &str) -> PathBuf {
    let direct = directory.join(file_name);
    if !direct.exists() {
        return direct;
    }
    let stem = file_name.strip_suffix(".png").unwrap_or(file_name);
    for index in 2..=999 {
        let candidate = directory.join(format!("{stem} ({index}).png"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!(
        "{stem}-{}.png",
        chrono::Utc::now().timestamp_millis()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_share_filenames() {
        assert_eq!(
            normalize_file_name("SpringCat-AI周报-2026-08-10_2026-08-16.png").unwrap(),
            "SpringCat-AI周报-2026-08-10_2026-08-16.png"
        );
    }

    #[test]
    fn rejects_paths_and_non_png_names() {
        assert!(normalize_file_name("../report.png").is_err());
        assert!(normalize_file_name("report.jpg").is_err());
        assert!(normalize_file_name("C:\\Temp\\report.png").is_err());
    }
}
