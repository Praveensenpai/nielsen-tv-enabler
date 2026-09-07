//! Auto-detection logic for installed Nielsen accessibility services.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::info;
use regex::Regex;

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
