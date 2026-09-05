//! Domain logic for inspecting and keeping Nielsen Accessibility service enabled.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::{debug, info, warn};
use regex::Regex;

/// Queries the currently enabled accessibility services list from Android settings.
///
/// # Errors
/// Returns an error if querying device settings fails.
pub fn get_enabled_services(
    device: &impl DeviceCommander,
    device_target: &str,
) -> Result<Vec<String>> {
    let output = device.run_shell(
        device_target,
        "settings get secure enabled_accessibility_services",
    )?;
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }

    let services: Vec<String> = trimmed
        .split(':')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(services)
}

/// Checks if the master accessibility switch (`accessibility_enabled`) is set to 1.
///
/// # Errors
/// Returns an error if querying device settings fails.
pub fn is_global_accessibility_enabled(
    device: &impl DeviceCommander,
    device_target: &str,
) -> Result<bool> {
    let output = device.run_shell(device_target, "settings get secure accessibility_enabled")?;
    Ok(output.trim() == "1")
}

/// Auto-detects the installed Nielsen accessibility service component name.
///
/// # Errors
/// Returns an error if communicating with the device fails.
pub fn detect_nielsen_service(
    device: &impl DeviceCommander,
    device_target: &str,
) -> Result<Option<String>> {
    info!("Attempting to auto-detect Nielsen accessibility service on {device_target}...");

    if let Some(candidate) = detect_from_dumpsys(device, device_target) {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = detect_from_cmd(device, device_target) {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = detect_from_packages(device, device_target) {
        return Ok(Some(candidate));
    }

    Ok(None)
}

/// Ensures that the Nielsen accessibility service is enabled while preserving other services.
///
/// # Errors
/// Returns an error if running settings or verification commands fails.
pub fn ensure_accessibility_enabled(
    device: &impl DeviceCommander,
    device_target: &str,
    nielsen_component: &str,
) -> Result<bool> {
    let current_services = get_enabled_services(device, device_target)?;
    let global_enabled = is_global_accessibility_enabled(device, device_target)?;

    let already_present = current_services.iter().any(|s| {
        s == nielsen_component
            || ((s.to_lowercase().contains("nielsen") || s.to_lowercase().contains("nlsn"))
                && (nielsen_component.to_lowercase().contains("nielsen")
                    || nielsen_component.to_lowercase().contains("nlsn")))
    });

    if already_present && global_enabled {
        debug!("Nielsen accessibility service is already enabled ({nielsen_component})");
        return Ok(false);
    }

    let mut new_services = current_services;
    if !new_services.iter().any(|s| s == nielsen_component) {
        new_services.push(nielsen_component.to_string());
    }

    let new_services_str = new_services.join(":");
    info!(
        "Enabling Nielsen accessibility service (setting enabled_accessibility_services='{new_services_str}')..."
    );
    device.run_shell(
        device_target,
        &format!("settings put secure enabled_accessibility_services \"{new_services_str}\""),
    )?;

    if !global_enabled {
        info!("Enabling global accessibility switch (accessibility_enabled=1)...");
        device.run_shell(device_target, "settings put secure accessibility_enabled 1")?;
    }

    verify_enabled_state(device, device_target, nielsen_component)
}

/// Searches dumpsys accessibility for Nielsen components.
fn detect_from_dumpsys(device: &impl DeviceCommander, device_target: &str) -> Option<String> {
    let dumpsys = device
        .run_shell(device_target, "dumpsys accessibility")
        .ok()?;
    let regex =
        Regex::new(r"(?i)\b([a-zA-Z0-9._]*(?:nlsn|nielsen)[a-zA-Z0-9._]*/[a-zA-Z0-9._]+)\b")
            .ok()?;
    if let Some(cap) = regex.captures_iter(&dumpsys).next() {
        let candidate = cap[1].to_string();
        info!("Found Nielsen accessibility component from dumpsys: {candidate}");
        return Some(candidate);
    }
    None
}

/// Searches `cmd accessibility get-installed-accessibility-services` for Nielsen components.
fn detect_from_cmd(device: &impl DeviceCommander, device_target: &str) -> Option<String> {
    let cmd_out = device
        .run_shell(
            device_target,
            "cmd accessibility get-installed-accessibility-services",
        )
        .ok()?;
    let regex =
        Regex::new(r"(?i)\b([a-zA-Z0-9._]*(?:nlsn|nielsen)[a-zA-Z0-9._]*/[a-zA-Z0-9._]+)\b")
            .ok()?;
    let cap = regex.captures(&cmd_out)?;
    let candidate = cap[1].to_string();
    info!("Found Nielsen accessibility component from cmd: {candidate}");
    Some(candidate)
}

/// Searches package manager and dumpsys package for Nielsen services.
fn detect_from_packages(device: &impl DeviceCommander, device_target: &str) -> Option<String> {
    let pm_out = device.run_shell(device_target, "pm list packages").ok()?;
    for line in pm_out.lines() {
        let pkg = line.trim().trim_start_matches("package:").trim();
        let lower = pkg.to_lowercase();
        if !lower.contains("nielsen") && !lower.contains("nlsn") {
            continue;
        }

        info!("Found Nielsen package: {pkg}");
        if let Ok(pkg_dump) = device.run_shell(device_target, &format!("dumpsys package {pkg}")) {
            let service_regex =
                Regex::new(&format!(r"(?i)\b({}/[a-zA-Z0-9._]+)\b", regex::escape(pkg))).ok()?;
            if let Some(cap) = service_regex.captures_iter(&pkg_dump).find(|cap| {
                let candidate = cap[1].to_lowercase();
                candidate.contains("service") || candidate.contains("accessibility")
            }) {
                let candidate = cap[1].to_string();
                info!("Found Nielsen accessibility service: {candidate}");
                return Some(candidate);
            }
        }
        return Some(format!("{pkg}/.AccessibilityService"));
    }
    None
}

/// Verifies that the accessibility service is properly enabled.
fn verify_enabled_state(
    device: &impl DeviceCommander,
    device_target: &str,
    nielsen_component: &str,
) -> Result<bool> {
    let verified_services = get_enabled_services(device, device_target)?;
    let verified_global = is_global_accessibility_enabled(device, device_target)?;

    let is_ok = verified_services.iter().any(|s| s == nielsen_component) && verified_global;
    if is_ok {
        info!("Successfully enabled Nielsen accessibility service on {device_target}!");
        Ok(true)
    } else {
        warn!("Verification showed services={verified_services:?}, global={verified_global}");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    struct FakeDevice {
        responses: RwLock<HashMap<String, String>>,
    }

    impl FakeDevice {
        fn new() -> Self {
            Self {
                responses: RwLock::new(HashMap::new()),
            }
        }

        fn set_response(&self, cmd: &str, resp: &str) {
            let mut map = self
                .responses
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.insert(cmd.to_string(), resp.to_string());
        }
    }

    impl DeviceCommander for FakeDevice {
        fn run_shell(&self, _target: &str, command: &str) -> Result<String> {
            let map = self
                .responses
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Ok(map.get(command).cloned().unwrap_or_default())
        }
    }

    #[test]
    fn test_get_enabled_services_empty() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response("settings get secure enabled_accessibility_services", "null");
        let services = get_enabled_services(&fake, "target")?;
        assert!(services.is_empty());
        Ok(())
    }

    #[test]
    fn test_get_enabled_services_parsed() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response(
            "settings get secure enabled_accessibility_services",
            "com.google/TalkBack:com.nlsn.tv/.AccService",
        );
        let services = get_enabled_services(&fake, "target")?;
        assert_eq!(services.len(), 2);
        assert_eq!(services[0], "com.google/TalkBack");
        assert_eq!(services[1], "com.nlsn.tv/.AccService");
        Ok(())
    }

    #[test]
    fn test_global_accessibility_enabled() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response("settings get secure accessibility_enabled", "1\n");
        assert!(is_global_accessibility_enabled(&fake, "target")?);

        fake.set_response("settings get secure accessibility_enabled", "0\n");
        assert!(!is_global_accessibility_enabled(&fake, "target")?);
        Ok(())
    }
}
