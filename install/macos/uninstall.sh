#!/bin/bash
set -e

echo "FingerPain Uninstaller for macOS"
echo "================================"

BINARY_DIR="/usr/local/bin"
PLIST_DIR="$HOME/Library/LaunchAgents"
DATA_DIR="$HOME/Library/Application Support/com.fingerpain.fingerpain"

# Stop and unload LaunchAgent
echo "Stopping daemon..."
launchctl unload "$PLIST_DIR/com.fingerpain.daemon.plist" 2>/dev/null || true

# Remove LaunchAgent
echo "Removing LaunchAgent..."
rm -f "$PLIST_DIR/com.fingerpain.daemon.plist"

# Remove binaries
echo "Removing binaries..."
sudo rm -f "$BINARY_DIR/fingerpain-daemon"
sudo rm -f "$BINARY_DIR/fingerpain"

# Ask about data
read -p "Do you want to remove all FingerPain data? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Removing data..."
    rm -rf "$DATA_DIR"
    echo "Data removed."
else
    echo "Data preserved at: $DATA_DIR"
fi

echo ""
echo "Uninstallation complete!"
