/**
 * inject.js — Runs in MAIN world (page context) at document_start.
 *
 * Guaranteed to run before any page JS because Chrome injects
 * MAIN-world content scripts synchronously at document_start.
 *
 * Monkey-patches navigator.credentials.create/.get to proxy
 * localhost rpId calls through the extension.
 */

(() => {
  const DEFAULT_RP_ID = 'getvela.app';
  // Current target rpId. Defaults to getvela.app, overridden at runtime by
  // bridge.js (which reads the user's configured domain from chrome.storage).
  let configuredRpId = DEFAULT_RP_ID;
  const TAG = '[VelaWebAuthnProxy]';
  let reqId = 0;
  const pending = new Map();

  function bufToB64(buf) {
    if (!buf) return null;
    const bytes = new Uint8Array(buf.buffer || buf);
    let binary = '';
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
  }

  function b64ToBuf(b64) {
    if (!b64) return null;
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes.buffer;
  }

  function serializeCreateOptions(opts) {
    const pk = Object.assign({}, opts.publicKey);
    pk.challenge = bufToB64(pk.challenge);
    if (pk.user) pk.user = Object.assign({}, pk.user, { id: bufToB64(pk.user.id) });
    if (pk.excludeCredentials) {
      pk.excludeCredentials = pk.excludeCredentials.map(c => Object.assign({}, c, { id: bufToB64(c.id) }));
    }
    return { publicKey: pk };
  }

  function serializeGetOptions(opts) {
    const pk = Object.assign({}, opts.publicKey);
    pk.challenge = bufToB64(pk.challenge);
    if (pk.allowCredentials) {
      pk.allowCredentials = pk.allowCredentials.map(c => Object.assign({}, c, { id: bufToB64(c.id) }));
    }
    return { publicKey: pk };
  }

  function deserializeCreateResponse(data) {
    return {
      id: data.id,
      rawId: b64ToBuf(data.rawId),
      type: data.type,
      authenticatorAttachment: data.authenticatorAttachment,
      response: {
        clientDataJSON: b64ToBuf(data.response.clientDataJSON),
        attestationObject: b64ToBuf(data.response.attestationObject),
        getTransports: () => data.response.transports || [],
        getPublicKey: () => b64ToBuf(data.response.publicKey),
        getPublicKeyAlgorithm: () => data.response.publicKeyAlgorithm,
        getAuthenticatorData: () => b64ToBuf(data.response.authenticatorData),
      },
      getClientExtensionResults: () => data.clientExtensionResults || {},
    };
  }

  function deserializeGetResponse(data) {
    return {
      id: data.id,
      rawId: b64ToBuf(data.rawId),
      type: data.type,
      authenticatorAttachment: data.authenticatorAttachment,
      response: {
        clientDataJSON: b64ToBuf(data.response.clientDataJSON),
        authenticatorData: b64ToBuf(data.response.authenticatorData),
        signature: b64ToBuf(data.response.signature),
        userHandle: b64ToBuf(data.response.userHandle),
      },
      getClientExtensionResults: () => data.clientExtensionResults || {},
    };
  }

  // Listen for messages from bridge.js
  window.addEventListener('message', (e) => {
    if (e.source !== window) return;
    if (!e.data) return;
    // Config push from bridge.js (the user's configured target domain).
    if (e.data.type === 'VELA_WEBAUTHN_CONFIG') {
      if (e.data.proxyRpId) {
        configuredRpId = e.data.proxyRpId;
        window.__VELA_WEBAUTHN_PROXY_RPID__ = configuredRpId;
        console.log(TAG, 'config updated — rpId →', configuredRpId);
      }
      return;
    }
    if (e.data.type !== 'VELA_WEBAUTHN_RESPONSE') return;
    const { id, error, errorName, result, method } = e.data;
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (error) {
      console.error(TAG, 'proxy error:', error);
      // Preserve the original error name: collapsing everything to
      // NotAllowedError made the wallet treat real failures (e.g. "User
      // handle exceeds 64 bytes.") as a user cancellation.
      p.reject(new DOMException(error, errorName || 'NotAllowedError'));
    } else {
      const credential = method === 'create'
        ? deserializeCreateResponse(result)
        : deserializeGetResponse(result);
      console.log(TAG, method, 'proxied OK, credentialId:', credential.id);
      p.resolve(credential);
    }
  });

  // Patch navigator.credentials
  const origCreate = navigator.credentials.create.bind(navigator.credentials);
  const origGet = navigator.credentials.get.bind(navigator.credentials);

  function shouldProxy(rpId) {
    // Proxy only when the page asks for an rpId the browser itself can't satisfy
    // here — i.e. the rpId is neither the current host nor a parent domain of it.
    // On a dev/preview page that means "use the real production passkeys".
    if (!rpId) return false;
    var host = location.hostname;
    return host !== rpId && !host.endsWith('.' + rpId);
  }

  navigator.credentials.create = function (opts) {
    const rpId = opts?.publicKey?.rp?.id;
    if (!opts?.publicKey || !shouldProxy(rpId)) return origCreate(opts);
    console.log(TAG, 'intercepting credentials.create, proxying rpId →', rpId);
    const id = ++reqId;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      window.postMessage({ type: 'VELA_WEBAUTHN_REQUEST', id, method: 'create', options: serializeCreateOptions(opts) }, '*');
    });
  };

  navigator.credentials.get = function (opts) {
    const rpId = opts?.publicKey?.rpId;
    if (!opts?.publicKey || !shouldProxy(rpId)) return origGet(opts);
    console.log(TAG, 'intercepting credentials.get, proxying rpId →', rpId);
    const id = ++reqId;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      window.postMessage({ type: 'VELA_WEBAUTHN_REQUEST', id, method: 'get', options: serializeGetOptions(opts) }, '*');
    });
  };

  // Expose the proxy rpId so the app can use it for public-key uploads, etc.
  // (bridge.js may override configuredRpId shortly after, via VELA_WEBAUTHN_CONFIG.)
  window.__VELA_WEBAUTHN_PROXY_RPID__ = configuredRpId;

  console.log(TAG, 'installed (MAIN world) — WebAuthn → ' + configuredRpId);
})();
