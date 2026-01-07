# FingerPain

Cross-platform typing analytics tracker. Counts keystrokes without capturing content.

## Features

| Feature | Description |
|---------|-------------|
| Privacy-first | Tracks counts only, never actual keystrokes |
| Per-minute data | Aggregate by minute, hour, day, week, month, year |
| Per-app stats | See which apps you type in most |
| WPM tracking | Average and peak words-per-minute |
| Export | CSV and JSON formats |
| Multi-interface | CLI, web dashboard, menu bar tray |

## Quick Start

```bash
# Build
cargo build --release

# Install daemon (macOS)
cp target/release/fingerpain-daemon /Applications/FingerPain.app/Contents/MacOS/FingerPain
open /Applications/FingerPain.app

# Grant Accessibility permissions when prompted

# Check stats
fingerpain today
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `fingerpain today` | Today's stats |
| `fingerpain yesterday` | Yesterday's stats |
| `fingerpain week` | This week |
| `fingerpain month` | This month |
| `fingerpain year` | This year |
| `fingerpain range 2026-01-01 2026-01-07` | Custom range |
| `fingerpain peak` | Top typing periods |
| `fingerpain apps` | Per-app breakdown |
| `fingerpain export -f json -o stats.json` | Export data |
| `fingerpain status` | Daemon status |

## Web Dashboard

```bash
fingerpain-web
# Open http://127.0.0.1:7890
```

## Architecture

```
crates/
├── fingerpain-core     # Types, DB, metrics, export
├── fingerpain-listener # Cross-platform keystroke capture (rdev)
├── fingerpain-daemon   # Background service
├── fingerpain-cli      # Command-line interface
├── fingerpain-tray     # Menu bar app
└── fingerpain-web      # Web dashboard (Axum)
```

## Data Storage

- **macOS**: `~/Library/Application Support/com.fingerpain.fingerpain/fingerpain.db`
- **Linux**: `~/.local/share/fingerpain/fingerpain.db`
- **Windows**: `%APPDATA%\fingerpain\fingerpain.db`

## Auto-Start

**macOS**: LaunchAgent at `~/Library/LaunchAgents/com.fingerpain.daemon.plist`

**Linux**: systemd user service at `~/.config/systemd/user/fingerpain.service`

**Windows**: Registry key at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

## macOS Setup

### First-time Install

```bash
# 1. Build
cargo build --release

# 2. Create app bundle
mkdir -p /Applications/FingerPain.app/Contents/MacOS
cp target/release/fingerpain-daemon /Applications/FingerPain.app/Contents/MacOS/FingerPain
chmod +x /Applications/FingerPain.app/Contents/MacOS/FingerPain

# 3. Launch (triggers permission prompt)
open /Applications/FingerPain.app
```

### Required Permissions

| Permission | Location | Why |
|------------|----------|-----|
| Accessibility | System Settings → Privacy & Security → Accessibility | Capture keystrokes |
| Input Monitoring | System Settings → Privacy & Security → Input Monitoring | Some macOS versions require this too |

### Troubleshooting

| Problem | Solution |
|---------|----------|
| Not recording keystrokes | Grant both Accessibility AND Input Monitoring permissions |
| Permission granted but still not working | Toggle permission OFF → ON, then restart daemon |
| Daemon running but 0 new keystrokes | Kill daemon (`pkill FingerPain`), re-grant permissions, restart |
| Can't add app to permissions | Must use `.app` bundle, not raw binary |

### Restart Daemon

```bash
pkill FingerPain
open /Applications/FingerPain.app
```

### Verify Recording

```bash
# Type something, wait 60 seconds (data flushes per minute), then:
fingerpain today
```

## Other Platforms

- **Linux**: Add user to input group (`sudo usermod -aG input $USER`), logout/login
- **Windows**: Run as administrator for global capture

## Tech Stack

| Component | Library |
|-----------|---------|
| Language | Rust |
| Database | SQLite (rusqlite) |
| Keyboard capture | rdev |
| CLI | clap |
| Web server | Axum + Tokio |
| Tray | tray-icon + tao |

## License

MIT
