/**
 * background.js — Service worker.
 *
 * Opens a small popup window (webauthn.html) to perform the actual
 * WebAuthn call. A visible extension page is required for the
 * platform authenticator (Touch ID / Windows Hello) dialog to appear.
 */

let pendingRequest = null;

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  // From content script → open popup to perform WebAuthn
  if (msg.type === 'VELA_WEBAUTHN_REQUEST') {
    pendingRequest = { sendResponse };

    chrome.windows.create({
      url: chrome.runtime.getURL(
        `webauthn.html?method=${msg.method}&data=${encodeURIComponent(JSON.stringify(msg.options))}`
      ),
      type: 'popup',
      width: 420,
      height: 320,
      focused: true,
    });

    return true; // async sendResponse
  }

  // From webauthn.html → relay result back to content script
  if (msg.type === 'VELA_WEBAUTHN_RESULT') {
    if (pendingRequest) {
      pendingRequest.sendResponse(msg);
      pendingRequest = null;
    }
    // Close the popup window
    if (sender.tab?.windowId) {
      chrome.windows.remove(sender.tab.windowId);
    }
    return false;
  }
});
