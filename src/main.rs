mod adb;
mod config;
mod scanner;
mod systemd;

use adb::AdbClient;
use anyhow::{bail, Result};
use clap::Parser;
use config::Config;
use log::{debug, info, warn};
use scanner::Scanner;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use systemd::SystemdManager;

#[derive(Parser, Debug)]
#[command(
    name = "nielsen-tv-enabler",
    author = "paisen",
    version = "0.1.0",
    about = "Keeps Nielsen Accessibility Service enabled on Android TV via ADB"
)]
struct Args {
    /// Run as a continuous background daemon (default if no other mode is selected)
    #[arg(long, short = 'd')]
    daemon: bool,

    /// Run once and exit
    #[arg(long)]
    once: bool,

    /// Scan local network for Android TV / ADB devices and exit
    #[arg(long)]
    scan: bool,

    /// Detect Nielsen accessibility service on target device and exit
    #[arg(long)]
    detect: bool,

    /// Check and answer 'Who is watching?' dialog immediately (selects member + clicks OK)
    #[arg(long, alias = "answer-prompt")]
    dismiss_prompt: bool,

    /// Install and enable systemd user service
    #[arg(long)]
    install_service: bool,

    /// Uninstall systemd user service
    #[arg(long)]
    uninstall_service: bool,

    /// Check status of systemd user service
    #[arg(long)]
    status_service: bool,

    /// Target Android TV IP address (overrides config)
    #[arg(long, short = 'i')]
    ip: Option<String>,

    /// Target ADB port (default: 5555)
    #[arg(long, short = 'p')]
    port: Option<u16>,

    /// Nielsen accessibility service component (overrides config)
    #[arg(long, short = 's')]
    service: Option<String>,

    /// Interval in seconds between checks in daemon mode
    #[arg(long, short = 't')]
    interval: Option<u64>,

    /// Path to config file
    #[arg(long, short = 'c')]
    config: Option<PathBuf>,

    /// Enable verbose / debug logging
    #[arg(long, short = 'v')]
    verbose: bool,
}

fn init_logger(verbose: bool) {
    let default_level = if verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
        .format_timestamp_secs()
        .init();
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_logger(args.verbose);

    // Handle systemd actions
    if args.install_service {
        return SystemdManager::install();
    }
    if args.uninstall_service {
        return SystemdManager::uninstall();
    }
    if args.status_service {
        return SystemdManager::status();
    }

    // Load or create config
    let (mut cfg, config_path) = Config::load_or_create(args.config.as_deref())?;

    // Apply CLI overrides to config
    if let Some(ip) = args.ip {
        cfg.tv_ip = ip;
    }
    if let Some(port) = args.port {
        cfg.adb_port = port;
    }
    if let Some(service) = args.service {
        cfg.service_component = service;
    }
    if let Some(interval) = args.interval {
        cfg.check_interval_secs = interval;
    }

    let adb = AdbClient::new(cfg.adb_path.as_deref())?;

    // Scan mode
    if args.scan {
        println!("=== Scanning local network for Android TV / ADB (port {}) ===", cfg.adb_port);
        let found = Scanner::scan_subnet_for_adb(
            cfg.adb_port,
            cfg.subnet_cidr.as_deref(),
            cfg.last_known_ip.as_deref(),
        )?;
        if found.is_empty() {
            println!("No devices with ADB port {} found on the local network.", cfg.adb_port);
        } else {
            println!("Found {} device(s):", found.len());
            for ip in &found {
                println!("  -> {}:{}", ip, cfg.adb_port);
            }
        }
        return Ok(());
    }

    // Detect mode
    if args.detect {
        let (target, _) = resolve_target_device(&adb, &mut cfg, &config_path)?;
        println!("Connected to device: {}", target);
        match adb.detect_nielsen_service(&target)? {
            Some(service) => {
                println!("Detected Nielsen accessibility component: {}", service);
                cfg.service_component = service;
                let _ = cfg.save(&config_path);
            }
            None => {
                println!("Could not detect any Nielsen accessibility service on {}", target);
                println!("Current enabled accessibility services:");
                for s in adb.get_enabled_accessibility_services(&target)? {
                    println!("  - {}", s);
                }
            }
        }
        return Ok(());
    }

    // Dismiss prompt mode
    if args.dismiss_prompt {
        let (target, _) = resolve_target_device(&adb, &mut cfg, &config_path)?;
        let dismissed = adb.handle_who_is_watching(&target)?;
        if dismissed {
            info!("'Who is watching?' prompt was found and dismissed.");
        } else {
            info!("No 'Who is watching?' prompt currently visible on screen.");
        }
        return Ok(());
    }

    // Once mode
    if args.once {
        let (target, component) = resolve_target_device(&adb, &mut cfg, &config_path)?;
        info!("Running single accessibility check for {} on {}...", component, target);
        let updated = adb.ensure_accessibility_enabled(&target, &component)?;
        if updated {
            info!("Successfully enabled accessibility service.");
        } else {
            info!("Accessibility service was already active.");
        }

        if cfg.auto_handle_who_is_watching {
            if let Ok(true) = adb.handle_who_is_watching(&target) {
                info!("Auto-dismissed 'Who is watching?' dialog.");
            }
        }
        return Ok(());
    }

    // Daemon mode (default)
    run_daemon(adb, cfg, config_path)
}

