//! Device communication port trait for domain logic.

use anyhow::Result;

/// Port trait defining device shell command execution required by domain services.
pub trait DeviceCommander: Send + Sync {
    /// Executes a shell command on the target device and returns standard output.
    ///
    /// # Errors
    /// Returns an error if device communication or the command fails.
    fn run_shell(&self, device_target: &str, command: &str) -> Result<String>;
}
