//! Domain logic for granting VPN appops permissions and handling VPN connection dialogs.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::{debug, info};
use regex::Regex;

/// Default Android TV package name for Nielsen panel app.
pub const DEFAULT_NIELSEN_PACKAGE: &str = "com.nlsn.confluencetv";

/// Checks whether a VPN tunnel network interface (e.g. `tun0`) is currently up on the device.
///
/// # Errors
/// Returns an error if querying device network interfaces fails.
pub fn is_vpn_tunnel_active(device: &impl DeviceCommander, device_target: &str) -> Result<bool> {
    let output = device.run_shell(device_target, "ip -o link show")?;
    let active = output.lines().any(|line| {
        let lower = line.to_lowercase();
        lower.contains("tun")
            && (lower.contains("<up") || lower.contains(",up") || lower.contains("state up"))
    });
    Ok(active)
}

/// Checks if always-on VPN is configured for the given package in Android secure settings.
///
/// # Errors
/// Returns an error if querying device settings fails.
pub fn is_always_on_vpn_configured(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<bool> {
    let output = device.run_shell(device_target, "settings get secure always_on_vpn_app")?;
    Ok(output.trim() == package_name)
}

/// Checks if VPN is already active or configured for the specified package.
///
/// # Errors
/// Returns an error if device communication fails.
pub fn is_vpn_active(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<bool> {
    if is_vpn_tunnel_active(device, device_target).unwrap_or(false) {
        return Ok(true);
    }
    is_always_on_vpn_configured(device, device_target, package_name)
}

/// Ensures that VPN is enabled for the target package if currently disabled.
/// If VPN is already active or always-on is configured, does not touch or alter the VPN.
///
/// Returns `Ok(true)` if VPN configuration was applied, or `Ok(false)` if already active.
///
/// # Errors
/// Returns an error if executing device shell commands fails.
pub fn ensure_vpn_enabled(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<bool> {
    if is_vpn_active(device, device_target, package_name)? {
        debug!("VPN is already active or configured for {package_name} on {device_target}");
        let _ = handle_vpn_dialog(device, device_target);
        return Ok(false);
    }

    info!("VPN is disabled or unconfigured for {package_name} on {device_target}. Enabling...");
    grant_all_background_permissions(device, device_target, package_name)?;
    let _ = handle_vpn_dialog(device, device_target);
    Ok(true)
}

/// Grants all required background permissions (VPN, usage stats, alert window, notification listener)
/// and configures always-on VPN purely via ADB without touching or opening the app UI.
///
/// # Errors
/// Returns an error if device communication fails.
pub fn grant_all_background_permissions(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<()> {
    info!(
        "Ensuring all background ADB permissions and always-on VPN for {package_name} on {device_target}..."
    );

    // 1. Grant VPN appops
    let _ = device.run_shell(
        device_target,
        &format!("cmd appops set {package_name} ACTIVATE_VPN allow"),
    );

    // 2. Set Always-On VPN to start and maintain tunnel automatically in background
    let _ = device.run_shell(
        device_target,
        &format!("settings put secure always_on_vpn_app {package_name}"),
    );

    // 3. Grant App Usage Stats permission
    let _ = device.run_shell(
        device_target,
        &format!("cmd appops set {package_name} GET_USAGE_STATS allow"),
    );

    // 4. Grant Display Over Other Apps permission
    let _ = device.run_shell(
        device_target,
        &format!("cmd appops set {package_name} SYSTEM_ALERT_WINDOW allow"),
    );

    // 5. Grant Notification Listener access
    let notification_component = format!("{package_name}/nielsen.confluence.nlsdk.Clsdk");
    let _ = device.run_shell(
        device_target,
        &format!("cmd notification allow_listener {notification_component}"),
    );
    let _ = device.run_shell(
        device_target,
        &format!("settings put secure enabled_notification_listeners {notification_component}"),
    );

    info!(
        "[SUCCESS] All ADB background permissions and always-on VPN configured for {package_name}!"
    );
    Ok(())
}

/// Grants the `ACTIVATE_VPN` app-op permission to the target package via ADB.
///
/// # Errors
/// Returns an error if executing the ADB shell command fails.
pub fn grant_vpn_appops(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<()> {
    debug!("Granting ACTIVATE_VPN appop to {package_name} on {device_target}...");
    device.run_shell(
        device_target,
        &format!("cmd appops set {package_name} ACTIVATE_VPN allow"),
    )?;
    Ok(())
}

/// Checks if an Android VPN confirmation dialog (`com.android.vpndialogs`) is active.
/// If present, extracts the confirmation button coordinates, clicks OK, and confirms.
///
/// # Errors
/// Returns an error if device shell commands fail.
pub fn handle_vpn_dialog(device: &impl DeviceCommander, device_target: &str) -> Result<bool> {
    let Ok(focus) = device.run_shell(
        device_target,
        "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
    ) else {
        return Ok(false);
    };

    if !focus.contains("com.android.vpndialogs") && !focus.contains("VpnDialog") {
        return Ok(false);
    }

    info!("[VPN PROMPT DETECTED] VPN connection request dialog is active on {device_target}!");
    let dump_cmd = "uiautomator dump /data/local/tmp/uidump.xml >/dev/null 2>&1 && cat /data/local/tmp/uidump.xml";
    let Ok(ui_dump) = device.run_shell(device_target, dump_cmd) else {
        return send_dpad_confirm(device, device_target);
    };

    if let Some((x, y)) = extract_vpn_ok_button(&ui_dump) {
        info!("Tapping VPN confirmation button at ({x}, {y})...");
        device.run_shell(device_target, &format!("input tap {x} {y}"))?;
        info!("[SUCCESS] Confirmed VPN connection request via UI tap!");
        return Ok(true);
    }

    send_dpad_confirm(device, device_target)
}

/// Extracts center coordinates of the confirmation button in `com.android.vpndialogs`.
#[must_use]
pub fn extract_vpn_ok_button(ui_dump: &str) -> Option<(u32, u32)> {
    let button_re = Regex::new(
        r#"<node\b[^>]*(?:resource-id="android:id/button1"|text="(?i)(?:ok|allow)")[^>]*>"#,
    )
    .ok()?;
    let bounds_re = Regex::new(r#"bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]""#).ok()?;

    let button_match = button_re.find(ui_dump)?;
    let bounds = bounds_re.captures(button_match.as_str())?;

    let x1: u32 = bounds[1].parse().unwrap_or(0);
    let y1: u32 = bounds[2].parse().unwrap_or(0);
    let x2: u32 = bounds[3].parse().unwrap_or(0);
    let y2: u32 = bounds[4].parse().unwrap_or(0);

    Some((x1.midpoint(x2), y1.midpoint(y2)))
}

/// Fallback for Android TV interfaces by sending D-pad right and enter key events.
fn send_dpad_confirm(device: &impl DeviceCommander, device_target: &str) -> Result<bool> {
    info!("Confirming VPN dialog via Android TV D-Pad navigation...");
    device.run_shell(device_target, "input keyevent 22")?; // DPAD_RIGHT to highlight OK
    device.run_shell(device_target, "input keyevent 66")?; // KEYCODE_ENTER
    device.run_shell(device_target, "input keyevent 23")?; // KEYCODE_DPAD_CENTER
    info!("[SUCCESS] Dispatched D-Pad confirmation keys for VPN dialog!");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_vpn_ok_button_by_resource_id() {
        let xml = r#"<node class="android.widget.FrameLayout" package="com.android.vpndialogs">
            <node text="Connection request" resource-id="android:id/alertTitle" bounds="[100,100][500,200]" />
            <node text="Cancel" resource-id="android:id/button2" bounds="[200,600][350,700]" />
            <node text="OK" resource-id="android:id/button1" bounds="[400,600][550,700]" />
        </node>"#;

        let coords = extract_vpn_ok_button(xml);
        assert_eq!(coords, Some((475, 650)));
    }

    #[test]
    fn test_extract_vpn_ok_button_by_text() {
        let xml = r#"<node class="android.widget.FrameLayout" package="com.android.vpndialogs">
            <node text="Allow" bounds="[300,500][500,600]" />
        </node>"#;

        let coords = extract_vpn_ok_button(xml);
        assert_eq!(coords, Some((400, 550)));
    }

    #[test]
    fn test_extract_vpn_ok_button_missing() {
        let xml = r#"<node text="Cancel" resource-id="android:id/button2" bounds="[100,100][200,200]" />"#;
        assert_eq!(extract_vpn_ok_button(xml), None);
    }

    #[derive(Default)]
    struct FakeDevice {
        responses: std::sync::RwLock<std::collections::HashMap<String, String>>,
        executed: std::sync::RwLock<Vec<String>>,
    }

    impl FakeDevice {
        fn set(&self, cmd: &str, resp: &str) {
            self.responses
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(cmd.to_string(), resp.to_string());
        }

        fn ran(&self, pattern: &str) -> bool {
            self.executed
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .any(|c| c.contains(pattern))
        }
    }

    impl DeviceCommander for FakeDevice {
        fn run_shell(&self, _target: &str, command: &str) -> Result<String> {
            self.executed
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(command.to_string());
            let map = self
                .responses
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Ok(map.get(command).cloned().unwrap_or_default())
        }
    }

    #[test]
    fn test_is_vpn_tunnel_active() -> Result<()> {
        let fake = FakeDevice::default();
        fake.set(
            "ip -o link show",
            "1: lo: <LOOPBACK,UP,LOWER_UP>\n2: tun0: <POINTOPOINT,UP,LOWER_UP>\n",
        );
        assert!(is_vpn_tunnel_active(&fake, "target")?);

        fake.set("ip -o link show", "1: lo: <LOOPBACK,UP,LOWER_UP>\n");
        assert!(!is_vpn_tunnel_active(&fake, "target")?);
        Ok(())
    }

    #[test]
    fn test_is_always_on_vpn_configured() -> Result<()> {
        let fake = FakeDevice::default();
        fake.set(
            "settings get secure always_on_vpn_app",
            "com.nlsn.confluencetv\n",
        );
        assert!(is_always_on_vpn_configured(
            &fake,
            "target",
            "com.nlsn.confluencetv"
        )?);

        fake.set("settings get secure always_on_vpn_app", "null\n");
        assert!(!is_always_on_vpn_configured(
            &fake,
            "target",
            "com.nlsn.confluencetv"
        )?);
        Ok(())
    }

    #[test]
    fn test_ensure_vpn_enabled_leaves_active_alone() -> Result<()> {
        let fake = FakeDevice::default();
        fake.set(
            "settings get secure always_on_vpn_app",
            "com.nlsn.confluencetv\n",
        );
        fake.set(
            "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
            "mCurrentFocus=null",
        );

        let modified = ensure_vpn_enabled(&fake, "target", "com.nlsn.confluencetv")?;
        assert!(!modified);
        assert!(!fake.ran("settings put"));
        assert!(!fake.ran("ACTIVATE_VPN allow"));
        Ok(())
    }

    #[test]
    fn test_ensure_vpn_enabled_activates_when_disabled() -> Result<()> {
        let fake = FakeDevice::default();
        fake.set("settings get secure always_on_vpn_app", "null\n");
        fake.set("ip -o link show", "1: lo: <LOOPBACK,UP,LOWER_UP>\n");
        fake.set(
            "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
            "mCurrentFocus=null",
        );

        let modified = ensure_vpn_enabled(&fake, "target", "com.nlsn.confluencetv")?;
        assert!(modified);
        assert!(fake.ran("settings put secure always_on_vpn_app com.nlsn.confluencetv"));
        assert!(fake.ran("cmd appops set com.nlsn.confluencetv ACTIVATE_VPN allow"));
        Ok(())
    }
}
