use anyhow::{bail, Context, Result};
use log::info;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "nielsen-tv-enabler.service";

pub struct SystemdManager;

impl SystemdManager {
    pub fn get_service_file_path() -> Result<PathBuf> {
        if let Some(config_dir) = dirs::config_dir() {
            Ok(config_dir.join("systemd").join("user").join(SERVICE_NAME))
        } else {
            bail!("Could not determine user config directory for systemd");
        }
    }

    pub fn install() -> Result<()> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let cargo_bin = home.join(".cargo").join("bin").join("nielsen-tv-enabler");
        let local_bin = home.join(".local").join("bin").join("nielsen-tv-enabler");

        // Determine executable path
        let exe_path = if cargo_bin.exists() {
            cargo_bin
        } else if local_bin.exists() {
            local_bin
        } else if let Ok(current_exe) = std::env::current_exe() {
            // Copy current executable to ~/.cargo/bin if it exists, or ~/.local/bin
            let target = if home.join(".cargo/bin").exists() {
                cargo_bin
            } else {
                fs::create_dir_all(home.join(".local/bin"))?;
                local_bin
            };
            fs::copy(&current_exe, &target).with_context(|| {
                format!("Failed to copy executable to {}", target.display())
            })?;
            info!("Installed binary to {}", target.display());
            target
        } else {
            cargo_bin
        };

        let service_content = format!(
            r#"[Unit]
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
"#,
            exe_path.display(),
            home.display(),
            home.display()
        );

        let service_path = Self::get_service_file_path()?;
        if let Some(parent) = service_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&service_path, service_content)
            .with_context(|| format!("Failed to write service file at {}", service_path.display()))?;
        info!("Wrote systemd user unit to {}", service_path.display());

        // Reload systemd
        let reload = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;
        if !reload.status.success() {
            bail!("`systemctl --user daemon-reload` failed");
        }

        // Enable and start service
        let enable = Command::new("systemctl")
            .args(["--user", "enable", "--now", SERVICE_NAME])
            .output()?;
        if !enable.status.success() {
            bail!("`systemctl --user enable --now {}` failed", SERVICE_NAME);
        }

        info!("Service {} enabled and started successfully!", SERVICE_NAME);
        info!("View logs anytime with: journalctl --user -u {} -f", SERVICE_NAME);
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        info!("Stopping and disabling {}...", SERVICE_NAME);
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

        info!("Service {} uninstalled successfully.", SERVICE_NAME);
        Ok(())
    }

    pub fn status() -> Result<()> {
        let status = Command::new("systemctl")
            .args(["--user", "status", SERVICE_NAME])
            .status()?;
        if !status.success() {
            info!("Run `journalctl --user -u {} -n 50` for recent logs", SERVICE_NAME);
        }
        Ok(())
    }
}
