// Classic script on purpose: works over http(s), chrome-extension:// and file://.
// No inline script anywhere — MV3 forbids it, and a CSP hash would make the
// extension refuse to install outright.
(function () {
  'use strict';

  // --- which shell are we in -------------------------------------------------

  var isExtension =
    typeof chrome !== 'undefined' &&
    !!chrome.runtime &&
    !!chrome.runtime.id &&
    location.protocol === 'chrome-extension:';

  var isFile = location.protocol === 'file:';
  var surface = isExtension ? 'Chrome extension' : isFile ? 'Local file' : 'Web page';

  // The relying party is decided HERE and nowhere else.
  //
  // On the web it is simply the hostname, so localhost and preview deploys work
  // with no extra machinery — except that every getvela.app subdomain folds up
  // to the bare domain, so the whole family shares one passkey.
  //
  // In the extension it MUST be hardcoded. There, location.hostname is the
  // extension id, and using it never throws: it mints a perfectly valid passkey
  // that no other surface on earth recognises, so the user silently lands in a
  // different, empty wallet. The manifest's host_permissions entry is what lets
  // this page claim the real domain instead.
  var VELA_RP_ID = 'getvela.app';

  function relyingPartyId() {
    if (isExtension) return VELA_RP_ID;
    var host = location.hostname;
    if (host === VELA_RP_ID || host.endsWith('.' + VELA_RP_ID)) return VELA_RP_ID;
    return host; // '' under file:// — no origin, no passkeys
  }

  var rpId = relyingPartyId();

  // --- tiny helpers, no dependencies ----------------------------------------

  function $(id) {
    return document.getElementById(id);
  }

  function set(id, text) {
    var el = $(id);
    if (el) el.textContent = text;
  }

  function utf8(text) {
    return new TextEncoder().encode(text);
  }

  function bytes(buffer) {
    return new Uint8Array(buffer);
  }

  function concat(a, b) {
    var out = new Uint8Array(a.length + b.length);
    out.set(a, 0);
    out.set(b, a.length);
    return out;
  }

  function equal(a, b) {
    if (a.length !== b.length) return false;
    var diff = 0;
    for (var i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
    return diff === 0;
  }

  function b64url(data) {
    var binary = '';
    for (var i = 0; i < data.length; i++) binary += String.fromCharCode(data[i]);
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

  function random(n) {
    return crypto.getRandomValues(new Uint8Array(n));
  }

  function sha256(data) {
    return crypto.subtle.digest('SHA-256', data).then(bytes);
  }

  function shorten(text) {
    return text.length > 22 ? text.slice(0, 10) + '…' + text.slice(-8) : text;
  }

  // WebAuthn hands back a DER-encoded ECDSA signature; WebCrypto verifies the
  // raw r‖s form. Unpack SEQUENCE { INTEGER r, INTEGER s } and left-pad both.
  function derToRaw(der) {
    if (der[0] !== 0x30) throw new Error('signature is not a DER sequence');
    var offset = 2;
    if (der[1] & 0x80) offset = 2 + (der[1] & 0x7f);
    var out = new Uint8Array(64);
    for (var half = 0; half < 2; half++) {
      if (der[offset] !== 0x02) throw new Error('malformed DER integer');
      var length = der[offset + 1];
      var start = offset + 2;
      var value = der.subarray(start, start + length);
      while (value.length > 32 && value[0] === 0x00) value = value.subarray(1);
      if (value.length > 32) throw new Error('DER integer too wide for P-256');
      out.set(value, half * 32 + (32 - value.length));
      offset = start + length;
    }
    return out;
  }

  // --- stored passkey handle -------------------------------------------------

  var STORE_KEY = 'clearsigning.passkey.' + (rpId || 'unknown');

  function loadPasskey() {
    try {
      var raw = localStorage.getItem(STORE_KEY);
      return raw ? JSON.parse(raw) : null;
    } catch (e) {
      return null; // storage can be blocked; the ceremony still works
    }
  }

  function savePasskey(record) {
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(record));
    } catch (e) {
      /* ignore */
    }
  }

  var passkey = loadPasskey();

  // --- the bytes we are going to sign ---------------------------------------

  var payload =
    'Vela Clear Signing — hello, world\n' +
    'Relying party: ' + (rpId || '(none)') + '\n' +
    'Surface: ' + surface + '\n' +
    'Opened: ' + new Date().toISOString();

  // --- wiring ----------------------------------------------------------------

  var createButton = $('create');
  var signButton = $('sign');
  var checks = $('checks');

  set('runtime-badge', isExtension ? 'Extension' : isFile ? 'File' : 'Web');
  set('fact-surface', surface);
  set('fact-origin', location.origin === 'null' ? location.protocol : location.origin);
  set('fact-rpid', rpId || '— none under file://');
  set('payload', payload);
  refreshPasskeyFact();

  function refreshPasskeyFact() {
    set('fact-credential', passkey ? shorten(passkey.credentialId) : 'none yet');
    if (signButton) signButton.disabled = !passkey;
  }

  function say(text) {
    set('status', text);
  }

  function busy(on) {
    if (createButton) createButton.disabled = on;
    if (signButton) signButton.disabled = on || !passkey;
  }

  function explain(error) {
    if (error && error.name === 'NotAllowedError') {
      return 'Ceremony cancelled or timed out.';
    }
    if (error && error.name === 'SecurityError') {
      return 'the browser refused rpId "' + rpId + '" from ' + location.origin + '.';
    }
    return (error && (error.name + ': ' + error.message)) || String(error);
  }

  var supported =
    typeof window.PublicKeyCredential === 'function' &&
    !!(navigator.credentials && navigator.credentials.create) &&
    !!(crypto && crypto.subtle) &&
    rpId !== '';

  if (!supported) {
    if (createButton) createButton.disabled = true;
    if (signButton) signButton.disabled = true;
    say(
      isFile
        ? 'Passkeys need a real origin. Serve this folder over http://localhost, or load it as the extension.'
        : 'This browser exposes no WebAuthn authenticator.',
    );
  } else {
    say(passkey ? 'Passkey ready. Sign the message above.' : 'No passkey yet. Create one to sign.');
  }

  // --- create: a real navigator.credentials.create ---------------------------

  if (createButton) {
    createButton.addEventListener('click', function () {
      busy(true);
      say('Waiting for the authenticator…');

      navigator.credentials
        .create({
          publicKey: {
            rp: { id: rpId, name: 'Vela Clear Signing' },
            user: {
              id: random(16),
              name: 'hello@' + rpId,
              displayName: 'Vela Clear Signing',
            },
            challenge: random(32),
            // ES256 only: the same P-256 curve the wallet verifies on-chain.
            pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
            authenticatorSelection: {
              // Discoverable, or the key cannot be found again from a fresh
              // install — that was issue #1.
              residentKey: 'required',
              requireResidentKey: true,
              userVerification: 'required',
            },
            attestation: 'none',
            timeout: 60000,
          },
        })
        .then(function (credential) {
          var spki = credential.response.getPublicKey && credential.response.getPublicKey();
          if (!spki) throw new Error('authenticator returned no P-256 public key');
          passkey = {
            credentialId: b64url(bytes(credential.rawId)),
            publicKey: b64url(bytes(spki)),
            rpId: rpId,
            created: new Date().toISOString(),
          };
          savePasskey(passkey);
          refreshPasskeyFact();
          say('Passkey created on ' + rpId + '. Now sign the message.');
          if (checks) checks.hidden = true;
        })
        .catch(function (error) {
          say('Create failed — ' + explain(error));
        })
        .then(function () {
          busy(false);
        });
    });
  }

  // --- sign: a real navigator.credentials.get, verified locally --------------

  if (signButton) {
    signButton.addEventListener('click', function () {
      if (!passkey) return;
      busy(true);
      say('Waiting for the authenticator…');

      var challenge;

      sha256(utf8(payload))
        .then(function (digest) {
          challenge = digest;
          return navigator.credentials.get({
            publicKey: {
              challenge: challenge,
              rpId: rpId,
              allowCredentials: [
                { type: 'public-key', id: unb64url(passkey.credentialId) },
              ],
              userVerification: 'required',
              timeout: 60000,
            },
          });
        })
        .then(function (assertion) {
          return verify(assertion, challenge);
        })
        .then(function (result) {
          if (checks) checks.hidden = false;
          set('check-signature', result.signature ? 'valid over P-256' : 'INVALID');
          set('check-rpid', result.rpIdHash ? 'matches ' + rpId : 'MISMATCH');
          set('check-uv', result.userVerified ? 'yes' : 'NO');
          set('check-challenge', result.challenge ? 'exactly our bytes' : 'ALTERED');
          var allGood =
            result.signature && result.rpIdHash && result.userVerified && result.challenge;
          say(allGood ? 'Signed and verified against the stored public key.' : 'Signature rejected.');
        })
        .catch(function (error) {
          say('Sign failed — ' + explain(error));
        })
        .then(function () {
          busy(false);
        });
    });
  }

  // Verify the assertion the same way vela-core does: the signature covers
  // authenticatorData ‖ sha256(clientDataJSON), and the UV bit must be set.
  function verify(assertion, challenge) {
    var authData = bytes(assertion.response.authenticatorData);
    var clientDataJSON = bytes(assertion.response.clientDataJSON);
    var signature = derToRaw(bytes(assertion.response.signature));
    var flags = authData[32];

    var clientData = JSON.parse(new TextDecoder().decode(clientDataJSON));
    var challengeEchoed =
      clientData.type === 'webauthn.get' && clientData.challenge === b64url(challenge);

    return Promise.all([
      sha256(clientDataJSON),
      sha256(utf8(rpId)),
      crypto.subtle.importKey(
        'spki',
        unb64url(passkey.publicKey),
        { name: 'ECDSA', namedCurve: 'P-256' },
        false,
        ['verify'],
      ),
    ]).then(function (parts) {
      var clientDataHash = parts[0];
      var expectedRpIdHash = parts[1];
      var key = parts[2];
      return crypto.subtle
        .verify(
          { name: 'ECDSA', hash: 'SHA-256' },
          key,
          signature,
          concat(authData, clientDataHash),
        )
        .then(function (ok) {
          return {
            signature: ok,
            rpIdHash: equal(authData.subarray(0, 32), expectedRpIdHash),
            userVerified: (flags & 0x04) !== 0,
            challenge: challengeEchoed,
          };
        });
    });
  }
})();
