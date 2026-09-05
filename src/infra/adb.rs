//! Pure ADB communication adapter.

use crate::domain::DeviceCommander;
use anyhow::{Context, Result, bail};
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Lifecycle connection status of a targeted Android device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is connected, authorized, and responsive.
    Ready,
    /// Device is connected but requires user authorization on screen.
    Unauthorized,
    /// Device is offline or unreachable.
    Offline,
    /// Device was not found in attached devices list.
    NotFound,
}

/// Attached device entry from `adb devices`.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Serial or network IP:PORT.
    pub serial: String,
    /// ADB device state string.
    pub state: String,
}

/// Client for communicating with the ADB daemon.
#[derive(Clone, Debug)]
pub struct AdbClient {
    /// Resolved absolute path to `adb` binary.
    pub adb_binary: PathBuf,
}

impl AdbClient {
    /// Creates a new `AdbClient`, verifying that `adb` binary is available.
    ///
    /// # Errors
    /// Returns an error if the `adb` binary cannot be located or executed.
    pub fn new(custom_path: Option<&str>) -> Result<Self> {
        let path = resolve_adb_path(custom_path);
        verify_adb_binary(&path)?;
        debug!("Using ADB binary: {}", path.display());
        Ok(Self { adb_binary: path })
    }

    /// Executes an ADB command with the specified arguments.
    ///
    /// # Errors
    /// Returns an error if the process fails to spawn or exits with a non-zero status.
    pub fn run_adb(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.adb_binary)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run adb with args {args:?}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            let err_msg = if stderr.is_empty() { stdout } else { stderr };
            bail!("ADB error: {err_msg}");
        }

        Ok(stdout)
    }

    /// Connects to a network device over TCP.
    ///
    /// # Errors
    /// Returns an error if the ADB command fails.
    pub fn connect(&self, ip: &str, port: u16) -> Result<bool> {
        let target = format!("{ip}:{port}");
        debug!("Running adb connect {target}");
        let output = self.run_adb(&["connect", &target])?;

        if output.to_lowercase().contains("connected to") {
            Ok(true)
        } else {
            warn!("adb connect response: {output}");
            Ok(false)
        }
    }

    /// Checks the current connection status of a targeted device.
    #[must_use]
    pub fn check_device_status(&self, device_target: &str) -> DeviceStatus {
        let Ok(devices) = self.list_attached_devices() else {
            return DeviceStatus::NotFound;
        };

        for d in devices {
            if d.serial == device_target {
                return match d.state.as_str() {
                    "device" => DeviceStatus::Ready,
                    "unauthorized" => DeviceStatus::Unauthorized,
                    _ => DeviceStatus::Offline,
                };
            }
        }
        DeviceStatus::NotFound
    }

    /// Lists all attached devices recognized by the ADB daemon.
    ///
    /// # Errors
    /// Returns an error if querying attached devices fails.
    pub fn list_attached_devices(&self) -> Result<Vec<DeviceInfo>> {
        let output = self.run_adb(&["devices"])?;
        let mut devices = Vec::new();

        for line in output.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                devices.push(DeviceInfo {
                    serial: parts[0].to_string(),
                    state: parts[1].to_string(),
                });
            }
        }

        Ok(devices)
    }
}

impl DeviceCommander for AdbClient {
    fn run_shell(&self, device_target: &str, command: &str) -> Result<String> {
        self.run_adb(&["-s", device_target, "shell", command])
    }
}

/// Resolves path to the adb binary from custom path, PATH, or Android SDK.
fn resolve_adb_path(custom_path: Option<&str>) -> PathBuf {
    if let Some(custom) = custom_path {
        return PathBuf::from(custom);
    }
    if let Ok(which_path) = which_adb() {
        return which_path;
    }
    if let Some(home) = dirs::home_dir() {
        let sdk_adb = home.join("Android/Sdk/platform-tools/adb");
        if sdk_adb.is_file() {
            return sdk_adb;
        }
    }
    PathBuf::from("adb")
}

/// Verifies that the adb executable runs.
fn verify_adb_binary(path: &Path) -> Result<()> {
    let output = Command::new(path)
        .arg("version")
        .output()
        .with_context(|| format!("Failed to execute adb at '{}'", path.display()))?;

    if !output.status.success() {
        bail!(
            "`{} version` failed with {:?}",
            path.display(),
            output.status
        );
    }
    Ok(())
}

/// Finds adb in system PATH.
fn which_adb() -> Result<PathBuf> {
    let output = Command::new("which").arg("adb").output()?;
    if !output.status.success() {
        bail!("`adb` not found via `which`");
    }
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path_str.is_empty() {
        bail!("Empty path from `which adb`");
    }
    Ok(PathBuf::from(path_str))
}
