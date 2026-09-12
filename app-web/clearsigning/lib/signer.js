// Signing with a passkey — assertions only.
//
// The challenge is always a digest this page derived itself (lib/digest.js),
// never bytes handed over by the requester. And there is no way to create a
// key from here: see lib/enrol.js.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var RP_ID = 'getvela.app';

  function isExtension() {
    return typeof chrome !== 'undefined' && !!chrome.runtime && !!chrome.runtime.id &&
      location.protocol === 'chrome-extension:';
  }

  // Same rule as the demo page: in the extension the relying party MUST be
  // hardcoded, because there location.hostname is the extension id — using it
  // mints a valid passkey that no other surface recognises.
  function relyingPartyId() {
    if (isExtension()) return RP_ID;
    var host = location.hostname;
    if (host === RP_ID || host.endsWith('.' + RP_ID)) return RP_ID;
    return host;
  }

  function b64url(bytes) {
    var binary = '';
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  function unb64url(text) {
    var padded = text.replace(/-/g, '+').replace(/_/g, '/');
    while (padded.length % 4) padded += '=';
    var binary = atob(padded);
    var out = new Uint8Array(binary.length);
    for (var i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  }

  function hex(bytes) {
    var s = '';
    for (var i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return '0x' + s;
  }

  // WebAuthn returns DER; on-chain P-256 verifiers want raw r‖s.
  function derToRaw(der) {
    if (der[0] !== 0x30) throw new Error('signature is not a DER sequence');
    var offset = 2;
    if (der[1] & 0x80) offset = 2 + (der[1] & 0x7f);
    var out = new Uint8Array(64);
    for (var half = 0; half < 2; half++) {
      if (der[offset] !== 0x02) throw new Error('malformed DER integer');
      var length = der[offset + 1];
      var value = der.subarray(offset + 2, offset + 2 + length);
      while (value.length > 32 && value[0] === 0x00) value = value.subarray(1);
      out.set(value, half * 32 + (32 - value.length));
      offset = offset + 2 + length;
    }
    return out;
  }

  function storeKey() {
    return 'clearsigning.passkey.' + relyingPartyId();
  }

  function stored() {
    try {
      var raw = localStorage.getItem(storeKey());
      return raw ? JSON.parse(raw) : null;
    } catch (e) {
      return null;
    }
  }

  function remember(record) {
    try {
      localStorage.setItem(storeKey(), JSON.stringify(record));
    } catch (e) { /* storage can be blocked; the ceremony still works */ }
  }

  /**
   * Sign a digest we derived.
   *
   * This function NEVER creates a key. Creating one mints a NEW ACCOUNT —
   * address = f(all public keys) — so a key made here could not possibly own
   * the account being signed for, and the signature would be worthless. Worse,
   * it would teach people to accept an enrolment prompt in the middle of a
   * signing flow, which is exactly the confusion an attacker wants.
   *
   * `allowCredentials` comes from the REQUEST: the account's own key set. With
   * it the authenticator picks the right key; without it we ask for any
   * discoverable credential of this relying party and let the caller check
   * which one answered. Local storage is never consulted for this — a leftover
   * id in this browser says nothing about which account is being signed for.
   */
  function sign(digest, options) {
    options = options || {};
    var request = {
      challenge: digest,
      rpId: relyingPartyId(),
      userVerification: 'required',
      timeout: 60000,
    };
    if (options.allowCredentials && options.allowCredentials.length) {
      request.allowCredentials = options.allowCredentials.map(function (id) {
        return { type: 'public-key', id: unb64url(id) };
      });
    }

    return navigator.credentials.get({ publicKey: request }).then(function (assertion) {
      var authenticatorData = new Uint8Array(assertion.response.authenticatorData);
      var clientDataJSON = new Uint8Array(assertion.response.clientDataJSON);
      var credentialId = b64url(new Uint8Array(assertion.rawId));
      var record = stored();
      return {
        // Taken from the assertion, not from storage: this is the key that
        // actually answered.
        credentialId: credentialId,
        publicKey: record && record.credentialId === credentialId ? record.publicKey : null,
        signature: hex(derToRaw(new Uint8Array(assertion.response.signature))),
        authenticatorData: hex(authenticatorData),
        clientDataJSON: hex(clientDataJSON),
        userVerified: (authenticatorData[32] & 0x04) !== 0,
      };
    });
  }

  /**
   * The name THIS DEVICE recorded when one of these keys was enrolled, if any.
   * Used to help someone pick the right passkey — and pointedly not sourced
   * from the request, which could name the account anything it liked.
   */
  function knownName(credentialIds) {
    var record = stored();
    if (!record || !record.name) return null;
    if (!credentialIds || !credentialIds.length) return record.name;
    return credentialIds.indexOf(record.credentialId) >= 0 ? record.name : null;
  }

  ns.signer = {
    knownName: knownName,
    RP_ID: RP_ID,
    relyingPartyId: relyingPartyId,
    isExtension: isExtension,
    sign: sign,
    // Deliberately absent: any way to CREATE a key. Enrolment lives in
    // lib/enrol.js, which the signing page does not load, so "signing never
    // creates" is a property of the build rather than of anyone's memory.
    _derToRaw: derToRaw,
    _b64url: b64url,
    _unb64url: unb64url,
    _remember: remember,
    _stored: stored,
  };
})(window.VelaCS);
