/**
 * webauthn.js — Runs inside the extension popup window (chrome-extension://...).
 *
 * This page has the extension's origin, so navigator.credentials calls
 * with rpId "getvela.app" are allowed via host_permissions.
 *
 * Reads method + options from URL params, executes WebAuthn, sends result
 * back to background.js, which relays to content.js → inject.js → page.
 */

const PROXY_RP_ID = 'getvela.app';
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
  pk.rp = { ...pk.rp, id: PROXY_RP_ID };
  pk.challenge = b64ToBuf(pk.challenge);
  if (pk.user) pk.user = { ...pk.user, id: b64ToBuf(pk.user.id) };
  if (pk.excludeCredentials) {
    pk.excludeCredentials = pk.excludeCredentials.map(c => ({ ...c, id: b64ToBuf(c.id) }));
  }
  return { publicKey: pk };
}

function rebuildGetOptions(serialized) {
  const pk = { ...serialized.publicKey };
  pk.rpId = PROXY_RP_ID;
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

// ---- Main ----

(async () => {
  const params = new URLSearchParams(location.search);
  const method = params.get('method');
  const options = JSON.parse(decodeURIComponent(params.get('data')));

  console.log(TAG, `${method} starting, rpId: ${PROXY_RP_ID}`);

  try {
    let result;
    if (method === 'create') {
      const opts = rebuildCreateOptions(options);
      const cred = await navigator.credentials.create(opts);
      result = await serializeCreateResponse(cred);
    } else {
      const opts = rebuildGetOptions(options);
      const cred = await navigator.credentials.get(opts);
      result = serializeGetResponse(cred);
    }

    console.log(TAG, `${method} OK, credentialId:`, result.id);
    chrome.runtime.sendMessage({ type: 'VELA_WEBAUTHN_RESULT', result, method });
  } catch (err) {
    console.error(TAG, `${method} failed:`, err);
    chrome.runtime.sendMessage({ type: 'VELA_WEBAUTHN_RESULT', error: err.message });
  }
})();
