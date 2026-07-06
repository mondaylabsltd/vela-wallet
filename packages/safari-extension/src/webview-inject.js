// In-app WalletWebView injected bundle (iOS WKWebView + Android android.webkit.WebView).
//
// Single-sourced with the Safari extension: this pulls in the SAME EIP-1193/6963
// provider (inpage.js) and the SAME wire protocol (lib/protocol.js), then adds a
// thin native bridge shim that fills content.js's page<->host relay role — minus
// all the Safari UI / App-Group / sign-hand-off machinery, which becomes native RN.
//
// inpage.js never learns which surface it runs on: it always speaks the same
// `vela-1193` req/res/evt envelope over window.postMessage. Only the LAST hop
// differs — here it is a native message channel instead of browser.runtime.
//
// Injected at DOCUMENT START, main frame only (iOS WKUserScript(.atDocumentStart,
// forMainFrameOnly:true); Android WebViewCompat.addDocumentStartJavaScript).
import './inpage.js'; // side-effect: installs window.ethereum + the vela-1193 listener
import { CHANNEL } from './lib/protocol.js';

(() => {
  // ---- page -> native ------------------------------------------------------
  // Forward every request the provider posts. Native stamps the trusted origin
  // + isMainFrame itself (it must NEVER trust an origin in this JS payload).
  function nativePost(payload) {
    const s = typeof payload === 'string' ? payload : JSON.stringify(payload);
    try {
      if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.velaBridge) {
        window.webkit.messageHandlers.velaBridge.postMessage(s); // iOS WKScriptMessageHandler
      } else if (window.velaBridge && typeof window.velaBridge.postMessage === 'function') {
        window.velaBridge.postMessage(s); // Android addWebMessageListener("velaBridge")
      }
    } catch (_) {
      /* host channel not ready — provider's request will time out, which is safe */
    }
  }

  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
    nativePost(d);
  });

  // ---- native -> page ------------------------------------------------------
  // The native side owns the vela-1193 envelope through these two entry points,
  // so the RN/native layer never needs to know CHANNEL (no cross-language drift).
  // Re-posting as a window message means inpage's receiver (ev.source === window)
  // accepts it exactly like a Safari content.js response.
  window.__velaRespond = (id, result, error) => {
    try {
      const env = error
        ? { ch: CHANNEL, dir: 'res', id, error }
        : { ch: CHANNEL, dir: 'res', id, result };
      window.postMessage(env, window.location.origin);
    } catch (_) {
      /* malformed delivery; ignore */
    }
  };
  window.__velaEmit = (event, data) => {
    try {
      window.postMessage({ ch: CHANNEL, dir: 'evt', event, data }, window.location.origin);
    } catch (_) {
      /* ignore */
    }
  };

  // Let native know the provider installed (it may gate first delivery on this).
  nativePost({ ch: CHANNEL, dir: 'ready' });
})();
