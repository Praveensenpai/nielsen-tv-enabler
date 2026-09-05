#!/usr/bin/env bash
set -euo pipefail

REPO="Praveensenpai/nielsen-tv-enabler"
BINARY_NAME="nielsen-tv-enabler"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${BLUE}${BOLD}=== Nielsen TV Enabler 1-Click Installer ===${NC}"

# Detect OS and Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$OS" != "linux" ]; then
    echo -e "${RED}Error: This installer currently only supports Linux.${NC}"
    exit 1
fi

case "$ARCH" in
    x86_64)
        TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        TARGET="aarch64-unknown-linux-gnu"
        ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

echo -e "Detected: ${GREEN}${OS} (${ARCH})${NC} -> Target: ${GREEN}${TARGET}${NC}"

# Create install directory
mkdir -p "$INSTALL_DIR"
export PATH="$INSTALL_DIR:$PATH"

# Download binary release
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

ARCHIVE_NAME="${BINARY_NAME}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}"

echo -e "Downloading latest release from: ${BLUE}${DOWNLOAD_URL}${NC}..."
if curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE_NAME"; then
    echo -e "Extracting archive..."
    tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"
    cp "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    echo -e "${GREEN}✓ Successfully installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}${NC}"
else
    # Fallback: if cargo is available, build from source
    echo -e "${YELLOW}Could not download prebuilt binary. Checking if cargo is installed...${NC}"
    if command -v cargo >/dev/null 2>&1; then
        echo -e "Building from source via cargo..."
        cargo install --git "https://github.com/${REPO}.git"
        echo -e "${GREEN}✓ Successfully installed via cargo!${NC}"
    else
        echo -e "${RED}Failed to download binary and cargo is not installed.${NC}"
        exit 1
    fi
fi

# Check for ADB
if ! command -v adb >/dev/null 2>&1 && [ ! -f "$HOME/Android/Sdk/platform-tools/adb" ]; then
    echo -e "${YELLOW}⚠️ Warning: 'adb' was not found in PATH.${NC}"
    echo -e "Please install Android platform tools:"
    echo -e "  Arch Linux: sudo pacman -S android-tools"
    echo -e "  Debian/Ubuntu: sudo apt install adb"
fi

# Automatically install and enable systemd user service
echo -e "\n${BLUE}Setting up systemd user service...${NC}"
"$INSTALL_DIR/$BINARY_NAME" --install-service

echo -e "\n${GREEN}${BOLD}🎉 Installation Complete!${NC}"
echo -e "The Nielsen TV Enabler daemon is now running in the background."
echo -e ""
echo -e "Helpful commands:"
echo -e "  - Check status: ${BOLD}${BINARY_NAME} --status-service${NC}"
echo -e "  - View live logs: ${BOLD}journalctl --user -u ${BINARY_NAME}.service -f${NC}"
echo -e "  - Manual single pass: ${BOLD}${BINARY_NAME} --once${NC}"
echo -e "  - Config file: ${BOLD}~/.config/nielsen-tv-enabler/config.toml${NC}"
