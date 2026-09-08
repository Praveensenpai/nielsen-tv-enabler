//! Domain logic for inspecting and keeping Nielsen Accessibility service enabled.

use crate::domain::DeviceCommander;
pub use crate::domain::detect::detect_nielsen_service;
use anyhow::Result;
use log::{debug, info, warn};

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

/// Checks whether a service component identifier matches the target Nielsen component.
#[must_use]
pub fn is_nielsen_match(service: &str, nielsen_component: &str) -> bool {
    if service == nielsen_component {
        return true;
    }
    let s_lower = service.to_lowercase();
    let n_lower = nielsen_component.to_lowercase();
    (s_lower.contains("nielsen") || s_lower.contains("nlsn"))
        && (n_lower.contains("nielsen") || n_lower.contains("nlsn"))
}

/// Checks if the Nielsen accessibility service is actively bound in Android's accessibility manager.
#[must_use]
pub fn is_service_bound(
    device: &impl DeviceCommander,
    device_target: &str,
    nielsen_component: &str,
) -> bool {
    let Ok(dumpsys) = device.run_shell(device_target, "dumpsys accessibility") else {
        return false;
    };
    parse_is_service_bound(&dumpsys, nielsen_component)
}

fn section_contains_component(dumpsys: &str, header: &str, nielsen_component: &str) -> bool {
    let Some(start_idx) = dumpsys.find(header) else {
        return false;
    };
    let after = &dumpsys[start_idx + header.len()..];
    let Some(end_idx) = after.find('}') else {
        return false;
    };
    let section = &after[..end_idx];
    let cleaned = section.trim_matches(|c: char| c == '{' || c == '}' || c.is_whitespace());
    if cleaned.is_empty() {
        return false;
    }

    if cleaned.contains(nielsen_component) {
        return true;
    }

    let pkg = nielsen_component
        .split_once('/')
        .map_or(nielsen_component, |(p, _)| p);
    if cleaned.contains(pkg) {
        return true;
    }

    let lower = cleaned.to_lowercase();
    lower.contains("nlsn") || lower.contains("nielsen") || lower.contains("confluence")
}

/// Parses `dumpsys accessibility` output to determine if the service is currently in `Bound services`.
#[must_use]
pub fn parse_is_service_bound(dumpsys: &str, nielsen_component: &str) -> bool {
    section_contains_component(dumpsys, "Bound services:{", nielsen_component)
}

/// Parses `dumpsys accessibility` output to determine if the service is currently in `Binding services`.
#[must_use]
pub fn parse_is_service_binding(dumpsys: &str, nielsen_component: &str) -> bool {
    section_contains_component(dumpsys, "Binding services:{", nielsen_component)
}

/// Forces an accessibility toggle cycle when listed in settings but not bound by Android system server.
fn force_rebind_toggle(
    device: &impl DeviceCommander,
    device_target: &str,
    other_services: &[String],
    nielsen_component: &str,
) -> Result<()> {
    info!("Nielsen service listed in settings but not bound. Forcing re-bind toggle...");
    let temp_str = other_services.join(":");
    device.run_shell(
        device_target,
        &format!("settings put secure enabled_accessibility_services \"{temp_str}\""),
    )?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let pkg = nielsen_component
        .split_once('/')
        .map_or(nielsen_component, |(p, _)| p);
    let _ = device.run_shell(
        device_target,
        &format!("am broadcast -a com.android.imi.HOURLY_INTENT -p {pkg}"),
    );
    Ok(())
}

/// Ensures that the Nielsen accessibility service is enabled and bound while preserving other services.
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
    let dumpsys = device
        .run_shell(device_target, "dumpsys accessibility")
        .unwrap_or_default();
    let service_bound = parse_is_service_bound(&dumpsys, nielsen_component);
    let service_binding = parse_is_service_binding(&dumpsys, nielsen_component);

    let already_present = current_services
        .iter()
        .any(|s| is_nielsen_match(s, nielsen_component));

    if already_present && global_enabled && service_bound {
        debug!("Nielsen accessibility service is already enabled and bound ({nielsen_component})");
        return Ok(false);
    }

    if already_present && global_enabled && service_binding {
        debug!(
            "Nielsen accessibility service is currently binding in system server ({nielsen_component})"
        );
        return Ok(false);
    }

    let other_services: Vec<String> = current_services
        .into_iter()
        .filter(|s| !is_nielsen_match(s, nielsen_component))
        .collect();

    if already_present && !service_bound && !service_binding {
        force_rebind_toggle(device, device_target, &other_services, nielsen_component)?;
    }

    let mut new_services = other_services;
    new_services.push(nielsen_component.to_string());
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

