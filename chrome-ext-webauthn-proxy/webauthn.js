/**
 * webauthn.js — Runs inside the extension popup window (chrome-extension://...).
 *
 * This page has the extension's origin, so navigator.credentials calls
 * with rpId "getvela.app" are allowed via host_permissions.
 *
 * Reads method + options from URL params, executes WebAuthn, sends result
 * back to background.js, which relays to content.js → inject.js → page.
 */

const DEFAULT_RP_ID = 'getvela.app';
// Resolved in main() from (page-requested rpId → stored config → default).
let effectiveRpId = DEFAULT_RP_ID;
const TAG = '[VelaWebAuthnProxy:popup]';

// ---- Base64 ↔ ArrayBuffer ----

function b64ToBuf(b64) {
  if (!b64) return undefined;
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function bufToB64(buf) {
  if (!buf) return null;
  const bytes = new Uint8Array(buf);
  let binary = '';
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}

// ---- Rebuild native options ----

function rebuildCreateOptions(serialized) {
  const pk = { ...serialized.publicKey };
  pk.rp = { ...pk.rp, id: effectiveRpId };
  pk.challenge = b64ToBuf(pk.challenge);
  if (pk.user) pk.user = { ...pk.user, id: b64ToBuf(pk.user.id) };
  if (pk.excludeCredentials) {
    pk.excludeCredentials = pk.excludeCredentials.map(c => ({ ...c, id: b64ToBuf(c.id) }));
  }
  return { publicKey: pk };
}

function rebuildGetOptions(serialized) {
  const pk = { ...serialized.publicKey };
  pk.rpId = effectiveRpId;
  pk.challenge = b64ToBuf(pk.challenge);
  if (pk.allowCredentials) {
    pk.allowCredentials = pk.allowCredentials.map(c => ({ ...c, id: b64ToBuf(c.id) }));
  }
  return { publicKey: pk };
}

// ---- Serialize responses ----

async function serializeCreateResponse(cred) {
  const resp = cred.response;
  return {
    id: cred.id,
    rawId: bufToB64(cred.rawId),
    type: cred.type,
    authenticatorAttachment: cred.authenticatorAttachment,
    response: {
      clientDataJSON: bufToB64(resp.clientDataJSON),
      attestationObject: bufToB64(resp.attestationObject),
      transports: typeof resp.getTransports === 'function' ? resp.getTransports() : [],
      publicKey: typeof resp.getPublicKey === 'function' ? bufToB64(resp.getPublicKey()) : null,
      publicKeyAlgorithm: typeof resp.getPublicKeyAlgorithm === 'function' ? resp.getPublicKeyAlgorithm() : null,
      authenticatorData: typeof resp.getAuthenticatorData === 'function' ? bufToB64(resp.getAuthenticatorData()) : null,
    },
    clientExtensionResults: cred.getClientExtensionResults(),
  };
}

function serializeGetResponse(cred) {
  const resp = cred.response;
  return {
    id: cred.id,
    rawId: bufToB64(cred.rawId),
    type: cred.type,
    authenticatorAttachment: cred.authenticatorAttachment,
    response: {
      clientDataJSON: bufToB64(resp.clientDataJSON),
      authenticatorData: bufToB64(resp.authenticatorData),
      signature: bufToB64(resp.signature),
      userHandle: resp.userHandle ? bufToB64(resp.userHandle) : null,
    },
    clientExtensionResults: cred.getClientExtensionResults(),
  };
}

// ---- Host-access UI ----

const rpOrigin = (domain) => `*://${domain}/*`;

function showPanel(id) {
  for (const el of document.querySelectorAll('.panel')) {
    el.classList.toggle('hidden', el.id !== id);
  }
}

async function hasAccess(domain) {
  try {
    return await chrome.permissions.contains({ origins: [rpOrigin(domain)] });
  } catch {
    return false;
  }
}

/** Show the grant panel; resolve true/false once the user decides. */
function promptForAccess(domain) {
  return new Promise((resolve) => {
    document.getElementById('perm-domain').textContent = domain;
    showPanel('permission');
    document.getElementById('grant').addEventListener('click', async () => {
      // permissions.request must run in a user gesture — this click qualifies.
      const granted = await chrome.permissions.request({ origins: [rpOrigin(domain)] });
      resolve(granted);
    }, { once: true });
    document.getElementById('cancel').addEventListener('click', () => resolve(false), { once: true });
  });
}

async function runWebAuthn(method, options) {
  if (method === 'create') {
    const cred = await navigator.credentials.create(rebuildCreateOptions(options));
    return serializeCreateResponse(cred);
  }
  const cred = await navigator.credentials.get(rebuildGetOptions(options));
  return serializeGetResponse(cred);
}

const looksLikeAccessError = (msg) =>
  /HTTPS origins|pages served from an extension/i.test(msg || '');

// ---- Main ----

(async () => {
  const params = new URLSearchParams(location.search);
  const method = params.get('method');
  const options = JSON.parse(decodeURIComponent(params.get('data')));

  // Prefer the rpId the page actually requested; fall back to the user's
  // configured domain, then the default. The extension must hold host
  // permission for whichever domain this resolves to.
  const pageRpId = options?.publicKey?.rpId || options?.publicKey?.rp?.id;
  try {
    const cfg = await chrome.storage.sync.get({ proxyRpId: '' });
    effectiveRpId = pageRpId || cfg.proxyRpId || DEFAULT_RP_ID;
  } catch {
    effectiveRpId = pageRpId || DEFAULT_RP_ID;
  }

  console.log(TAG, `${method} starting, rpId: ${effectiveRpId}`);

  // Chrome can silently reset/withhold the extension's host access (e.g. after a
  // browser update). Detect it up front and let the user re-grant in one click,
  // right here, instead of failing with a cryptic "only available to HTTPS
  // origins…" error that they'd have to fix by digging into chrome://extensions.
  if (!(await hasAccess(effectiveRpId))) {
    console.warn(TAG, `no host access for ${effectiveRpId}; prompting user`);
    const granted = await promptForAccess(effectiveRpId);
    if (!granted) {
      chrome.runtime.sendMessage({
        type: 'VELA_WEBAUTHN_RESULT',
        error: `Passkey access to ${effectiveRpId} was not granted.`,
      });
      return;
    }
  }

  showPanel('loading');

  try {
    let result = await runWebAuthn(method, options);
    console.log(TAG, `${method} OK, credentialId:`, result.id);
    chrome.runtime.sendMessage({ type: 'VELA_WEBAUTHN_RESULT', result, method });
  } catch (err) {
    // Belt-and-suspenders: if it still failed on the origin gate, host access is
    // the likely culprit — offer the grant flow once more, then retry.
    if (looksLikeAccessError(err.message) && (await promptForAccess(effectiveRpId))) {
      showPanel('loading');
      try {
        const result = await runWebAuthn(method, options);
        chrome.runtime.sendMessage({ type: 'VELA_WEBAUTHN_RESULT', result, method });
        return;
      } catch (retryErr) {
        err = retryErr;
      }
    }
    console.error(TAG, `${method} failed:`, err);
    chrome.runtime.sendMessage({ type: 'VELA_WEBAUTHN_RESULT', error: err.message });
  }
})();
