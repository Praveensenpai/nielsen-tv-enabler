//! Handles detection and automatic response to the 'Who is watching?' survey prompt.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::{debug, info};
use regex::Regex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Checks if the "Who is watching?" dialog is currently on screen.
/// If present, randomly selects a member, taps their checkbox, clicks OK, and restores previous view.
///
/// # Errors
/// Returns an error if device shell commands fail.
pub fn handle_who_is_watching(device: &impl DeviceCommander, device_target: &str) -> Result<bool> {
    let Ok(focus) = device.run_shell(
        device_target,
        "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
    ) else {
        return Ok(false);
    };
    if !focus.contains("com.nlsn.confluencetv") {
        return Ok(false);
    }

    let dump_cmd = "uiautomator dump /data/local/tmp/uidump.xml >/dev/null 2>&1 && cat /data/local/tmp/uidump.xml";
    let Ok(ui_dump) = device.run_shell(device_target, dump_cmd) else {
        return Ok(false);
    };
    if !ui_dump.contains("Who is watching?") && !ui_dump.contains("buttonOk") {
        return Ok(false);
    }

    let options = extract_member_checkboxes(&ui_dump);
    if options.is_empty() {
        return Ok(false);
    }

    let (selected_name, sel_x, sel_y) = pick_random_member(&options);
    info!("[PROMPT DETECTED] Randomly selecting '{selected_name}' at ({sel_x}, {sel_y})...");

    device.run_shell(device_target, &format!("input tap {sel_x} {sel_y}"))?;
    std::thread::sleep(Duration::from_millis(300));

    let (ok_x, ok_y) = extract_ok_button(&ui_dump);
    info!("Clicking OK button at ({ok_x}, {ok_y})...");
    device.run_shell(device_target, &format!("input tap {ok_x} {ok_y}"))?;
    std::thread::sleep(Duration::from_millis(500));

    dismiss_overlay_if_active(device, device_target);
    info!("[SUCCESS] Answered 'Who is watching?' with '{selected_name}'!");
    Ok(true)
}

/// Parses member checkboxes and calculates center coordinates for tapping.
fn extract_member_checkboxes(ui_dump: &str) -> Vec<(String, u32, u32)> {
    let Ok(node_re) =
        Regex::new(r#"<node\b[^>]*\bresource-id="com\.nlsn\.confluencetv:id/checkbox"[^>]*>"#)
    else {
        return Vec::new();
    };
    let Ok(text_re) = Regex::new(r#"text="([^"]*)""#) else {
        return Vec::new();
    };
    let Ok(bounds_re) = Regex::new(r#"bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]""#) else {
        return Vec::new();
    };

    let mut options = Vec::new();
    for node_match in node_re.find_iter(ui_dump) {
        let node_str = node_match.as_str();
        let name = text_re
            .captures(node_str)
            .map_or_else(|| "Unknown".to_string(), |c| c[1].to_string());

        if let Some(bounds) = bounds_re.captures(node_str) {
            let x1: u32 = bounds[1].parse().unwrap_or(0);
            let y1: u32 = bounds[2].parse().unwrap_or(0);
            let x2: u32 = bounds[3].parse().unwrap_or(0);
            let y2: u32 = bounds[4].parse().unwrap_or(0);
            options.push((name, x1.midpoint(x2), y1.midpoint(y2)));
        }
    }
    options
}

/// Selects one member at random using system time.
fn pick_random_member(options: &[(String, u32, u32)]) -> (String, u32, u32) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mod_len = u128::try_from(options.len()).unwrap_or(1);
    let rand_idx = usize::try_from(now.as_nanos() % mod_len).unwrap_or(0);
    options[rand_idx].clone()
}

/// Finds center coordinates of the OK confirmation button.
fn extract_ok_button(ui_dump: &str) -> (u32, u32) {
    let Ok(ok_node_re) =
        Regex::new(r#"<node\b[^>]*\bresource-id="com\.nlsn\.confluencetv:id/buttonOk"[^>]*>"#)
    else {
        return (233, 471);
    };
    let Ok(bounds_re) = Regex::new(r#"bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]""#) else {
        return (233, 471);
    };

    if let Some(ok_match) = ok_node_re.find(ui_dump)
        && let Some(bounds) = bounds_re.captures(ok_match.as_str())
    {
        let x1: u32 = bounds[1].parse().unwrap_or(0);
        let y1: u32 = bounds[2].parse().unwrap_or(0);
        let x2: u32 = bounds[3].parse().unwrap_or(0);
        let y2: u32 = bounds[4].parse().unwrap_or(0);
        return (x1.midpoint(x2), y1.midpoint(y2));
    }
    (233, 471)
}

/// Closes the Dashboard overlay if it remained open, restoring background app.
fn dismiss_overlay_if_active(device: &impl DeviceCommander, device_target: &str) {
    if let Ok(after_focus) =
        device.run_shell(device_target, "dumpsys window | grep -E 'mCurrentFocus'")
        && after_focus.contains("com.nlsn.confluencetv")
    {
        debug!("Dismissing Dashboard overlay via BACK key...");
        let _ = device.run_shell(device_target, "input keyevent 4");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_member_checkboxes() {
        let xml = r#"<node index="0" text="" resource-id="" class="android.widget.FrameLayout" package="com.nlsn.confluencetv">
            <node index="1" text="Alice" resource-id="com.nlsn.confluencetv:id/checkbox" bounds="[100,200][200,300]" />
            <node index="2" text="Bob" resource-id="com.nlsn.confluencetv:id/checkbox" bounds="[100,320][200,420]" />
        </node>"#;

        let options = extract_member_checkboxes(xml);
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].0, "Alice");
        assert_eq!(options[0].1, 150);
        assert_eq!(options[0].2, 250);
        assert_eq!(options[1].0, "Bob");
        assert_eq!(options[1].1, 150);
        assert_eq!(options[1].2, 370);
    }

    #[test]
    fn test_extract_ok_button() {
        let xml = r#"<node text="OK" resource-id="com.nlsn.confluencetv:id/buttonOk" bounds="[200,400][300,500]" />"#;
        let (x, y) = extract_ok_button(xml);
        assert_eq!(x, 250);
        assert_eq!(y, 450);
    }
}
