# CODEBASE.md: Nielsen TV Enabler Semantic Digest

> **Notice**: This file is an AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
main.rs ──> cli.rs ──> app.rs / app/daemon.rs ──> domain/{service, prompt, vpn, sync, detect} ──> infra/{adb, scanner, systemd}
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language & Edition**: Rust 2024 edition (`Cargo.toml`).
- **Architectural Paradigm**: Role-based architecture (`domain/`, `infra/`, `app/`, `cli.rs`, `config.rs`).
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production `unwrap()`/`expect()`, 0 compiler/clippy warnings.
- **Target Distribution**: Linux `x86_64` and `aarch64` standalone binaries via GitHub Releases.

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: entrypoint, Lines: 19)
- **Responsibility**: Binary entrypoint; initializes logger and executes CLI dispatcher.
- **Imports**: `clap::Parser`, `nielsen_tv_enabler::{app, cli::Cli}`
- **Public Functions & Signatures**:
  ```rust
  fn main() -> anyhow::Result<()>
  ```
- **Consumers**: OS process launch.
- **Side Effects / I/O**: Process exit code, stderr logging.

### `src/lib.rs` (Role: root, Lines: 7)
- **Responsibility**: Root library exports declaring public modules.
- **Imports**: `app`, `cli`, `config`, `domain`, `infra`

### `src/cli.rs` (Role: cli, Lines: 79)
- **Responsibility**: Command-line argument definitions and flag parsing using `clap`.
- **Imports**: `clap::{Parser, Subcommand}`
- **Types & Enums**:
  ```rust
  pub struct Cli { pub daemon: bool, pub status: bool, pub once: bool, pub dismiss_prompt: bool, ... }
  pub enum Commands { InstallService, UninstallService, ServiceStatus, ViewConfig }
  ```
- **Consumers**: `src/main.rs`, `src/app.rs`

### `src/config.rs` (Role: config, Lines: 164)
- **Responsibility**: TOML configuration parsing, loading, defaults, and disk persistence.
- **Imports**: `serde::{Deserialize, Serialize}`, `toml`, `dirs`
- **Types & Enums**:
  ```rust
  pub struct Config { pub tv_ip: String, pub adb_port: u16, pub last_known_ip: Option<String>, pub service_component: String, pub check_interval_secs: u64, pub auto_handle_who_is_watching: bool, ... }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl Config { pub fn load_or_create(custom_path: Option<&Path>) -> Result<(Self, PathBuf)>; pub fn save(&self, path: &Path) -> Result<()>; }
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: Filesystem read/write to `~/.config/nielsen-tv-enabler/config.toml`.

### `src/app.rs` (Role: app, Lines: 230)
- **Responsibility**: High-level command dispatcher for CLI subcommands, one-off runs, and status checks.
- **Imports**: `crate::cli::Cli`, `crate::config::Config`, `crate::domain::{prompt, service, sync, vpn}`, `crate::infra::adb::AdbClient`, `crate::infra::systemd`
- **Public Functions & Signatures**:
  ```rust
  pub fn run(args: Cli) -> Result<()>
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Spawns ADB commands, runs systemd unit commands.

### `src/app/daemon.rs` (Role: app, Lines: 197)
- **Responsibility**: Continuous background daemon loop, TV reachability monitoring, auto-reconnect, and cycle execution.
- **Imports**: `crate::config::Config`, `crate::domain::sync::DailySyncTracker`, `crate::domain::{prompt, service, sync, vpn}`, `crate::infra::adb::AdbClient`, `crate::infra::scanner::Scanner`
- **Public Functions & Signatures**:
  ```rust
  pub fn run_daemon(adb: &AdbClient, cfg: Config, config_path: &Path) -> Result<()>
  pub fn resolve_tv_ip(adb: &AdbClient, cfg: &mut Config, config_path: &Path) -> Result<String>
  ```
- **Consumers**: `src/app.rs`
- **Side Effects / I/O**: Periodic network polling, thread sleep, ADB commands.

### `src/domain/device.rs` (Role: domain, Lines: 12)
- **Responsibility**: Core interface abstraction trait for ADB shell execution.
- **Types & Enums**:
  ```rust
  pub trait DeviceCommander { fn run_shell(&self, target: &str, command: &str) -> Result<String>; }
  ```
- **Consumers**: `domain::prompt`, `domain::service`, `domain::vpn`, `domain::detect`

