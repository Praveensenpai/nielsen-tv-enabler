# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