/// Verifies that the accessibility service is properly enabled and active.
fn verify_enabled_state(
    device: &impl DeviceCommander,
    device_target: &str,
    nielsen_component: &str,
) -> Result<bool> {
    let verified_services = get_enabled_services(device, device_target)?;
    let verified_global = is_global_accessibility_enabled(device, device_target)?;

    let is_ok = verified_services
        .iter()
        .any(|s| is_nielsen_match(s, nielsen_component))
        && verified_global;
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

    #[test]
    fn test_parse_is_service_bound_empty() {
        let dump = "User state[\n     Bound services:{}\n     Enabled services:{{com.nlsn/svc}}\n]";
        assert!(!parse_is_service_bound(dump, "com.nlsn/svc"));
    }

    #[test]
    fn test_parse_is_service_bound_confluence() {
        let dump = "User state[\n     Bound services:{Service[label=ConfluenceTV, feedbackType[FEEDBACK_GENERIC]]}\n]";
        assert!(parse_is_service_bound(
            dump,
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
        ));
    }

    #[test]
    fn test_parse_is_service_bound_exact_component() {
        let dump = "User state[\n     Bound services:{com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService}\n]";
        assert!(parse_is_service_bound(
            dump,
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
        ));
    }

    #[test]
    fn test_parse_is_service_bound_other_service() {
        let dump = "User state[\n     Bound services:{Service[label=TalkBack]}\n]";
        assert!(!parse_is_service_bound(
            dump,
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
        ));
    }

    #[test]
    fn test_is_nielsen_match() {
        assert!(is_nielsen_match(
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService",
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
        ));
        assert!(is_nielsen_match(
            "com.nlsn.confluencetv/nlsn.service",
            "com.nielsen.app/service"
        ));
        assert!(!is_nielsen_match(
            "com.google.android.marvin.talkback/.TalkBackService",
            "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
        ));
    }

    const COMP: &str = "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService";

    #[test]
    fn test_ensure_accessibility_rebinds_when_unbound() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response("settings get secure enabled_accessibility_services", COMP);
        fake.set_response("settings get secure accessibility_enabled", "1\n");
        fake.set_response("dumpsys accessibility", "Bound services:{}\n");
        assert!(ensure_accessibility_enabled(&fake, "target", COMP)?);
        Ok(())
    }

    #[test]
    fn test_parse_is_service_binding() {
        let dump = "Bound services:{}\n     Binding services:{{com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService}}";
        assert!(parse_is_service_binding(dump, COMP));
        assert!(!parse_is_service_bound(dump, COMP));
    }

    #[test]
    fn test_ensure_accessibility_waits_when_binding() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response("settings get secure enabled_accessibility_services", COMP);
        fake.set_response("settings get secure accessibility_enabled", "1\n");
        let dump = "Bound services:{}\n Binding services:{{com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService}}";
        fake.set_response("dumpsys accessibility", dump);
        assert!(!ensure_accessibility_enabled(&fake, "target", COMP)?);
        Ok(())
    }

    #[test]
    fn test_ensure_accessibility_skips_when_bound() -> Result<()> {
        let fake = FakeDevice::new();
        fake.set_response("settings get secure enabled_accessibility_services", COMP);
        fake.set_response("settings get secure accessibility_enabled", "1\n");
        fake.set_response(
            "dumpsys accessibility",
            "Bound services:{Service[label=ConfluenceTV]}",
        );
        assert!(!ensure_accessibility_enabled(&fake, "target", COMP)?);
        Ok(())
    }
}
