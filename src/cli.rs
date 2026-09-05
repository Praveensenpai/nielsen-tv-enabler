//! Command-line argument parsing definitions.

use clap::Parser;
use std::path::PathBuf;

/// Command line arguments for nielsen-tv-enabler.
// reason: CLI flags for clap parser represent distinct operational commands and modes.
#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug)]
#[command(
    name = "nielsen-tv-enabler",
    author = "paisen",
    version = "0.1.2",
    about = "Keeps Nielsen Accessibility Service enabled on Android TV via ADB"
)]
pub struct Args {
    /// Run as a continuous background daemon (default if no other mode is selected)
    #[arg(long, short = 'd')]
    pub daemon: bool,

    /// Run once and exit
    #[arg(long)]
    pub once: bool,

    /// Scan local network for Android TV / ADB devices and exit
    #[arg(long)]
    pub scan: bool,

    /// Detect Nielsen accessibility service on target device and exit
    #[arg(long)]
    pub detect: bool,

    /// Check and answer 'Who is watching?' dialog immediately (selects member + clicks OK)
    #[arg(long, alias = "answer-prompt")]
    pub dismiss_prompt: bool,

    /// Grant VPN permission and approve any active VPN connection request dialog immediately
    #[arg(long, alias = "allow-vpn")]
    pub vpn: bool,

    /// Trigger background data sync via ADB immediately without opening the app UI
    #[arg(long, alias = "sync-now")]
    pub sync: bool,

    /// Install and enable systemd user service
    #[arg(long)]
    pub install_service: bool,

    /// Uninstall systemd user service
    #[arg(long)]
    pub uninstall_service: bool,

    /// Check status of systemd user service
    #[arg(long)]
    pub status_service: bool,

    /// Target Android TV IP address (overrides config)
    #[arg(long, short = 'i')]
    pub ip: Option<String>,

    /// Target ADB port (default: 5555)
    #[arg(long, short = 'p')]
    pub port: Option<u16>,

    /// Nielsen accessibility service component (overrides config)
    #[arg(long, short = 's')]
    pub service: Option<String>,

    /// Interval in seconds between checks in daemon mode
    #[arg(long, short = 't')]
    pub interval: Option<u64>,

    /// Path to config file
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Enable verbose / debug logging
    #[arg(long, short = 'v')]
    pub verbose: bool,
}
