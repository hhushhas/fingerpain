/**
 * FingerPain Browser Tracker
 *
 * This service worker tracks the currently active tab and sends context updates
 * to the FingerPain daemon via the browser-context API endpoint.
 */

let currentTabId = null;
let currentUrl = null;
let browserName = 'Chrome'; // Default to Chrome

// Detect browser name from user agent
if (navigator.userAgent.includes('Helium')) {
  browserName = 'Helium';
}

/**
 * Send context update to the local FingerPain API
 */
async function updateContext(url, title, browserNameOverride = null) {
  const name = browserNameOverride || browserName;

  try {
    const response = await fetch('http://127.0.0.1:7890/api/browser-context', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        url: url,
        title: title,
        browser_name: name,
        timestamp: Date.now(),
      }),
    });

    if (!response.ok) {
      console.warn('Failed to update browser context:', response.status);
    }
  } catch (err) {
    // Silent fail - daemon might not be running yet
    console.debug('FingerPain daemon not responding (OK if not started):', err.message);
  }
}

/**
 * Handle tab activation (user switches tabs)
 */
chrome.tabs.onActivated.addListener(async (activeInfo) => {
  try {
    const tab = await chrome.tabs.get(activeInfo.tabId);
    currentTabId = tab.id;
    currentUrl = tab.url;
    await updateContext(tab.url, tab.title);
  } catch (err) {
    console.error('Error in onActivated:', err);
  }
});

/**
 * Handle tab updates (URL changes, title changes)
 */
chrome.tabs.onUpdated.addListener(async (tabId, changeInfo, tab) => {
  // Only process updates to the active tab
  if (tabId !== currentTabId) {
    return;
  }

  // Only process URL changes (navigation)
  if (changeInfo.url) {
    currentUrl = changeInfo.url;
    await updateContext(tab.url, tab.title);
  }
});

/**
 * Handle window focus changes
 */
chrome.windows.onFocusChanged.addListener(async (windowId) => {
  if (windowId !== chrome.windows.WINDOW_ID_NONE && currentTabId) {
    try {
      const tab = await chrome.tabs.get(currentTabId);
      await updateContext(tab.url, tab.title);
    } catch (err) {
      console.error('Error in onFocusChanged:', err);
    }
  }
});

/**
 * Initialize on extension startup
 */
chrome.runtime.onStartup.addListener(async () => {
  try {
    const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tabs.length > 0) {
      const tab = tabs[0];
      currentTabId = tab.id;
      currentUrl = tab.url;
      await updateContext(tab.url, tab.title);
    }
  } catch (err) {
    console.error('Error on startup:', err);
  }
});

/**
 * Also initialize when extension is installed or reloaded
 */
chrome.runtime.onInstalled.addListener(async () => {
  try {
    const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tabs.length > 0) {
      const tab = tabs[0];
      currentTabId = tab.id;
      currentUrl = tab.url;
      await updateContext(tab.url, tab.title);
    }
  } catch (err) {
    console.error('Error on install:', err);
  }
});

console.log('FingerPain Browser Tracker initialized. Tracking:', browserName);