### `src/domain/prompt.rs` (Role: domain, Lines: 180)
- **Responsibility**: Detects and answers the "Who is watching?" survey prompt with humanized delay and random member selection.
- **Imports**: `crate::domain::DeviceCommander`, `regex::Regex`, `std::time::{Duration, SystemTime, UNIX_EPOCH}`
- **Public Functions & Signatures**:
  ```rust
  pub fn handle_who_is_watching(device: &impl DeviceCommander, device_target: &str) -> Result<bool>
  pub fn humanized_reaction_delay() -> Duration // 500ms..=1200ms
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: Device UI automator dump, ADB tap inputs.

### `src/domain/service.rs` (Role: domain, Lines: 381)
- **Responsibility**: Queries and activates the Nielsen Accessibility Service, detecting bound/binding state and atomic toggling.
- **Imports**: `crate::domain::DeviceCommander`, `crate::domain::detect`
- **Public Functions & Signatures**:
  ```rust
  pub fn ensure_accessibility_enabled(device: &impl DeviceCommander, device_target: &str, nielsen_component: &str) -> Result<bool>
  pub fn is_accessibility_globally_enabled(device: &impl DeviceCommander, device_target: &str) -> Result<bool>
  pub fn get_enabled_accessibility_services(device: &impl DeviceCommander, device_target: &str) -> Result<Vec<String>>
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: ADB settings commands, dumpsys queries.

### `src/domain/detect.rs` (Role: domain, Lines: 90)
- **Responsibility**: Scans installed packages and accessibility services to auto-detect the Nielsen component string.
- **Imports**: `crate::domain::DeviceCommander`, `regex::Regex`
- **Public Functions & Signatures**:
  ```rust
  pub fn detect_nielsen_service(device: &impl DeviceCommander, device_target: &str) -> Result<Option<String>>
  ```
- **Consumers**: `src/domain/service.rs`, `src/app/daemon.rs`

### `src/domain/vpn.rs` (Role: domain, Lines: 352)
- **Responsibility**: Configures always-on VPN in Android secure settings and dismisses VPN connection request prompts.
- **Imports**: `crate::domain::DeviceCommander`, `regex::Regex`
- **Public Functions & Signatures**:
  ```rust
  pub fn ensure_vpn_enabled(device: &impl DeviceCommander, device_target: &str, package_name: &str) -> Result<bool>
  pub fn handle_vpn_dialog_if_present(device: &impl DeviceCommander, device_target: &str) -> Result<bool>
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`

### `src/domain/sync.rs` (Role: domain, Lines: 184)
- **Responsibility**: Daily sync tracking with organic random jitter variance and wake-driven sync scheduling.
- **Imports**: `crate::domain::DeviceCommander`, `std::time::{Duration, Instant, SystemTime, UNIX_EPOCH}`
- **Public Functions & Signatures**:
  ```rust
  pub struct DailySyncTracker { ... }
  pub fn trigger_background_sync(device: &impl DeviceCommander, device_target: &str, package_name: &str) -> Result<()>
  ```
- **Consumers**: `src/app/daemon.rs`, `src/app.rs`

### `src/infra/adb.rs` (Role: infra, Lines: 184)
- **Responsibility**: Wraps `adb` CLI binary commands and implements `DeviceCommander`.
- **Types & Enums**:
  ```rust
  pub struct AdbClient { pub adb_path: String }
  pub enum DeviceStatus { Ready, Unauthorized, Offline, Missing }
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: Subprocess execution of `adb`.

### `src/infra/scanner.rs` (Role: infra, Lines: 168)
- **Responsibility**: Fast parallel TCP port probing across local subnets to find ADB targets.
- **Public Functions & Signatures**:
  ```rust
  pub struct Scanner;
  impl Scanner { pub fn scan_subnet_for_adb(...) -> Result<Vec<String>>; pub fn probe_tcp_port(...) -> bool; }
  ```
- **Consumers**: `src/app/daemon.rs`

### `src/infra/systemd.rs` (Role: infra, Lines: 161)
- **Responsibility**: Creates, enables, uninstalls, and checks user-level systemd daemon service files.
- **Consumers**: `src/app.rs`
- **Side Effects / I/O**: Creates `~/.config/systemd/user/nielsen-tv-enabler.service`, runs `systemctl`.

## 4. Execution Lifecycle Trace
1. **Startup**: Entrypoint `main.rs` initializes env_logger and parses `cli::Cli`.
2. **Dispatch**: `app::run()` matches CLI flags (`--daemon`, `--status`, `--once`, `--dismiss-prompt`, or subcommands).
3. **Daemon Loop**: `app::daemon::run_daemon()` resolves TV IP, establishes ADB connection, and checks readiness.
4. **Active Cycle**: Checks accessibility service state via `dumpsys`, re-binds if unbound, grants VPN, dismisses "Who is watching?" with 500ms–1200ms delay, and executes daily sync if due.
5. **Sleep & Retry**: Sleeps for `check_interval_secs` (or `offline_retry_interval_secs` if disconnected).

## 5. Verification Commands
```bash
# Build
cargo build --release --target x86_64-unknown-linux-gnu

# Test
cargo test --all-targets

# Lint & Format
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## 6. Recent Iteration Changes
- **2026-09-29**: Reduced humanized reaction delay in `src/domain/prompt.rs` from 2.2s–5.4s to 500ms–1,200ms (`nanos % 701`) for faster survey prompt response; bumped version to v0.1.11; added `CODEBASE.md`.
