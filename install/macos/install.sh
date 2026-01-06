#!/bin/bash
set -e

echo "FingerPain Installer for macOS"
echo "=============================="

# Check if running as user (not root)
if [ "$EUID" -eq 0 ]; then
    echo "Please run this script as a regular user, not as root."
    exit 1
fi

# Determine installation paths
BINARY_DIR="/usr/local/bin"
PLIST_DIR="$HOME/Library/LaunchAgents"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if binaries exist
if [ ! -f "$SCRIPT_DIR/../../target/release/fingerpain-daemon" ]; then
    echo "Building FingerPain..."
    cd "$SCRIPT_DIR/../.."
    cargo build --release
fi

# Install binaries
echo "Installing binaries to $BINARY_DIR..."
sudo cp "$SCRIPT_DIR/../../target/release/fingerpain-daemon" "$BINARY_DIR/"
sudo cp "$SCRIPT_DIR/../../target/release/fingerpain" "$BINARY_DIR/"
sudo chmod +x "$BINARY_DIR/fingerpain-daemon"
sudo chmod +x "$BINARY_DIR/fingerpain"

# Install LaunchAgent
echo "Installing LaunchAgent..."
mkdir -p "$PLIST_DIR"
cp "$SCRIPT_DIR/com.fingerpain.daemon.plist" "$PLIST_DIR/"

# Load LaunchAgent
echo "Loading LaunchAgent..."
launchctl load "$PLIST_DIR/com.fingerpain.daemon.plist" 2>/dev/null || true

echo ""
echo "Installation complete!"
echo ""
echo "IMPORTANT: You need to grant Accessibility permissions to fingerpain-daemon."
echo "Go to System Preferences → Security & Privacy → Privacy → Accessibility"
echo "and add /usr/local/bin/fingerpain-daemon"
echo ""
echo "To check status:  fingerpain status"
echo "To view stats:    fingerpain today"
echo ""