/// Resolves target IP and connects, then resolves Nielsen component
fn resolve_target_device(
    adb: &AdbClient,
    cfg: &mut Config,
    config_path: &std::path::Path,
) -> Result<(String, String)> {
    let ip = resolve_tv_ip(adb, cfg, config_path)?;
    let target = format!("{}:{}", ip, cfg.adb_port);

    // Ensure connected
    match adb.check_device_status(&target) {
        adb::DeviceStatus::Ready => {}
        adb::DeviceStatus::Unauthorized => {
            bail!(
                "Device at {} is UNAUTHORIZED.\n>> ACTION REQUIRED: Please look at your TV screen and select 'Always allow from this computer' to grant ADB access.",
                target
            );
        }
        _ => {
            debug!("Connecting to target {}", target);
            let _ = adb.connect(&ip, cfg.adb_port);
            match adb.check_device_status(&target) {
                adb::DeviceStatus::Ready => {}
                adb::DeviceStatus::Unauthorized => {
                    bail!(
                        "Device at {} is UNAUTHORIZED.\n>> ACTION REQUIRED: Please look at your TV screen and select 'Always allow from this computer' to grant ADB access.",
                        target
                    );
                }
                _ => {
                    bail!("Device at {} is not ready or offline", target);
                }
            }
        }
    }

    // Resolve Nielsen service component
    let component = if !cfg.service_component.is_empty() && cfg.service_component != "auto" {
        cfg.service_component.clone()
    } else {
        match adb.detect_nielsen_service(&target)? {
            Some(detected) => {
                info!("Auto-detected Nielsen accessibility component: {}", detected);
                cfg.service_component = detected.clone();
                let _ = cfg.save(config_path);
                detected
            }
            None => {
                bail!(
                    "Failed to auto-detect Nielsen accessibility service on {}. Please specify with --service or in {}",
                    target,
                    config_path.display()
                );
            }
        }
    };

    Ok((target, component))
}

/// Finds the TV IP either from config, attached devices, or network scan
fn resolve_tv_ip(
    adb: &AdbClient,
    cfg: &mut Config,
    config_path: &std::path::Path,
) -> Result<String> {
    // 1. Check if already attached in `adb devices`
    if let Ok(attached) = adb.list_attached_devices() {
        for dev in attached {
            if let Some((ip_part, _port)) = dev.serial.split_once(':') {
                debug!("Found already attached network device: {} ({})", dev.serial, dev.state);
                if cfg.last_known_ip.as_deref() != Some(ip_part) {
                    cfg.last_known_ip = Some(ip_part.to_string());
                    let _ = cfg.save(config_path);
                }
                return Ok(ip_part.to_string());
            }
        }
    }

    // 2. If an explicit IP is configured (not "auto" and not empty)
    if !cfg.tv_ip.is_empty() && cfg.tv_ip != "auto" {
        return Ok(cfg.tv_ip.clone());
    }

    // 3. Quick test last_known_ip first
    if let Some(ref last_ip) = cfg.last_known_ip {
        if Scanner::probe_tcp_port(last_ip, cfg.adb_port, Duration::from_millis(350)) {
            debug!("Last known IP {} is reachable", last_ip);
            return Ok(last_ip.clone());
        }
    }

    // 4. Subnet scan
    info!("Scanning local subnet for Android TV (port {})...", cfg.adb_port);
    let found = Scanner::scan_subnet_for_adb(
        cfg.adb_port,
        cfg.subnet_cidr.as_deref(),
        cfg.last_known_ip.as_deref(),
    )?;

    if let Some(first) = found.first() {
        info!("Auto-detected Android TV at {}", first);
        cfg.last_known_ip = Some(first.clone());
        let _ = cfg.save(config_path);
        Ok(first.clone())
    } else {
        bail!("Could not locate Android TV on local network. TV may be off or sleeping.");
    }
}

