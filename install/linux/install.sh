#!/bin/bash
set -e

echo "FingerPain Installer for Linux"
echo "=============================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/../.."
BINARY_DIR="/usr/local/bin"
SERVICE_DIR="$HOME/.config/systemd/user"

# Check if binaries exist
if [ ! -f "$PROJECT_ROOT/target/release/fingerpain-daemon" ]; then
    echo "Building FingerPain..."
    cd "$PROJECT_ROOT"
    cargo build --release
fi

# Install binaries
echo "Installing binaries to $BINARY_DIR..."
sudo cp "$PROJECT_ROOT/target/release/fingerpain-daemon" "$BINARY_DIR/"
sudo cp "$PROJECT_ROOT/target/release/fingerpain" "$BINARY_DIR/"
sudo chmod +x "$BINARY_DIR/fingerpain-daemon"
sudo chmod +x "$BINARY_DIR/fingerpain"

# Install systemd user service
echo "Installing systemd user service..."
mkdir -p "$SERVICE_DIR"
cp "$SCRIPT_DIR/fingerpain.service" "$SERVICE_DIR/"

# Reload systemd and enable service
echo "Enabling and starting service..."
systemctl --user daemon-reload
systemctl --user enable fingerpain.service
systemctl --user start fingerpain.service

# Check if user is in input group
if ! groups | grep -q '\binput\b'; then
    echo ""
    echo "WARNING: You may need to add yourself to the 'input' group for keyboard access:"
    echo "  sudo usermod -aG input $USER"
    echo "Then log out and back in."
fi

echo ""
echo "Installation complete!"
echo ""
echo "Commands:"
echo "  fingerpain status  - Check daemon status"
echo "  fingerpain today   - View today's stats"
echo ""
echo "Service management:"
echo "  systemctl --user status fingerpain   - Check service status"
echo "  systemctl --user stop fingerpain     - Stop service"
echo "  systemctl --user start fingerpain    - Start service"
echo ""
