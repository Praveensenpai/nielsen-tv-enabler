# Nielsen TV Accessibility Keeper (`nielsen-tv-enabler`)

A lightweight, robust Rust daemon and CLI tool that automatically connects to your Android TV via ADB and ensures that Nielsen's Accessibility Service (`com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService`) remains enabled at all times, even after the TV reboots, sleeps, or turns off.

---

## Features

- **Auto-Dismiss "Who is Watching?"**: Automatically detects Nielsen's periodic "Who is watching?" survey popup on the TV screen, randomly picks a household member, clicks OK, and dismisses the dashboard overlay so TV playback is uninterrupted.
- **Subnet Auto-Scanning**: Automatically scans your local network (`192.168.1.0/24`) on port `5555` concurrently across all 254 hosts in seconds.
- **Smart IP Caching**: Caches the last known working IP address for instantaneous reconnection on startup.
- **Graceful Offline Handling**: When the TV is asleep or powered off, it backs off and retries cleanly without spamming logs.
- **Idempotent & Safe**: Preserves existing enabled accessibility services (joins them with colons) and ensures `accessibility_enabled=1`.
- **Systemd User Service**: One command to install, start, stop, or check status as a background daemon on login/boot.
- **Configurable**: Fully customizable via `~/.config/nielsen-tv-enabler/config.toml` or CLI flags.

---

## ⚡ Quick 1-Click Install

Run this single command to download, install, and automatically start the background daemon:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/nielsen-tv-enabler/main/install.sh | bash
```

---

## 🛠️ Build from Source

```bash
git clone https://github.com/Praveensenpai/nielsen-tv-enabler.git
cd nielsen-tv-enabler
cargo install --path .
nielsen-tv-enabler --install-service
```

---

## Usage

### 1. Run as Background Systemd Service (Recommended)

To install and start the background daemon:
```bash
nielsen-tv-enabler --install-service
```

Check status:
```bash
nielsen-tv-enabler --status-service
```

View live logs:
```bash
journalctl --user -u nielsen-tv-enabler.service -f
```

To stop and remove the service:
```bash
nielsen-tv-enabler --uninstall-service
```

---

### 2. CLI Modes

- **Single check and enable pass**:
  ```bash
  nielsen-tv-enabler --once
  ```

- **Scan local network for Android TV / ADB devices**:
  ```bash
  nielsen-tv-enabler --scan
  ```

- **Detect Nielsen service component on TV**:
  ```bash
  nielsen-tv-enabler --detect
  ```

- **Run in foreground terminal / tmux**:
  ```bash
  nielsen-tv-enabler --daemon
  ```

- **Verbose debug output**:
  ```bash
  nielsen-tv-enabler -v
  ```

---

## Configuration

Settings are saved in `~/.config/nielsen-tv-enabler/config.toml`:

```toml
# "auto" uses cached last_known_ip first, then scans subnet if unreachable
tv_ip = "auto"
adb_port = 5555
last_known_ip = "192.168.1.35"
service_component = "com.nlsn.confluencetv/nielsen.imi.acsdk.services.NxtLogService"
check_interval_secs = 30
offline_retry_interval_secs = 45
```

You can also override any configuration value using CLI flags:
- `-i, --ip <IP>`: Specify target IP
- `-p, --port <PORT>`: Specify ADB port
- `-s, --service <COMPONENT>`: Specify service component
- `-t, --interval <SECONDS>`: Set check interval
