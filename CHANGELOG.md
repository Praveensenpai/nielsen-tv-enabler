# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.1.9] - 2026-09-07

Actively verify Android accessibility service bound status and auto-recover from dormant states!

### 🌟 Key Highlights
- **🔍 Active Service Bound Verification**: Inspects `dumpsys accessibility` to verify that Nielsen's accessibility service is actively in `Bound services`, preventing false-positive status when settings are present but Android system server hasn't bound the service.
- **🔄 Autonomous Re-bind Recovery**: Detects dormant/unbound accessibility states and executes an atomic toggle cycle (`settings put secure enabled_accessibility_services`) and wake-up broadcast to force Android to re-bind the service.
- **🧱 Modular Architecture**: Cleanly separated service auto-detection into `domain::detect` to maintain strict module boundaries and file size limits (<350 lines).
- **🧪 Comprehensive Unit Testing**: Added unit test coverage for `dumpsys accessibility` bound parsing, match detection, and re-binding workflows.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.8] - 2026-09-06

Optimized release profile for minimal binary footprint and reduced RAM usage!

### 🌟 Key Highlights
- **📉 40% Smaller Binary Size**: Configured aggressive size optimizations (`opt-level = "z"`), whole-program Link-Time Optimization (`lto = true`), and single codegen unit (`codegen-units = 1`), reducing binary size from 4.5 MB to 2.7 MB.
- **🧠 Lower Memory Footprint**: Streamlined binary code mapping and eliminated stack unwinding metadata (`panic = "abort"`), lowering resident memory (RSS) by ~20% and systemd cgroup memory by ~50%.
- **🔍 Developer-Friendly Debugging**: Preserved symbol tables (`strip = "debuginfo"`) so that panic backtraces retain full module and function names for diagnostics.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.7] - 2026-09-06

Idempotent VPN verification and non-disruptive background daemon management!

### 🌟 Key Highlights
- **🛡️ Idempotent VPN Verification**: Added `is_vpn_tunnel_active`, `is_always_on_vpn_configured`, and `is_vpn_active` to inspect whether a VPN interface (`tun0`) or always-on setting is already running.
- **✨ Non-Disruptive Daemon Cycles**: Leaves active VPN tunnels completely untouched without repeatedly re-issuing settings or appop commands every loop cycle.
- **🔌 Automatic Activation When Disabled**: Activates always-on VPN and background permissions only if the VPN is disabled or unconfigured, and auto-confirms pending VPN authorization dialogs.
- **🧪 Comprehensive Unit Testing**: Added unit tests verifying network interface checks, settings queries, and idempotency guarantees.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.6] - 2026-09-06

Fix CLI version flag to dynamically reflect Cargo package version!

### 🌟 Key Highlights
- **🏷️ Dynamic CLI Versioning**: Replaced static hardcoded version string in `Args` CLI struct with `#[command(version)]`, guaranteeing `-V` and `--version` always precisely reflect the crate's `Cargo.toml` release version.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.5] - 2026-09-06

Safe hot-restarts for active daemons and seamless in-place updates without file-busy locks!

### 🌟 Key Highlights
- **🔄 Zero-Downtime Hot Restart**: Automatically triggers `systemctl --user restart` upon `--install-service` or re-running the 1-click installer, seamlessly swapping running processes.
- **🛡️ ETXTBSY Protection**: Installer now leverages atomic file replacement (`install -m 755`) to eliminate "text file busy" errors when overwriting an active executable.
- **📦 Multi-Location Binary Sync**: Seamlessly detects and updates active binaries in both `~/.local/bin` and `~/.cargo/bin`.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.4] - 2026-09-06

Humanized prompt reaction delays, organic daily sync jitter, and wake-driven sync scheduling!

### 🌟 Key Highlights
- **🎭 Humanized Prompt Reaction Delay**: Injects an organic delay between 2,200ms and 5,400ms before answering *"Who is watching?"* survey dialogs to mimic genuine human reaction and decision-making time.
- **🎲 Organic Sync Jitter**: Daily background sync cycles now feature configurable pseudo-random variance (`sync_jitter_mins = 30` by default), avoiding rigid cron-like timing patterns.
- **📺 Organic Wake-Driven Synchronization**: Seamlessly adapts to natural viewer habits where the TV is turned on at arbitrary times throughout the day, aligning background sync triggers with real viewing sessions.
- **⚙️ Configurable Timing Parameters**: Added `sync_jitter_mins` to `config.toml` for full control over timing randomization.
- **🧪 Unit Testing**: Added unit tests for reaction delay boundary ranges and jittered sync tracker intervals.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.3] - 2026-09-06

