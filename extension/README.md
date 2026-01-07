# FingerPain Browser Tracker Extension

This Chromium extension tracks your active browser tab and sends webpage information to FingerPain for contextual typing analytics.

## Features

- Automatically detects active tab and domain
- Sends updates when you switch tabs or navigate
- Works with both Helium and Chrome browsers
- Fails silently if daemon is not running

## Installation

### Step 1: Create Icons (First Time Only)

The extension requires three PNG icon files. You can create simple placeholder icons using ImageMagick:

```bash
brew install imagemagick  # If not already installed

convert -size 16x16 xc:#FF6B6B icons/icon-16.png
convert -size 48x48 xc:#FF6B6B icons/icon-48.png
convert -size 128x128 xc:#FF6B6B icons/icon-128.png
```

Or use any PNG images you prefer (16x16, 48x48, 128x128 pixels).

### Step 2: Load Extension in Browser

**For Chrome or Helium:**

1. Open `chrome://extensions` (or `helium://extensions`)
2. Enable "Developer mode" (toggle in top-right)
3. Click "Load unpacked"
4. Navigate to the `extension/` directory of this project and select it

### Step 3: Verify Installation

1. The extension icon should appear in your toolbar
2. Open your browser console (`Cmd+Option+J`) to see debug messages
3. The console should show: `"FingerPain Browser Tracker initialized. Tracking: [Browser Name]"`

## How It Works

The extension:
- Monitors tab changes using `chrome.tabs.onActivated`
- Detects URL navigation using `chrome.tabs.onUpdated`
- Sends HTTP POST to `http://127.0.0.1:7890/api/browser-context`
- Includes: URL, page title, browser name, timestamp

The daemon receives this context and associates it with keystrokes recorded during that minute.

## Troubleshooting

### Extension not loading?
- Make sure you're in the correct directory: `fingerpain/extension/`
- Verify `manifest.json` and `background.js` are present
- Check that PNG icon files exist in `icons/` folder
- Try refreshing the extension (reload button on extensions page)

### Not tracking domains?
- Verify daemon is running: `pgrep -l FingerPain`
- Check browser console for POST errors
- Confirm API endpoint is accessible: `curl http://127.0.0.1:7890/api/browser-context`
- Make sure Helium/Chrome has Accessibility permissions (same as daemon)

### Permissions warning?
This is normal. The extension needs `<all_urls>` host permission to access webpage titles and URLs.

## Testing

After installation, test with:

```bash
# In one terminal, start the daemon:
cargo run --bin fingerpain-daemon

# In another terminal, start the web server:
cargo run --bin fingerpain-web

# Now navigate to different websites in your browser and type
# Wait 1 minute for data to flush, then check:
cargo run --bin fingerpain -- apps

# You should see domains listed under the browser app:
# Helium    1,234    500    45.2%
#   → x.com    800    320    64.8%
#   → chatgpt.com ...
```

## Development

The extension uses Manifest V3 (latest standard) and:
- Service Worker (`background.js`) for event handling
- Chrome Extension APIs for tab tracking
- Fetch API for HTTP communication

To debug:
- Open `chrome://extensions`
- Find FingerPain Browser Tracker
- Click "Service worker" to open debugger
- Check console for logs (use `console.log()` in `background.js`)

## Permissions

- `tabs`: Required to read active tab information
- `activeTab`: Required for current tab access
- `<all_urls>`: Required to capture page titles and domains from any website

## Notes

- The daemon must be running for tracking to work
- Domain data is only sent when actively typing
- Fails silently if API is unavailable (daemon not running)
- Works offline; no cloud communication
