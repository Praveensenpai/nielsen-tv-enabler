//! Application configuration loader and persistence.

use anyhow::{Context, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// IP address of the Android TV. Set to "auto" or leave empty to auto-scan subnet.
    #[serde(default = "default_auto")]
    pub tv_ip: String,

    /// ADB port (typically 5555)
    #[serde(default = "default_adb_port")]
    pub adb_port: u16,

    /// Last known working IP of the TV, automatically updated.
    #[serde(default)]
    pub last_known_ip: Option<String>,

    /// Nielsen accessibility service component, e.g.:
    /// "com.nielsen.mobile/.AccessibilityService"
    /// Leave empty or "auto" to auto-detect from the TV.
    #[serde(default = "default_auto")]
    pub service_component: String,

    /// Interval in seconds between accessibility checks while connected.
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,

    /// Interval in seconds before retrying when TV is offline / unreachable.
    #[serde(default = "default_offline_retry")]
    pub offline_retry_interval_secs: u64,

    /// Custom subnet CIDR to scan (e.g. "192.168.1.0/24"). Leave empty to auto-detect.
    #[serde(default)]
    pub subnet_cidr: Option<String>,

    /// Path to adb binary (if not in standard PATH or Android SDK)
    #[serde(default)]
    pub adb_path: Option<String>,

    /// Automatically dismiss the 'Who is watching?' dialog by randomly selecting a member.
    #[serde(default = "default_true")]
    pub auto_handle_who_is_watching: bool,

    /// Automatically grant VPN permission and approve VPN connection request dialogs.
    #[serde(default = "default_true")]
    pub auto_allow_vpn: bool,

    /// Automatically trigger background data sync once a day after enabling services.
    #[serde(default = "default_true")]
    pub daily_sync: bool,

    /// Delay in seconds after enabling services before triggering background daily sync.
    #[serde(default = "default_sync_delay")]
    pub sync_delay_secs: u64,
}

const fn default_true() -> bool {
    true
}

fn default_auto() -> String {
    "auto".to_string()
}

const fn default_adb_port() -> u16 {
    5555
}

const fn default_check_interval() -> u64 {
    5
}

const fn default_offline_retry() -> u64 {
    45
}

const fn default_sync_delay() -> u64 {
    10
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tv_ip: default_auto(),
            adb_port: default_adb_port(),
            last_known_ip: None,
            service_component: default_auto(),
            check_interval_secs: default_check_interval(),
            offline_retry_interval_secs: default_offline_retry(),
            subnet_cidr: None,
            adb_path: None,
            auto_handle_who_is_watching: default_true(),
            auto_allow_vpn: default_true(),
            daily_sync: default_true(),
            sync_delay_secs: default_sync_delay(),
        }
    }
}

impl Config {
    /// Returns the standard default user config path in `~/.config/nielsen-tv-enabler/config.toml`.
    #[must_use]
    pub fn get_default_config_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("nielsen-tv-enabler").join("config.toml")
        } else {
            PathBuf::from("nielsen-tv-enabler.toml")
        }
    }

    /// Loads existing configuration from disk or creates default configuration file.
    ///
    /// # Errors
    /// Returns an error if reading or creating the configuration file fails.
    pub fn load_or_create(custom_path: Option<&Path>) -> Result<(Self, PathBuf)> {
        let path = match custom_path {
            Some(p) => p.to_path_buf(),
            None => Self::get_default_config_path(),
        };

        if path.exists() {
            debug!("Loading configuration from {}", path.display());
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file at {}", path.display()))?;
            let config: Self = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file at {}", path.display()))?;
            Ok((config, path))
        } else {
            let config = Self::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let toml_str =
                toml::to_string_pretty(&config).context("Failed to serialize default config")?;
            fs::write(&path, toml_str).with_context(|| {
                format!("Failed to create default config at {}", path.display())
            })?;
            info!("Created default configuration at {}", path.display());
            Ok((config, path))
        }
    }

    /// Saves the current configuration to the specified file path.
    ///
    /// # Errors
    /// Returns an error if serializing or writing to the file fails.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, toml_str)
            .with_context(|| format!("Failed to save config to {}", path.display()))?;
        Ok(())
    }
}