/// Continuous daemon loop
fn run_daemon(adb: AdbClient, mut cfg: Config, config_path: PathBuf) -> Result<()> {
    info!("Starting Nielsen TV Enabler daemon");
    info!(
        "Check interval: {}s | Offline retry: {}s | Config: {}",
        cfg.check_interval_secs,
        cfg.offline_retry_interval_secs,
        config_path.display()
    );

    let mut was_connected = false;
    let mut logged_offline = false;
    let mut cached_component: Option<String> = if !cfg.service_component.is_empty() && cfg.service_component != "auto" {
        Some(cfg.service_component.clone())
    } else {
        None
    };

    loop {
        // Step 1: Find TV IP
        let ip_result = resolve_tv_ip(&adb, &mut cfg, &config_path);

        match ip_result {
            Ok(ip) => {
                let target = format!("{}:{}", ip, cfg.adb_port);

                // Check or establish ADB connection
                let status = adb.check_device_status(&target);
                let is_ready = match status {
                    adb::DeviceStatus::Ready => true,
                    adb::DeviceStatus::Unauthorized => {
                        if !logged_offline {
                            warn!(
                                "[ACTION REQUIRED] TV at {} is connected but UNAUTHORIZED! Please check your TV screen and allow the debugging prompt.",
                                target
                            );
                            logged_offline = true;
                        }
                        false
                    }
                    _ => {
                        debug!("Attempting adb connect {}", target);
                        let _ = adb.connect(&ip, cfg.adb_port);
                        match adb.check_device_status(&target) {
                            adb::DeviceStatus::Ready => true,
                            adb::DeviceStatus::Unauthorized => {
                                if !logged_offline {
                                    warn!(
                                        "[ACTION REQUIRED] TV at {} is connected but UNAUTHORIZED! Please check your TV screen and allow the debugging prompt.",
                                        target
                                    );
                                    logged_offline = true;
                                }
                                false
                            }
                            _ => false,
                        }
                    }
                };

                if is_ready {
                    if !was_connected {
                        info!("Connected to Android TV at {}", target);
                        was_connected = true;
                        logged_offline = false;
                    }

                    // Step 2: Ensure we have the service component name
                    let component = match cached_component.clone() {
                        Some(comp) => comp,
                        None => match adb.detect_nielsen_service(&target) {
                            Ok(Some(detected)) => {
                                info!("Detected Nielsen accessibility component: {}", detected);
                                cfg.service_component = detected.clone();
                                let _ = cfg.save(&config_path);
                                cached_component = Some(detected.clone());
                                detected
                            }
                            Ok(None) => {
                                warn!("Could not detect Nielsen accessibility component on {}. Will retry.", target);
                                thread::sleep(Duration::from_secs(cfg.check_interval_secs));
                                continue;
                            }
                            Err(e) => {
                                warn!("Error querying accessibility services: {}", e);
                                thread::sleep(Duration::from_secs(cfg.check_interval_secs));
                                continue;
                            }
                        },
                    };

                    // Step 3: Check and enforce accessibility
                    match adb.ensure_accessibility_enabled(&target, &component) {
                        Ok(changed) => {
                            if changed {
                                info!("[STATUS] Nielsen accessibility service was re-enabled successfully!");
                            } else {
                                debug!("Nielsen accessibility service is confirmed active.");
                            }
                        }
                        Err(e) => {
                            warn!("Failed to verify or enable accessibility service: {}", e);
                        }
                    }

                    // Step 4: Check and auto-dismiss 'Who is watching?' prompt
                    if cfg.auto_handle_who_is_watching {
                        if let Err(e) = adb.handle_who_is_watching(&target) {
                            debug!("Who-is-watching check error: {}", e);
                        }
                    }

                    thread::sleep(Duration::from_secs(cfg.check_interval_secs));
                    continue;
                } else {
                    if was_connected || !logged_offline {
                        info!("Android TV at {} is offline or not responding. Waiting for TV...", target);
                        was_connected = false;
                        logged_offline = true;
                    }
                }
            }
            Err(e) => {
                if was_connected || !logged_offline {
                    info!("TV offline or unreachable ({}). Waiting...", e);
                    was_connected = false;
                    logged_offline = true;
                }
            }
        }

        // TV offline backoff
        thread::sleep(Duration::from_secs(cfg.offline_retry_interval_secs));
    }
}
