#!/bin/bash
set -e

echo "FingerPain Uninstaller for Linux"
echo "================================="

BINARY_DIR="/usr/local/bin"
SERVICE_DIR="$HOME/.config/systemd/user"
DATA_DIR="$HOME/.local/share/fingerpain"

# Stop and disable service
echo "Stopping service..."
systemctl --user stop fingerpain.service 2>/dev/null || true
systemctl --user disable fingerpain.service 2>/dev/null || true

# Remove service file
echo "Removing systemd service..."
rm -f "$SERVICE_DIR/fingerpain.service"
systemctl --user daemon-reload

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
