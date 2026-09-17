//! Read-only overview data. Network probes never run on the UI thread.
use std::time::{Duration, Instant};

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::event_collector::CollectorState;
use crate::repository::DailyUsage;

const PROBE_TIMEOUT: Duration = Duration::from_millis(2500);

#[derive(Clone, Copy)]
struct ProbeTarget {
    service: &'static str,
    host: &'static str,
    path: &'static str,
}

const PROBE_TARGETS: [ProbeTarget; 2] = [
    ProbeTarget {
        service: "ChatGPT",
        host: "chatgpt.com",
        path: "/",
    },
    ProbeTarget {
        service: "Google",
        host: "www.google.com",
        path: "/",
    },
];

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodayUsage {
    date: String,
    input_tokens: i64,
    output_tokens: i64,
    total_tokens: i64,
}

fn sum_today(rows: &[DailyUsage], date: &str) -> TodayUsage {
    let mut result = TodayUsage {
        date: date.into(),
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
    };
    for row in rows.iter().filter(|row| row.date == date) {
        // Cached input and reasoning are already included in source totals.
        result.input_tokens += row.input_tokens;
        result.output_tokens += row.output_tokens;
        result.total_tokens += row.total_tokens;
    }
    result
}

pub fn today_usage(app: &AppHandle) -> Result<TodayUsage, String> {
    let date = Local::now().format("%Y-%m-%d").to_string();
    let collector = app
        .try_state::<CollectorState>()
        .ok_or("用量采集尚未就绪")?;
    let rows = collector
        .db
        .lock()
        .map_err(|_| "用量数据库暂不可用")?
        .list_usage_month(&date[..7])?;
    Ok(sum_today(&rows, &date))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    connection_type: String,
    status: String,
    foreign_reachable: bool,
    reachable_service: Option<&'static str>,
    latency_ms: Option<u64>,
    probes: Vec<ProbeResult>,
    checked_at: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    service: &'static str,
    reachable: bool,
    latency_ms: Option<u64>,
}

pub fn network_snapshot() -> Result<NetworkSnapshot, String> {
    let (connection_type, status) = connection_profile()?;
    let probes = if status == "offline" {
        PROBE_TARGETS
            .iter()
            .map(|target| ProbeResult {
                service: target.service,
                reachable: false,
                latency_ms: None,
            })
            .collect()
    } else {
        probe_foreign_services()
    };
    let reachable = probes.iter().find(|probe| probe.reachable);
    Ok(NetworkSnapshot {
        connection_type,
        status,
        foreign_reachable: reachable.is_some(),
        reachable_service: reachable.map(|probe| probe.service),
        latency_ms: reachable.and_then(|probe| probe.latency_ms),
        probes,
        checked_at: Local::now().to_rfc3339(),
    })
}

fn probe_foreign_services() -> Vec<ProbeResult> {
    std::thread::scope(|scope| {
        let workers: Vec<_> = PROBE_TARGETS
            .iter()
            .copied()
            .map(|target| scope.spawn(move || https_head_probe(target, PROBE_TIMEOUT)))
            .collect();
        workers
            .into_iter()
            .enumerate()
            .map(|(index, worker)| {
                worker.join().unwrap_or(ProbeResult {
                    service: PROBE_TARGETS[index].service,
                    reachable: false,
                    latency_ms: None,
                })
            })
            .collect()
    })
}

