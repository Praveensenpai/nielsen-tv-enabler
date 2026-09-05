//! Domain logic for triggering background data sync via ADB broadcasts and jobs.

use crate::domain::DeviceCommander;
use anyhow::Result;
use log::{debug, info};
use std::time::{Duration, Instant};

/// Sync broadcast intents sent to Nielsen `ConfluenceTV`.
pub const SYNC_BROADCAST_ACTIONS: &[&str] = &[
    "com.android.imi.HOURLY_INTENT",
    "com.android.nielsen.POSTING_INTENT",
    "com.android.imi.POSTING_INTENT",
    "com.android.nlsdk.COLLECTION",
];

/// Fallback `JobScheduler` Job ID for Nielsen `WorkManager` tasks.
pub const DEFAULT_NIELSEN_JOB_ID: u32 = 1101;

/// Triggers background data synchronization entirely via ADB without opening the app UI.
///
/// # Errors
/// Returns an error if device communication fails.
pub fn trigger_background_sync(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Result<()> {
    info!("Triggering background data sync for {package_name} on {device_target}...");

    // 1. Trigger JobScheduler job (e.g. WorkManager sync worker)
    let job_id =
        find_nielsen_job_id(device, device_target, package_name).unwrap_or(DEFAULT_NIELSEN_JOB_ID);
    debug!("Executing JobScheduler job {job_id} for {package_name}...");
    let _ = device.run_shell(
        device_target,
        &format!("cmd jobscheduler run -f {package_name} {job_id}"),
    );

    // 2. Dispatch background broadcast intents
    for action in SYNC_BROADCAST_ACTIONS {
        debug!("Broadcasting {action} to {package_name}...");
        let _ = device.run_shell(
            device_target,
            &format!("am broadcast -a {action} -p {package_name}"),
        );
    }

    info!("[SUCCESS] Background data sync commands dispatched to {package_name}!");
    Ok(())
}

/// Discovers the active Nielsen `WorkManager` Job ID from `JobScheduler` dump.
#[must_use]
pub fn find_nielsen_job_id(
    device: &impl DeviceCommander,
    device_target: &str,
    package_name: &str,
) -> Option<u32> {
    let output = device
        .run_shell(device_target, "dumpsys jobscheduler")
        .ok()?;
    parse_job_id_from_dump(&output, package_name)
}

/// Parses the first matching job ID for the given package from `dumpsys jobscheduler` output.
#[must_use]
pub fn parse_job_id_from_dump(dump: &str, package_name: &str) -> Option<u32> {
    for line in dump.lines() {
        if line.contains(package_name) && line.contains("JOB #") {
            // Format: JOB #u0a106/1101: ... or JOB #1000/123: ...
            if let Some(slash_idx) = line.find('/') {
                let after_slash = &line[slash_idx + 1..];
                let num_str: String = after_slash
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if let Ok(id) = num_str.parse::<u32>() {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// Calculates an organic daily interval (86,400s ± jitter) to mimic natural human TV viewing variance.
fn calculate_daily_interval(jitter_mins: u32) -> Duration {
    let base_secs: u64 = 86_400;
    let jitter_secs = u64::from(jitter_mins) * 60;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let jitter_offset = if jitter_secs > 0 {
        let spread = jitter_secs * 2;
        u64::try_from(nanos % u128::from(spread)).unwrap_or(0)
    } else {
        0
    };
    let actual_secs = base_secs
        .saturating_sub(jitter_secs)
        .saturating_add(jitter_offset);
    Duration::from_secs(actual_secs)
}

/// Tracks once-a-day synchronization state and delays.
#[derive(Debug, Clone)]
pub struct DailySyncTracker {
    last_sync: Option<Instant>,
    daily_interval: Duration,
    post_enable_delay: Duration,
    jitter_mins: u32,
}

impl DailySyncTracker {
    /// Creates a new tracker with organic 24-hour interval jitter and custom post-enable delay.
    #[must_use]
    pub fn new(post_enable_delay_secs: u64, jitter_mins: u32) -> Self {
        Self {
            last_sync: None,
            daily_interval: calculate_daily_interval(jitter_mins),
            post_enable_delay: Duration::from_secs(post_enable_delay_secs),
            jitter_mins,
        }
    }

    /// Checks if a daily sync is due.
    #[must_use]
    pub fn is_sync_due(&self) -> bool {
        match self.last_sync {
            None => true,
            Some(last) => last.elapsed() >= self.daily_interval,
        }
    }

    /// Records that a sync was completed and recalibrates the next daily cycle interval.
    pub fn mark_synced(&mut self) {
        self.last_sync = Some(Instant::now());
        self.daily_interval = calculate_daily_interval(self.jitter_mins);
    }

    /// Returns the required delay duration after turning on permissions before syncing.
    #[must_use]
    pub fn post_enable_delay(&self) -> Duration {
        self.post_enable_delay
    }

    /// Returns the active daily interval duration.
    #[must_use]
    pub fn daily_interval(&self) -> Duration {
        self.daily_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_id_from_dump() {
        let dump = "  JOB #u0a106/1101: 68aa23 com.nlsn.confluencetv/androidx.work.impl.background.systemjob.SystemJobService\n";
        assert_eq!(
            parse_job_id_from_dump(dump, "com.nlsn.confluencetv"),
            Some(1101)
        );
    }

    #[test]
    fn test_parse_job_id_missing() {
        let dump = "  JOB #u0a26/1: com.android.statementservice/...\n";
        assert_eq!(parse_job_id_from_dump(dump, "com.nlsn.confluencetv"), None);
    }

    #[test]
    fn test_daily_sync_tracker() {
        let mut tracker = DailySyncTracker::new(10, 30);
        assert!(tracker.is_sync_due());
        assert_eq!(tracker.post_enable_delay(), Duration::from_secs(10));
        assert!(tracker.daily_interval() >= Duration::from_mins(1410));
        assert!(tracker.daily_interval() <= Duration::from_mins(1470));
        tracker.mark_synced();
        assert!(!tracker.is_sync_due());
    }
}