Pure ADB background automation with zero UI disruption, always-on VPN configuration, and automated daily data sync!

### 🌟 Key Highlights
- **🤫 100% Pure ADB Background Execution**: Zero app launches and zero simulated UI clicks on your TV screen. Keeps your TV experience completely undisturbed.
- **🛡️ Full Background Permissions & Always-On VPN**:
  - Automatically configures Android system `always_on_vpn_app` so the VPN tunnel connects and persists in the background.
  - Automatically grants `ACTIVATE_VPN`, `GET_USAGE_STATS`, `SYSTEM_ALERT_WINDOW`, and Notification Listener permissions via ADB.
- **🔄 Once-A-Day Data Synchronization**:
  - Automatically schedules and triggers Nielsen background data sync once every 24 hours.
  - Enforces a 10-second delay gap (`sync_delay_secs = 10`) after turning on accessibility and permissions before initiating sync.
  - Dispatches `WorkManager` `JobScheduler` jobs and broadcast intents (`HOURLY_INTENT`, `POSTING_INTENT`, `COLLECTION`).
- **⚡ On-Demand CLI Flag**: Added `--sync` (alias `--sync-now`) to immediately trigger background data sync on demand.
- **🧪 Unit Tests Added**: Unit tests for job ID dump parsing and daily sync tracking state.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.2] - 2026-09-06

Autonomous VPN access permission granting and Android TV VPN dialog confirmation!

### 🌟 Key Highlights
- **🛡️ VPN Permission Granting**: Automatically grants `ACTIVATE_VPN` app-op permission via ADB to Nielsen apps (`com.nlsn.confluencetv`), preventing Android background service restrictions.
- **✅ VPN Dialog Auto-Approval**: Monitors for Android's system VPN connection request dialog (`com.android.vpndialogs`), calculates the OK/Allow button coordinates, taps to confirm, and falls back to TV D-Pad keys (`DPAD_RIGHT` + `ENTER`).
- **⚙️ Configurable & Command Line Modes**: Introduced `--vpn` / `--allow-vpn` CLI command and `auto_allow_vpn = true` config setting (enabled by default).
- **🧪 Unit Tests Added**: Full unit test suite for VPN dialog XML parsing and coordinate extraction.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.1] - 2026-09-06

Aesthetic quality improvements, architecture decoupling, and automated CI release generation!

### 🌟 Key Highlights
- **🏗️ Decoupled Hexagonal Architecture**: Introduced `DeviceCommander` port trait in `domain/`, fully isolating pure domain logic from infrastructure adapters.
- **📏 Strict Rust Rules Compliance**: Enforced zero-warning Clippy pedantic policies, sub-300 line file limits, and added 5 domain unit tests.
- **⚙️ Automated GitHub Releases**: Enhanced CI workflow with prerequisite release creation from `CHANGELOG.md` and multi-arch asset uploads.
- **🎨 Aesthetic Documentation**: Revamped `README.md` and release layout with interactive diagrams and command guides.

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## [v0.1.0] - 2026-09-06

First official release featuring continuous background accessibility enforcement, survey prompt auto-dismissal, and automated Android TV subnet scanning!

### 🌟 Key Highlights
- **🤖 Autonomous Prompt Dismissal**: Detects Nielsen's intrusive *"Who is watching?"* modal on screen, calculates member checkbox coordinates, taps randomly, confirms OK, and restores television playback.
- **🔍 Subnet Auto-Discovery**: Concurrently probes all 254 hosts on your local subnet on port `5555` to find and connect to your Android TV in seconds.
- **⚡ Instant Reconnection & Caching**: Caches the last verified working IP for zero-overhead startup and reconnects automatically.
- **💤 Silent & Graceful Offline Backoff**: Backs off and retries when the TV is powered off or asleep without flooding logs or wasting CPU cycles.
- **🛡️ Native Systemd User Service**: One command to install, monitor, or remove background service integration (`--install-service`, `--status-service`).

### 📦 Multi-Architecture Binaries
- **x86_64 Linux**: `nielsen-tv-enabler-x86_64-unknown-linux-gnu.tar.gz`
- **aarch64 / ARM64 Linux**: `nielsen-tv-enabler-aarch64-unknown-linux-gnu.tar.gz`

### ⚡ Quick 1-Click Install
```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```
