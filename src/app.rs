//! Top-level application orchestration and CLI command dispatch.

pub mod daemon;

pub use daemon::run_daemon;

use crate::cli::Args;
use crate::config::Config;
use crate::domain::{prompt, service};
use crate::infra::adb::{AdbClient, DeviceStatus};
use crate::infra::scanner::Scanner;
use crate::infra::systemd::SystemdManager;
use anyhow::{Result, bail};
use log::info;
use std::path::Path;

/// Main application dispatcher handling CLI modes and daemon launch.
///
/// # Errors
/// Returns an error if any requested mode or service command fails.
pub fn run(args: &Args) -> Result<()> {
    if args.install_service {
        return SystemdManager::install();
    }
    if args.uninstall_service {
        return SystemdManager::uninstall();
    }
    if args.status_service {
        return SystemdManager::status();
    }

    let (mut cfg, config_path) = Config::load_or_create(args.config.as_deref())?;
    apply_cli_overrides(args, &mut cfg);
    let adb = AdbClient::new(cfg.adb_path.as_deref())?;

    if args.scan {
        return run_scan_mode(&cfg);
    }
    if args.detect {
        return run_detect_mode(&adb, &mut cfg, &config_path);
    }
    if args.dismiss_prompt {
        return run_dismiss_mode(&adb, &mut cfg, &config_path);
    }
    if args.once {
        return run_once_mode(&adb, &mut cfg, &config_path);
    }

    run_daemon(&adb, cfg, &config_path)
}

/// Overrides configuration options with values passed via CLI flags.
fn apply_cli_overrides(args: &Args, cfg: &mut Config) {
    if let Some(ref ip) = args.ip {
        cfg.tv_ip.clone_from(ip);
    }
    if let Some(port) = args.port {
        cfg.adb_port = port;
    }
    if let Some(ref s) = args.service {
        cfg.service_component.clone_from(s);
    }
    if let Some(interval) = args.interval {
        cfg.check_interval_secs = interval;
    }
}

/// Executes subnet scan and displays discovered ADB devices.
fn run_scan_mode(cfg: &Config) -> Result<()> {
    println!(
        "=== Scanning local network for Android TV / ADB (port {}) ===",
        cfg.adb_port
    );
    let found = Scanner::scan_subnet_for_adb(
        cfg.adb_port,
        cfg.subnet_cidr.as_deref(),
        cfg.last_known_ip.as_deref(),
    )?;

    if found.is_empty() {
        println!(
            "No devices with ADB port {} found on the local network.",
            cfg.adb_port
        );
    } else {
        println!("Found {} device(s):", found.len());
        for ip in &found {
            println!("  -> {ip}:{}", cfg.adb_port);
        }
    }
    Ok(())
}

/// Connects to device and prints detected Nielsen accessibility service.
fn run_detect_mode(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<()> {
    let (target, _) = resolve_target_device(adb, cfg, config_path)?;
    println!("Connected to device: {target}");

    if let Some(svc) = service::detect_nielsen_service(adb, &target)? {
        println!("Detected Nielsen accessibility component: {svc}");
        cfg.service_component = svc;
        let _ = cfg.save(config_path);
    } else {
        println!("Could not detect any Nielsen accessibility service on {target}");
        println!("Current enabled accessibility services:");
        for s in service::get_enabled_services(adb, &target)? {
            println!("  - {s}");
        }
    }
    Ok(())
}

/// Dismisses 'Who is watching?' prompt if currently active.
fn run_dismiss_mode(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<()> {
    let (target, _) = resolve_target_device(adb, cfg, config_path)?;
    if prompt::handle_who_is_watching(adb, &target)? {
        info!("'Who is watching?' prompt was found and answered.");
    } else {
        info!("No 'Who is watching?' prompt currently visible on screen.");
    }
    Ok(())
}

/// Executes a single check and enable pass.
fn run_once_mode(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<()> {
    let (target, component) = resolve_target_device(adb, cfg, config_path)?;
    info!("Running single accessibility check for {component} on {target}...");

    let updated = service::ensure_accessibility_enabled(adb, &target, &component)?;
    if updated {
        info!("Successfully enabled accessibility service.");
    } else {
        info!("Accessibility service was already active.");
    }

    if cfg.auto_handle_who_is_watching && prompt::handle_who_is_watching(adb, &target)? {
        info!("Auto-dismissed 'Who is watching?' dialog.");
    }
    Ok(())
}

/// Resolves target IP and connects, ensuring Nielsen component name is known.
///
/// # Errors
/// Returns an error if the target device cannot be reached or authorized.
pub fn resolve_target_device(
    adb: &AdbClient,
    cfg: &mut Config,
    config_path: &Path,
) -> Result<(String, String)> {
    let ip = daemon::resolve_tv_ip(adb, cfg, config_path)?;
    let target = format!("{ip}:{}", cfg.adb_port);

    ensure_device_ready(adb, &ip, cfg.adb_port, &target)?;
    let component = resolve_service_component(adb, cfg, config_path, &target)?;
    Ok((target, component))
}

/// Ensures device is connected and authorized.
fn ensure_device_ready(adb: &AdbClient, ip: &str, port: u16, target: &str) -> Result<()> {
    match adb.check_device_status(target) {
        DeviceStatus::Ready => Ok(()),
        DeviceStatus::Unauthorized => {
            bail!(
                "Device at {target} is UNAUTHORIZED. Please accept debugging prompt on TV screen."
            );
        }
        _ => {
            let _ = adb.connect(ip, port);
            match adb.check_device_status(target) {
                DeviceStatus::Ready => Ok(()),
                DeviceStatus::Unauthorized => {
                    bail!(
                        "Device at {target} is UNAUTHORIZED. Please accept debugging prompt on TV screen."
                    );
                }
                _ => bail!("Device at {target} is not ready or offline"),
            }
        }
    }
}

/// Resolves the Nielsen component name from config or auto-detection.
fn resolve_service_component(
    adb: &AdbClient,
    cfg: &mut Config,
    config_path: &Path,
    target: &str,
) -> Result<String> {
    if !cfg.service_component.is_empty() && cfg.service_component != "auto" {
        return Ok(cfg.service_component.clone());
    }

    let Some(detected) = service::detect_nielsen_service(adb, target)? else {
        bail!(
            "Failed to auto-detect Nielsen accessibility service on {target}. Please specify in config."
        );
    };

    info!("Auto-detected Nielsen accessibility component: {detected}");
    cfg.service_component.clone_from(&detected);
    let _ = cfg.save(config_path);
    Ok(detected)
}
