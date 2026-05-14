/**
 * bridge.js — Runs in ISOLATED world (has chrome.runtime API).
 *
 * Relays messages between inject.js (page/MAIN world, via postMessage)
 * and background.js (service worker, via chrome.runtime.sendMessage).
 */

const TAG = '[VelaWebAuthnProxy:bridge]';

window.addEventListener('message', async (e) => {
  if (e.source !== window) return;
  if (e.data?.type !== 'VELA_WEBAUTHN_REQUEST') return;

  const { id, method, options } = e.data;
  console.log(TAG, `forwarding ${method} #${id} to background`);

  try {
    const response = await chrome.runtime.sendMessage({
      type: 'VELA_WEBAUTHN_REQUEST',
      id,
      method,
      options,
    });

    if (response?.error) {
      window.postMessage({ type: 'VELA_WEBAUTHN_RESPONSE', id, method, error: response.error }, '*');
    } else {
      window.postMessage({ type: 'VELA_WEBAUTHN_RESPONSE', id, method, result: response.result }, '*');
    }
  } catch (err) {
    console.error(TAG, 'sendMessage failed:', err);
    window.postMessage({
      type: 'VELA_WEBAUTHN_RESPONSE',
      id,
      method,
      error: err.message || 'Bridge failed',
    }, '*');
  }
});

console.log(TAG, 'ready');
