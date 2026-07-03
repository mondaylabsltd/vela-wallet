/**
 * background.js — Service worker.
 *
 * Opens a small popup window (webauthn.html) to perform the actual
 * WebAuthn call. A visible extension page is required for the
 * platform authenticator (Touch ID / Windows Hello) dialog to appear.
 */

// One pending sendResponse per popup window. A Map (not a single slot) so
// concurrent requests can't clobber each other, and keyed by windowId so a
// popup the user closes by hand can fail its own request — a dangling entry
// used to leave the page's WebAuthn promise hanging forever.
const pendingByWindow = new Map();

// Toolbar icon → open the settings (configure the target rp domain).
chrome.action.onClicked.addListener(() => chrome.runtime.openOptionsPage());

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  // From content script → open popup to perform WebAuthn
  if (msg.type === 'VELA_WEBAUTHN_REQUEST') {
    chrome.windows.create(
      {
        url: chrome.runtime.getURL(
          `webauthn.html?method=${msg.method}&data=${encodeURIComponent(JSON.stringify(msg.options))}`
        ),
        type: 'popup',
        width: 480,
        height: 640,
        focused: true,
      },
      (win) => {
        if (!win) {
          sendResponse({ error: 'Could not open the passkey window.' });
          return;
        }
        pendingByWindow.set(win.id, sendResponse);
      }
    );

    return true; // async sendResponse
  }

  // From webauthn.html → relay result back to content script
  if (msg.type === 'VELA_WEBAUTHN_RESULT') {
    const windowId = sender.tab?.windowId;
    const respond = windowId != null ? pendingByWindow.get(windowId) : undefined;
    if (respond) {
      pendingByWindow.delete(windowId);
      respond(msg);
    }
    // Close the popup window
    if (windowId != null) {
      chrome.windows.remove(windowId);
    }
    return false;
  }
});

// User closed the popup before completing the ceremony → fail the page's
// promise cleanly instead of leaving it pending forever. (When the result
// handler above closes the window, its entry is already gone — no double fire.)
chrome.windows.onRemoved.addListener((windowId) => {
  const respond = pendingByWindow.get(windowId);
  if (respond) {
    pendingByWindow.delete(windowId);
    // Closing the window IS a user cancellation — keep NotAllowedError semantics.
    respond({ error: 'The passkey window was closed before completing.', errorName: 'NotAllowedError' });
  }
});
