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

### `src/domain/prompt.rs` (Role: domain)
- **Responsibility**: Detects and answers the "Who is watching?" survey prompt with humanized delay and random member selection.
- **Imports**: `crate::domain::DeviceCommander`, `regex::Regex`, `std::time::{Duration, SystemTime, UNIX_EPOCH}`
- **Public Functions & Signatures**:
  ```rust
  pub fn handle_who_is_watching(device: &impl DeviceCommander, device_target: &str) -> Result<bool>
  pub fn humanized_reaction_delay() -> Duration // 500ms..=1200ms
  ```
- **Private Functions**:
  ```rust
  fn nielsen_in_foreground(device: &impl DeviceCommander, device_target: &str) -> bool // window OR activity dump
  fn dump_ui_with_retry(device: &impl DeviceCommander, device_target: &str) -> Option<String> // retries once
  fn extract_member_checkboxes(ui_dump: &str) -> Vec<(String, u32, u32)>
  fn pick_random_member(options: &[(String, u32, u32)]) -> (String, u32, u32)
  fn extract_ok_button(ui_dump: &str) -> (u32, u32)
  fn dismiss_overlay_if_active(device: &impl DeviceCommander, device_target: &str)
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: Device UI automator dump, ADB tap inputs.
- **Notes**: Detection checks both `mCurrentFocus`/`mFocusedApp` and `ResumedActivity`, since `PersonDialogActivity` can leave `mCurrentFocus=null`. UI dump retries once because `uiautomator` intermittently returns "null root node".

### `src/domain/service.rs` (Role: domain)
- **Responsibility**: Queries and activates the Nielsen Accessibility Service, detecting bound/binding state with rate-limited remediation.
- **Imports**: `crate::domain::DeviceCommander`, `crate::domain::detect`, `std::sync::atomic`, `std::time`
- **Public Functions & Signatures**:
  ```rust
  pub fn ensure_accessibility_enabled(device: &impl DeviceCommander, device_target: &str, nielsen_component: &str) -> Result<bool>
  pub fn is_accessibility_globally_enabled(device: &impl DeviceCommander, device_target: &str) -> Result<bool>
  pub fn get_enabled_accessibility_services(device: &impl DeviceCommander, device_target: &str) -> Result<Vec<String>>
  pub fn is_nielsen_match(service: &str, nielsen_component: &str) -> bool
  pub fn is_service_bound(device: &impl DeviceCommander, device_target: &str, nielsen_component: &str) -> bool
  pub fn parse_is_service_bound(dumpsys: &str, nielsen_component: &str) -> bool
  pub fn parse_is_service_binding(dumpsys: &str, nielsen_component: &str) -> bool
  ```
- **Private State / Functions**:
  ```rust
  const REBIND_BACKOFF: Duration = 60s
  static LAST_REBIND_MS: AtomicU64
  fn rebind_backoff_elapsed() -> bool
  fn force_rebind_toggle(...) -> Result<()>
  fn verify_enabled_state(...) -> Result<bool>
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: ADB settings commands, dumpsys queries.
- **Notes**: If the service is `Bound`/`Binding`, settings are left untouched (rewriting cancels Android's in-flight bind). All remediation is rate-limited to one attempt per `REBIND_BACKOFF` window to avoid wedging the accessibility framework.

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

### `src/infra/adb.rs` (Role: infra)
- **Responsibility**: Wraps `adb` CLI binary commands and implements `DeviceCommander`.
- **Types & Enums**:
  ```rust
  pub struct AdbClient { pub adb_binary: PathBuf }
  pub enum DeviceStatus { Ready, Unauthorized, Offline, NotFound }
  pub struct DeviceInfo { pub serial: String, pub state: String }
  ```
- **Private Helpers**:
  ```rust
  const ADB_COMMAND_TIMEOUT: Duration = 15s
  fn spawn_reader<R: Read + Send + 'static>(reader: R) -> thread::JoinHandle<String>
  fn join_reader(handle: Option<thread::JoinHandle<String>>) -> String
  fn resolve_adb_path(custom_path: Option<&str>) -> PathBuf
  fn verify_adb_binary(path: &Path) -> Result<()>
  ```
- **Consumers**: `src/app.rs`, `src/app/daemon.rs`
- **Side Effects / I/O**: Subprocess execution of `adb`.
- **Notes**: `run_adb` polls the child process and kills it after `ADB_COMMAND_TIMEOUT`, so a wedged `adb shell` cannot stall the daemon loop.

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
4. **Active Cycle**: Checks accessibility service state via `dumpsys`. If the service is bound or binding, settings are left untouched; if unbound and listed, remediation is rate-limited to one attempt per 60s backoff window. Grants VPN, dismisses "Who is watching?" with 500ms–1200ms delay, and executes daily sync if due.
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
- **2026-10-06**: Fixed "Who is watching?" prompt never being dismissed. Root cause: `ensure_accessibility_enabled` rewrote `enabled_accessibility_services` on every cycle even while the service was in Android's `Binding` state, cancelling the in-flight bind and wedging the accessibility framework (which made `uiautomator` return "null root node"). Fixes: (1) `run_adb` now enforces a 15s timeout so a wedged shell cannot stall the daemon; (2) `ensure_accessibility_enabled` leaves settings untouched while bound/binding and rate-limits remediation to one attempt per 60s `REBIND_BACKOFF` window; (3) `handle_who_is_watching` checks both window and `ResumedActivity` focus and retries the UI dump once.
- **2026-09-29**: Reduced humanized reaction delay in `src/domain/prompt.rs` from 2.2s–5.4s to 500ms–1,200ms (`nanos % 701`) for faster survey prompt response; bumped version to v0.1.11; added `CODEBASE.md`.
