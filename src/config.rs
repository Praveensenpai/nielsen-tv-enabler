use anyhow::{Context, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// IP address of the Android TV. Set to "auto" or leave empty to auto-scan subnet.
    #[serde(default = "default_tv_ip")]
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
    #[serde(default = "default_service_component")]
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
    #[serde(default = "default_auto_handle_who_is_watching")]
    pub auto_handle_who_is_watching: bool,
}

fn default_auto_handle_who_is_watching() -> bool {
    true
}

fn default_tv_ip() -> String {
    "auto".to_string()
}

fn default_adb_port() -> u16 {
    5555
}

fn default_service_component() -> String {
    "auto".to_string()
}

fn default_check_interval() -> u64 {
    30
}

fn default_offline_retry() -> u64 {
    45
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tv_ip: default_tv_ip(),
            adb_port: default_adb_port(),
            last_known_ip: None,
            service_component: default_service_component(),
            check_interval_secs: default_check_interval(),
            offline_retry_interval_secs: default_offline_retry(),
            subnet_cidr: None,
            adb_path: None,
            auto_handle_who_is_watching: default_auto_handle_who_is_watching(),
        }
    }
}

impl Config {
    pub fn get_default_config_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("nielsen-tv-enabler").join("config.toml")
        } else {
            PathBuf::from("nielsen-tv-enabler.toml")
        }
    }

    pub fn load_or_create(custom_path: Option<&Path>) -> Result<(Self, PathBuf)> {
        let path = match custom_path {
            Some(p) => p.to_path_buf(),
            None => Self::get_default_config_path(),
        };

        if path.exists() {
            debug!("Loading configuration from {}", path.display());
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file at {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file at {}", path.display()))?;
            Ok((config, path))
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(&config)
                .context("Failed to serialize default config")?;
            fs::write(&path, toml_str)
                .with_context(|| format!("Failed to create default config at {}", path.display()))?;
            info!("Created default configuration at {}", path.display());
            Ok((config, path))
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        fs::write(path, toml_str)
            .with_context(|| format!("Failed to save config to {}", path.display()))?;
        Ok(())
    }
}
