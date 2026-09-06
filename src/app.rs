//! Top-level application orchestration and CLI command dispatch.

pub mod daemon;

pub use daemon::run_daemon;

use crate::cli::Args;
use crate::config::Config;
use crate::domain::{prompt, service, sync, vpn};
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
    if args.vpn {
        return run_vpn_mode(&adb, &mut cfg, &config_path);
    }
    if args.sync {
        return run_sync_mode(&adb, &mut cfg, &config_path);
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

    let package = component
        .split_once('/')
        .map_or(vpn::DEFAULT_NIELSEN_PACKAGE, |(pkg, _)| pkg);

    if cfg.auto_allow_vpn {
        let _ = vpn::ensure_vpn_enabled(adb, &target, package);
    }

    if cfg.auto_handle_who_is_watching && prompt::handle_who_is_watching(adb, &target)? {
        info!("Auto-dismissed 'Who is watching?' dialog.");
    }

    if cfg.daily_sync {
        info!(
            "Waiting {}s before running initial data sync...",
            cfg.sync_delay_secs
        );
        std::thread::sleep(std::time::Duration::from_secs(cfg.sync_delay_secs));
        let _ = sync::trigger_background_sync(adb, &target, package);
    }
    Ok(())
}

/// Grants all background permissions and always-on VPN via ADB, and handles any active VPN dialog.
fn run_vpn_mode(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<()> {
    let (target, component) = resolve_target_device(adb, cfg, config_path)?;
    let package = component
        .split_once('/')
        .map_or(vpn::DEFAULT_NIELSEN_PACKAGE, |(pkg, _)| pkg);

    let changed = vpn::ensure_vpn_enabled(adb, &target, package)?;
    if changed {
        info!("Configured and enabled VPN for {package}.");
    } else {
        info!("VPN was already active and configured; left running untouched.");
    }
    Ok(())
}

/// Triggers background data synchronization via ADB immediately without opening the app UI.
fn run_sync_mode(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<()> {
    let (target, component) = resolve_target_device(adb, cfg, config_path)?;
    let package = component
        .split_once('/')
        .map_or(vpn::DEFAULT_NIELSEN_PACKAGE, |(pkg, _)| pkg);

    sync::trigger_background_sync(adb, &target, package)?;
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

    match adb.check_device_status(&target) {
        DeviceStatus::Ready => (),
        DeviceStatus::Unauthorized => {
            bail!("Device at {target} is UNAUTHORIZED. Please accept prompt on TV screen.");
        }
        _ => {
            let _ = adb.connect(&ip, cfg.adb_port);
            if adb.check_device_status(&target) != DeviceStatus::Ready {
                bail!("Device at {target} is not ready or offline");
            }
        }
    }

    let component = if !cfg.service_component.is_empty() && cfg.service_component != "auto" {
        cfg.service_component.clone()
    } else {
        let Some(detected) = service::detect_nielsen_service(adb, &target)? else {
            bail!("Failed to auto-detect Nielsen accessibility service on {target}.");
        };
        info!("Auto-detected Nielsen accessibility component: {detected}");
        cfg.service_component.clone_from(&detected);
        let _ = cfg.save(config_path);
        detected
    };

    Ok((target, component))
}
