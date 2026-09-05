use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Ready,
    Unauthorized,
    Offline,
    NotFound,
}

#[derive(Clone, Debug)]
pub struct AdbClient {
    pub adb_binary: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub serial: String,
    pub state: String,
}

impl AdbClient {
    pub fn new(custom_path: Option<&str>) -> Result<Self> {
        let path = if let Some(custom) = custom_path {
            PathBuf::from(custom)
        } else if let Ok(which_path) = which_adb() {
            which_path
        } else {
            // Check common Android SDK path in user's home
            if let Some(home) = dirs::home_dir() {
                let sdk_adb = home.join("Android/Sdk/platform-tools/adb");
                if sdk_adb.is_file() {
                    sdk_adb
                } else {
                    PathBuf::from("adb")
                }
            } else {
                PathBuf::from("adb")
            }
        };

        // Test running adb version
        let output = Command::new(&path)
            .arg("version")
            .output()
            .with_context(|| format!("Failed to execute adb at '{}'", path.display()))?;

        if !output.status.success() {
            bail!("`{} version` failed with status {:?}", path.display(), output.status);
        }

        debug!("Using ADB binary: {}", path.display());
        Ok(Self { adb_binary: path })
    }

    /// Execute adb command with arbitrary arguments.
    pub fn run_adb(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.adb_binary)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run adb with args {:?}", args))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            let err_msg = if !stderr.is_empty() { stderr } else { stdout };
            bail!("ADB error: {}", err_msg);
        }

        Ok(stdout)
    }

    /// Execute an ADB shell command on a specific device.
    pub fn run_shell(&self, device_target: &str, command: &str) -> Result<String> {
        self.run_adb(&["-s", device_target, "shell", command])
    }

    /// Connect to a network device (e.g. "192.168.1.39:5555").
    pub fn connect(&self, ip: &str, port: u16) -> Result<bool> {
        let target = format!("{}:{}", ip, port);
        debug!("Running adb connect {}", target);
        let output = self.run_adb(&["connect", &target])?;
        debug!("adb connect output: {}", output);

        // "connected to 192.168.1.39:5555" or "already connected to 192.168.1.39:5555"
        if output.to_lowercase().contains("connected to") {
            Ok(true)
        } else {
            warn!("adb connect response: {}", output);
            Ok(false)
        }
    }

    /// Disconnect a network device.
    #[allow(dead_code)]
    pub fn disconnect(&self, ip: &str, port: u16) -> Result<()> {
        let target = format!("{}:{}", ip, port);
        let _ = self.run_adb(&["disconnect", &target]);
        Ok(())
    }

    /// Check `adb get-state` for the device. Returns "device", "offline", "unauthorized", etc.
    #[allow(dead_code)]
    pub fn get_device_state(&self, device_target: &str) -> Result<String> {
        self.run_adb(&["-s", device_target, "get-state"])
    }

    /// Check the status of a device (Ready, Unauthorized, Offline, NotFound)
    pub fn check_device_status(&self, device_target: &str) -> DeviceStatus {
        if let Ok(devices) = self.list_attached_devices() {
            for d in devices {
                if d.serial == device_target {
                    return match d.state.as_str() {
                        "device" => DeviceStatus::Ready,
                        "unauthorized" => DeviceStatus::Unauthorized,
                        "offline" => DeviceStatus::Offline,
                        _ => DeviceStatus::Offline,
                    };
                }
            }
        }
        DeviceStatus::NotFound
    }

    /// Check if device is online and ready ("device" state).
    #[allow(dead_code)]
    pub fn is_device_ready(&self, device_target: &str) -> bool {
        self.check_device_status(device_target) == DeviceStatus::Ready
    }

    /// List all devices currently attached to ADB.
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

    /// Query the currently enabled accessibility services.
    /// Returns a list of component strings like `["com.nielsen.tv/.AccessibilityService"]`.
    pub fn get_enabled_accessibility_services(&self, device_target: &str) -> Result<Vec<String>> {
        let output = self.run_shell(
            device_target,
            "settings get secure enabled_accessibility_services",
        )?;

        let trimmed = output.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return Ok(Vec::new());
        }

        // Services are colon separated
        let services: Vec<String> = trimmed
            .split(':')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(services)
    }

    /// Check if `accessibility_enabled` global toggle is set to 1.
    pub fn is_accessibility_globally_enabled(&self, device_target: &str) -> Result<bool> {
        let output = self.run_shell(device_target, "settings get secure accessibility_enabled")?;
        Ok(output.trim() == "1")
    }

    /// Auto-detect Nielsen accessibility service component from the target device.
    pub fn detect_nielsen_service(&self, device_target: &str) -> Result<Option<String>> {
        info!("Attempting to auto-detect Nielsen accessibility service on {}...", device_target);

        // 1. Try `dumpsys accessibility`
        if let Ok(dumpsys) = self.run_shell(device_target, "dumpsys accessibility") {
            // Match pattern like: com.nlsn.xxx/...Service, com.nielsen.xxx/...Service
            let regex = Regex::new(r"(?i)\b([a-zA-Z0-9._]*(?:nlsn|nielsen)[a-zA-Z0-9._]*/[a-zA-Z0-9._]+)\b")?;
            for cap in regex.captures_iter(&dumpsys) {
                let candidate = cap[1].to_string();
                info!("Found Nielsen accessibility component from dumpsys: {}", candidate);
                return Ok(Some(candidate));
            }
        }

        // 2. Try `cmd accessibility get-installed-accessibility-services`
        if let Ok(cmd_out) = self.run_shell(device_target, "cmd accessibility get-installed-accessibility-services") {
            let regex = Regex::new(r"(?i)\b([a-zA-Z0-9._]*(?:nlsn|nielsen)[a-zA-Z0-9._]*/[a-zA-Z0-9._]+)\b")?;
            if let Some(cap) = regex.captures(&cmd_out) {
                let candidate = cap[1].to_string();
                info!("Found Nielsen accessibility component from cmd: {}", candidate);
                return Ok(Some(candidate));
            }
        }

        // 3. Search packages for "nielsen" or "nlsn"
        if let Ok(pm_out) = self.run_shell(device_target, "pm list packages") {
            for line in pm_out.lines() {
                let pkg = line.trim().trim_start_matches("package:").trim();
                let lower = pkg.to_lowercase();
                if lower.contains("nielsen") || lower.contains("nlsn") {
                    info!("Found Nielsen package: {}", pkg);
                    // Query package details for services
                    if let Ok(pkg_dump) = self.run_shell(device_target, &format!("dumpsys package {}", pkg)) {
                        // Look for Service: entries
                        let service_regex = Regex::new(&format!(r"(?i)\b({}/[a-zA-Z0-9._]+)\b", regex::escape(pkg)))?;
                        for cap in service_regex.captures_iter(&pkg_dump) {
                            let candidate = cap[1].to_string();
                            if candidate.to_lowercase().contains("service") || candidate.to_lowercase().contains("accessibility") {
                                info!("Found Nielsen accessibility service: {}", candidate);
                                return Ok(Some(candidate));
                            }
                        }
                    }

                    // Common fallback if class name is standard
                    let standard_candidate = format!("{}/.AccessibilityService", pkg);
                    warn!("Could not find exact class name in dumpsys, trying standard component: {}", standard_candidate);
                    return Ok(Some(standard_candidate));
                }
            }
        }

        Ok(None)
    }

    /// Ensure Nielsen accessibility service is enabled on the device.
    /// Preserves any other accessibility services already enabled!
    pub fn ensure_accessibility_enabled(
        &self,
        device_target: &str,
        nielsen_component: &str,
    ) -> Result<bool> {
        let current_services = self.get_enabled_accessibility_services(device_target)?;
        let global_enabled = self.is_accessibility_globally_enabled(device_target)?;

        let already_present = current_services.iter().any(|s| {
            s == nielsen_component
                || ((s.to_lowercase().contains("nielsen") || s.to_lowercase().contains("nlsn"))
                    && (nielsen_component.to_lowercase().contains("nielsen") || nielsen_component.to_lowercase().contains("nlsn")))
        });

        if already_present && global_enabled {
            debug!("Nielsen accessibility service is already enabled ({})", nielsen_component);
            return Ok(false); // Nothing modified
        }

        // Build new list: preserve all other services, add nielsen_component if not present
        let mut new_services = current_services.clone();
        if !new_services.iter().any(|s| s == nielsen_component) {
            new_services.push(nielsen_component.to_string());
        }

        let new_services_str = new_services.join(":");
        info!(
            "Enabling Nielsen accessibility service (setting enabled_accessibility_services='{}')...",
            new_services_str
        );

        self.run_shell(
            device_target,
            &format!("settings put secure enabled_accessibility_services \"{}\"", new_services_str),
        )?;

        if !global_enabled {
            info!("Enabling global accessibility switch (accessibility_enabled=1)...");
            self.run_shell(device_target, "settings put secure accessibility_enabled 1")?;
        }

        // Verify changes took effect
        let verified_services = self.get_enabled_accessibility_services(device_target)?;
        let verified_global = self.is_accessibility_globally_enabled(device_target)?;

        let is_ok = verified_services.iter().any(|s| s == nielsen_component) && verified_global;
        if is_ok {
            info!("Successfully enabled Nielsen accessibility service on {}!", device_target);
            Ok(true)
        } else {
            warn!(
                "Verification after enabling showed services={:?}, global={}",
                verified_services, verified_global
            );
            Ok(false)
        }
    }

    /// Checks if the "Who is watching?" dialog is currently on screen.
    /// If found, randomly selects one viewer, clicks OK, and restores previous view.
    pub fn handle_who_is_watching(&self, device_target: &str) -> Result<bool> {
        // Fast pre-check: see if window focus involves Nielsen
        let focus = match self.run_shell(device_target, "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'") {
            Ok(f) => f,
            Err(_) => return Ok(false),
        };

        if !focus.contains("com.nlsn.confluencetv") {
            return Ok(false);
        }

        // Dump UI hierarchy
        let dump_cmd = "uiautomator dump /data/local/tmp/uidump.xml >/dev/null 2>&1 && cat /data/local/tmp/uidump.xml";
        let ui_dump = match self.run_shell(device_target, dump_cmd) {
            Ok(d) => d,
            Err(_) => return Ok(false),
        };

        if !ui_dump.contains("Who is watching?") && !ui_dump.contains("buttonOk") {
            return Ok(false);
        }

        // Parse member checkboxes
        let node_re = Regex::new(r#"<node\b[^>]*\bresource-id="com\.nlsn\.confluencetv:id/checkbox"[^>]*>"#)?;
        let text_re = Regex::new(r#"text="([^"]*)""#)?;
        let bounds_re = Regex::new(r#"bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]""#)?;

        let mut options = Vec::new();
        for node_match in node_re.find_iter(&ui_dump) {
            let node_str = node_match.as_str();
            let name = text_re
                .captures(node_str)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            if let Some(bounds) = bounds_re.captures(node_str) {
                let x1: u32 = bounds[1].parse().unwrap_or(0);
                let y1: u32 = bounds[2].parse().unwrap_or(0);
                let x2: u32 = bounds[3].parse().unwrap_or(0);
                let y2: u32 = bounds[4].parse().unwrap_or(0);
                let cx = (x1 + x2) / 2;
                let cy = (y1 + y2) / 2;
                options.push((name, cx, cy));
            }
        }

        if options.is_empty() {
            return Ok(false);
        }

        // Pick one option randomly
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let rand_idx = (now.as_nanos() as usize) % options.len();
        let (selected_name, sel_x, sel_y) = &options[rand_idx];

        info!(
            "[PROMPT DETECTED] 'Who is watching?' detected on TV! Randomly selecting '{}' at ({}, {})...",
            selected_name, sel_x, sel_y
        );

        // Tap the chosen checkbox
        self.run_shell(device_target, &format!("input tap {} {}", sel_x, sel_y))?;
        std::thread::sleep(std::time::Duration::from_millis(300));

        // Find OK button
        let ok_node_re = Regex::new(r#"<node\b[^>]*\bresource-id="com\.nlsn\.confluencetv:id/buttonOk"[^>]*>"#)?;
        let mut ok_x = 233;
        let mut ok_y = 471;
        if let Some(ok_match) = ok_node_re.find(&ui_dump) {
            if let Some(bounds) = bounds_re.captures(ok_match.as_str()) {
                let x1: u32 = bounds[1].parse().unwrap_or(0);
                let y1: u32 = bounds[2].parse().unwrap_or(0);
                let x2: u32 = bounds[3].parse().unwrap_or(0);
                let y2: u32 = bounds[4].parse().unwrap_or(0);
                ok_x = (x1 + x2) / 2;
                ok_y = (y1 + y2) / 2;
            }
        }

        info!("Clicking OK button at ({}, {})...", ok_x, ok_y);
        self.run_shell(device_target, &format!("input tap {} {}", ok_x, ok_y))?;
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Dismiss the dashboard if it was left open so user can keep watching TV
        if let Ok(after_focus) = self.run_shell(device_target, "dumpsys window | grep -E 'mCurrentFocus'") {
            if after_focus.contains("com.nlsn.confluencetv") {
                debug!("Dismissing Dashboard overlay via BACK key...");
                let _ = self.run_shell(device_target, "input keyevent 4");
            }
        }

        info!(
            "[SUCCESS] Successfully responded to 'Who is watching?' with '{}'!",
            selected_name
        );
        Ok(true)
    }
}

fn which_adb() -> Result<PathBuf> {
    if let Ok(output) = Command::new("which").arg("adb").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                return Ok(PathBuf::from(path_str));
            }
        }
    }
    bail!("`adb` not found via `which`");
}
