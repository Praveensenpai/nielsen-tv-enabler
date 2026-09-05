//! Domain logic for granting VPN appops permissions and handling VPN connection dialogs.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::{debug, info};
use regex::Regex;

/// Default Android TV package name for Nielsen panel app.
pub const DEFAULT_NIELSEN_PACKAGE: &str = "com.nlsn.confluencetv";

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
}
