//! Systemd user service management.

use anyhow::{Context, Result, bail};
use log::info;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SERVICE_NAME: &str = "nielsen-tv-enabler.service";

/// Manager for installing, inspecting, and uninstalling the user systemd service.
pub struct SystemdManager;

impl SystemdManager {
    /// Returns the target systemd user service file path.
    ///
    /// # Errors
    /// Returns an error if the user configuration directory cannot be resolved.
    pub fn get_service_file_path() -> Result<PathBuf> {
        let Some(config_dir) = dirs::config_dir() else {
            bail!("Could not determine user config directory for systemd");
        };
        Ok(config_dir.join("systemd").join("user").join(SERVICE_NAME))
    }

    /// Installs and activates the background systemd service.
    ///
    /// # Errors
    /// Returns an error if binary location, file writing, or systemctl invocation fails.
    pub fn install() -> Result<()> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let exe_path = locate_or_install_binary(&home)?;
        let service_content = generate_service_unit(&exe_path, &home);
        let service_path = Self::get_service_file_path()?;

        if let Some(parent) = service_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&service_path, service_content).with_context(|| {
            format!("Failed to write service file at {}", service_path.display())
        })?;
        info!("Wrote systemd user unit to {}", service_path.display());

        reload_and_enable_service()?;
        info!("Service {SERVICE_NAME} enabled and started successfully!");
        info!("View logs anytime with: journalctl --user -u {SERVICE_NAME} -f");
        Ok(())
    }

    /// Stops, disables, and removes the systemd service.
    ///
    /// # Errors
    /// Returns an error if removing the unit file or running systemctl fails.
    pub fn uninstall() -> Result<()> {
        info!("Stopping and disabling {SERVICE_NAME}...");
        let _ = Command::new("systemctl")
            .args(["--user", "stop", SERVICE_NAME])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", SERVICE_NAME])
            .output();

        let service_path = Self::get_service_file_path()?;
        if service_path.exists() {
            fs::remove_file(&service_path)?;
            info!("Removed service unit file {}", service_path.display());
        }

        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        info!("Service {SERVICE_NAME} uninstalled successfully.");
        Ok(())
    }

    /// Displays the status of the systemd service.
    ///
    /// # Errors
    /// Returns an error if invoking systemctl fails.
    pub fn status() -> Result<()> {
        let status = Command::new("systemctl")
            .args(["--user", "status", SERVICE_NAME])
            .status()?;
        if !status.success() {
            info!("Run `journalctl --user -u {SERVICE_NAME} -n 50` for recent logs");
        }
        Ok(())
    }
}

/// Locates existing binary or installs current executable into ~/.cargo/bin or ~/.local/bin.
fn locate_or_install_binary(home: &Path) -> Result<PathBuf> {
    let cargo_bin = home.join(".cargo/bin/nielsen-tv-enabler");
    let local_bin = home.join(".local/bin/nielsen-tv-enabler");

    if cargo_bin.exists() {
        return Ok(cargo_bin);
    }
    if local_bin.exists() {
        return Ok(local_bin);
    }
    if let Ok(current_exe) = std::env::current_exe() {
        let target = if home.join(".cargo/bin").exists() {
            cargo_bin
        } else {
            fs::create_dir_all(home.join(".local/bin"))?;
            local_bin
        };
        fs::copy(&current_exe, &target)
            .with_context(|| format!("Failed to copy executable to {}", target.display()))?;
        info!("Installed binary to {}", target.display());
        return Ok(target);
    }
    Ok(cargo_bin)
}

/// Generates systemd unit file content.
fn generate_service_unit(exe_path: &Path, home: &Path) -> String {
    format!(
        r"[Unit]
Description=Nielsen Android TV Accessibility Keeper
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={} --daemon
Restart=always
RestartSec=15
Environment=PATH={}/.cargo/bin:{}/Android/Sdk/platform-tools:/usr/local/bin:/usr/bin:/bin
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
",
        exe_path.display(),
        home.display(),
        home.display()
    )
}

/// Runs daemon-reload and enables the service unit.
fn reload_and_enable_service() -> Result<()> {
    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()?;
    if !reload.status.success() {
        bail!("`systemctl --user daemon-reload` failed");
    }
    let enable = Command::new("systemctl")
        .args(["--user", "enable", "--now", SERVICE_NAME])
        .output()?;
    if !enable.status.success() {
        bail!("`systemctl --user enable --now {SERVICE_NAME}` failed");
    }
    let _ = Command::new("systemctl")
        .args(["--user", "restart", SERVICE_NAME])
        .output();
    Ok(())
}
