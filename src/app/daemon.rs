//! Continuous background daemon loop and TV network discovery.

use crate::config::Config;
use crate::domain::{prompt, service};
use crate::infra::adb::{AdbClient, DeviceStatus};
use crate::infra::scanner::Scanner;
use anyhow::{Result, bail};
use log::{info, warn};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Continuous daemon loop keeping accessibility active and prompts answered.
///
/// # Errors
/// Returns an error if an unrecoverable failure occurs in the daemon loop.
pub fn run_daemon(adb: &AdbClient, mut cfg: Config, config_path: &Path) -> Result<()> {
    info!("Starting Nielsen TV Enabler daemon");
    info!(
        "Check interval: {}s | Offline retry: {}s",
        cfg.check_interval_secs, cfg.offline_retry_interval_secs
    );

    let mut was_connected = false;
    let mut logged_offline = false;
    let mut cached_component =
        if !cfg.service_component.is_empty() && cfg.service_component != "auto" {
            Some(cfg.service_component.clone())
        } else {
            None
        };

    loop {
        match resolve_tv_ip(adb, &mut cfg, config_path) {
            Ok(ip) => {
                let target = format!("{ip}:{}", cfg.adb_port);
                let ready =
                    check_target_ready(adb, &ip, cfg.adb_port, &target, &mut logged_offline);

                if ready {
                    if !was_connected {
                        info!("Connected to Android TV at {target}");
                        was_connected = true;
                        logged_offline = false;
                    }

                    handle_active_cycle(adb, &target, &mut cached_component, &mut cfg, config_path);
                    thread::sleep(Duration::from_secs(cfg.check_interval_secs));
                    continue;
                }
                was_connected = false;
            }
            Err(e) => {
                if was_connected || !logged_offline {
                    info!("TV offline or unreachable ({e}). Waiting...");
                    was_connected = false;
                    logged_offline = true;
                }
            }
        }
        thread::sleep(Duration::from_secs(cfg.offline_retry_interval_secs));
    }
}

/// Discovers the TV IP address via attached devices, config, or network scan.
///
/// # Errors
/// Returns an error if the TV cannot be found on the network.
pub fn resolve_tv_ip(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<String> {
    if let Ok(attached) = adb.list_attached_devices() {
        for dev in attached {
            if let Some((ip_part, _)) = dev.serial.split_once(':') {
                if cfg.last_known_ip.as_deref() != Some(ip_part) {
                    cfg.last_known_ip = Some(ip_part.to_string());
                    let _ = cfg.save(config_path);
                }
                return Ok(ip_part.to_string());
            }
        }
    }

    if !cfg.tv_ip.is_empty() && cfg.tv_ip != "auto" {
        return Ok(cfg.tv_ip.clone());
    }

    if let Some(ref last_ip) = cfg.last_known_ip
        && Scanner::probe_tcp_port(last_ip, cfg.adb_port, Duration::from_millis(350))
    {
        return Ok(last_ip.clone());
    }

    let found = Scanner::scan_subnet_for_adb(
        cfg.adb_port,
        cfg.subnet_cidr.as_deref(),
        cfg.last_known_ip.as_deref(),
    )?;
    let Some(first) = found.first() else {
        bail!("Could not locate Android TV on local network. TV may be off or sleeping.");
    };

    info!("Auto-detected Android TV at {first}");
    cfg.last_known_ip = Some(first.clone());
    let _ = cfg.save(config_path);
    Ok(first.clone())
}

/// Verifies whether the target device is ready or logs unauthorized.
fn check_target_ready(
    adb: &AdbClient,
    ip: &str,
    port: u16,
    target: &str,
    logged_offline: &mut bool,
) -> bool {
    let status = match adb.check_device_status(target) {
        DeviceStatus::Ready => return true,
        DeviceStatus::Unauthorized => DeviceStatus::Unauthorized,
        _ => {
            let _ = adb.connect(ip, port);
            adb.check_device_status(target)
        }
    };

    if status == DeviceStatus::Unauthorized && !*logged_offline {
        warn!(
            "[ACTION REQUIRED] TV at {target} is UNAUTHORIZED! Please accept the prompt on TV screen."
        );
        *logged_offline = true;
    }
    status == DeviceStatus::Ready
}

/// Executes one monitoring cycle for accessibility service and who-is-watching prompt.
fn handle_active_cycle(
    adb: &AdbClient,
    target: &str,
    cached_component: &mut Option<String>,
    cfg: &mut Config,
    config_path: &Path,
) {
    let component = match cached_component.clone() {
        Some(c) => c,
        None => match service::detect_nielsen_service(adb, target) {
            Ok(Some(detected)) => {
                info!("Detected Nielsen accessibility component: {detected}");
                cfg.service_component.clone_from(&detected);
                let _ = cfg.save(config_path);
                *cached_component = Some(detected.clone());
                detected
            }
            _ => return,
        },
    };

    if let Ok(changed) = service::ensure_accessibility_enabled(adb, target, &component)
        && changed
    {
        info!("[STATUS] Nielsen accessibility service was re-enabled successfully!");
    }

    if cfg.auto_handle_who_is_watching {
        let _ = prompt::handle_who_is_watching(adb, target);
    }
}