#[cfg(windows)]
fn https_head_probe(target: ProbeTarget, timeout: Duration) -> ProbeResult {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
        WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    };

    struct Handle(*mut core::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                let _ = unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
    let failed = || ProbeResult {
        service: target.service,
        reachable: false,
        latency_ms: None,
    };
    let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    let started = Instant::now();
    let session = Handle(unsafe {
        WinHttpOpen(
            w!("SpringCat network check"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    });
    if session.0.is_null() {
        return failed();
    }
    if unsafe { WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms) }
        .is_err()
    {
        return failed();
    }
    let host = HSTRING::from(target.host);
    let connection = Handle(unsafe { WinHttpConnect(session.0, &host, 443, 0) });
    if connection.0.is_null() {
        return failed();
    }
    let path = HSTRING::from(target.path);
    let request = Handle(unsafe {
        WinHttpOpenRequest(
            connection.0,
            w!("HEAD"),
            &path,
            PCWSTR::null(),
            PCWSTR::null(),
            core::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    });
    if request.0.is_null() {
        return failed();
    }
    let result: windows::core::Result<()> = (|| unsafe {
        WinHttpSendRequest(request.0, None, None, 0, 0, 0)?;
        WinHttpReceiveResponse(request.0, core::ptr::null_mut())?;
        Ok(())
    })();
    if result.is_err() {
        return failed();
    }
    ProbeResult {
        service: target.service,
        reachable: true,
        latency_ms: Some(started.elapsed().as_millis().max(1) as u64),
    }
}

#[cfg(not(windows))]
fn https_head_probe(target: ProbeTarget, _timeout: Duration) -> ProbeResult {
    ProbeResult {
        service: target.service,
        reachable: false,
        latency_ms: None,
    }
}

#[cfg(windows)]
fn connection_profile() -> Result<(String, String), String> {
    use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
    use windows::Win32::Foundation::{E_POINTER, RPC_E_CHANGED_MODE};
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { RoUninitialize() };
            }
        }
    }
    let _apartment = match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        Ok(()) => Apartment(true),
        Err(error) if error.code() == RPC_E_CHANGED_MODE => Apartment(false),
        Err(error) => return Err(error.to_string()),
    };
    // Windows' preferred internet interface, not an arbitrary installed adapter.
    // Null profile is projected by windows-rs as E_POINTER (disconnected).
    let profile = match NetworkInformation::GetInternetConnectionProfile() {
        Ok(profile) => profile,
        Err(error) if error.code() == E_POINTER => return Ok(("none".into(), "offline".into())),
        Err(error) => return Err(error.to_string()),
    };
    let level = profile
        .GetNetworkConnectivityLevel()
        .map_err(|e| e.to_string())?;
    let status = if level == NetworkConnectivityLevel::None {
        "offline"
    } else if level == NetworkConnectivityLevel::InternetAccess {
        "internet"
    } else {
        "local"
    };
    let kind = if profile.IsWlanConnectionProfile().unwrap_or(false) {
        "wifi"
    } else if profile.IsWwanConnectionProfile().unwrap_or(false) {
        "cellular"
    } else {
        match profile
            .NetworkAdapter()
            .and_then(|a| a.IanaInterfaceType())
            .ok()
        {
            Some(6) => "ethernet",
            Some(23 | 131) => "vpn",
            _ => "other",
        }
    };
    Ok((kind.into(), status.into()))
}

#[cfg(not(windows))]
fn connection_profile() -> Result<(String, String), String> {
    // Do not invent a Wi-Fi/Ethernet label on platforms without a native reader.
    Ok(("unknown".into(), "unknown".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskSource;

    fn usage(date: &str, source: TaskSource) -> DailyUsage {
        DailyUsage {
            date: date.into(),
            source,
            model: None,
            context_tier: "short".into(),
            input_tokens: 100,
            cached_input_tokens: 80,
            output_tokens: 20,
            reasoning_tokens: 15,
            total_tokens: 120,
        }
    }

    #[test]
    fn today_combines_sources_without_double_counting_cached_or_reasoning_tokens() {
        let result = sum_today(
            &[
                usage("2026-09-17", TaskSource::Codex),
                usage("2026-09-17", TaskSource::Cursor),
                usage("2026-09-16", TaskSource::Codex),
                usage("2026-09-18", TaskSource::Codex),
            ],
            "2026-09-17",
        );
        assert_eq!(
            (
                result.input_tokens,
                result.output_tokens,
                result.total_tokens
            ),
            (200, 40, 240)
        );
    }

    #[test]
    fn new_day_has_zero_recorded_usage() {
        assert_eq!(
            sum_today(&[usage("2026-09-30", TaskSource::Codex)], "2026-10-01").total_tokens,
            0
        );
    }

    #[test]
    fn configured_probes_are_https_sites_outside_the_local_network() {
        assert_eq!(
            PROBE_TARGETS.map(|target| (target.service, target.host, target.path)),
            [
                ("ChatGPT", "chatgpt.com", "/"),
                ("Google", "www.google.com", "/"),
            ]
        );
    }
}
