//! Rolling file logs. Never write credentials, full prompts, or tool dumps.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

use crate::paths;

pub fn init() -> Option<WorkerGuard> {
    let _ = paths::ensure_dirs();
    let file_appender = tracing_appender::rolling::daily(paths::log_dir(), "springcat.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(non_blocking.and(std::io::stderr))
        .init();
    Some(guard)
}

pub fn redact(value: &str) -> String {
    if value.len() > 80 {
        format!("{}…", value.chars().take(40).collect::<String>())
    } else {
        value.to_string()
    }
}
