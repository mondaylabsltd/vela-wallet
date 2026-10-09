// Key ceremonies: create a key, sign in, prove a key — the page as a passkey
// route of its own (spec 075), beside "this device", "a phone or tablet" and
// "a security key".
//
// The founding rule holds here as it does for signing (lib/digest.js):
// **the page signs only what it derived itself.** A requester never hands over
// the challenge. Each kind has its own, made here, in a form no transaction
// hash can take:
//
//   sign-in       UTF-8 "vela-signin-<ms>-<16 hex>"      (made here: clock + 8 random bytes)
//   proof         UTF-8 "vela-verify-<ms>" / "vela-recover-<ms>"   (the shells' own form)
//   member proof  keccak256(abi.encode(chainId, registry, rpId, publicKey,
//                   keccak256(abi.encode(groupPublicKey, attestation))))
//                 — computed HERE from the inputs on screen; the page asks
//                 no registry anything (it reaches no network, 076).
//
// The first two are text, never 32 bytes. The third IS 32 bytes, but it is a
// keccak over a fixed ABI layout of what the card shows: nobody can choose
// inputs that make it equal a SafeOp or an EIP-191 hash.
//
// Creating a key lives here (it used to be lib/enrol.js). What keeps a create
// out of a signing flow is the decision in resolve.js: only a
// `vela_createPasskey` request from a Vela wallet reaches `create()`, as its
// own request, with its own card — never inside a signature. Since spec 102
// the apps run these on the page only for a wallet whose keys live on the
// page's own domain (R3); the request says where the key is or will be
// (`place`, `hints`, and for a named key its `transports`), so the browser
// goes straight there.
//
// No words here, and no decisions about WHETHER a request may run: that is
// resolve.js. This file derives and performs.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var METHODS = {
    vela_createPasskey: 'create',
    vela_signIn: 'signIn',
    vela_proof: 'proof',
    vela_memberProof: 'memberProof',
  };

  var PURPOSES = { verify: 'vela-verify-', recover_first: 'vela-recover-', recover_second: 'vela-recover-' };

  var DEFAULT_REGISTRY = 'https://p256-index-v2.getvela.app';
  var CEREMONY_TIMEOUT = 120000;

  // Where a new key is made, by the place the person chose (spec 102 R5):
  // this device's own authenticator, or another one — a phone over the
  // browser's QR code, or a security key.
  var ATTACHMENT = { platform: 'platform', hybrid: 'cross-platform', security_key: 'cross-platform' };

  // --- bytes -----------------------------------------------------------------

  function utf8(text) {
    return new TextEncoder().encode(text);
  }

  function hex(bytes) {
    var s = '';
    for (var i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return s;
  }

  function fromHex(text) {
    var body = String(text || '').replace(/^0x/i, '');
    if (body.length % 2 || /[^0-9a-fA-F]/.test(body)) return null;
    var out = new Uint8Array(body.length / 2);
    for (var i = 0; i < out.length; i++) out[i] = parseInt(body.substr(i * 2, 2), 16);
    return out;
  }

  function b64url(bytes) {
    return ns.signer._b64url(bytes);
  }

  function unb64url(text) {
    return ns.signer._unb64url(text);
  }

  function kindOf(method) {
    return Object.prototype.hasOwnProperty.call(METHODS, method) ? METHODS[method] : null;
  }

  // --- the challenges this page derives ----------------------------------------

  /** "vela-signin-<ms>-<16 hex>". `now` and `random` (8 bytes) are for tests. */
  function signInChallenge(now, random) {
    var noise = random || crypto.getRandomValues(new Uint8Array(8));
    return 'vela-signin-' + (now === undefined ? Date.now() : now) + '-' + hex(noise);
  }

  /** "vela-verify-<ms>" or "vela-recover-<ms>"; null for an unknown purpose. */
  function proofChallenge(purpose, now) {
    if (!Object.prototype.hasOwnProperty.call(PURPOSES, purpose)) return null;
    return PURPOSES[purpose] + (now === undefined ? Date.now() : now);
  }

  /** keccak256(abi.encode(bytes groupPublicKey, bytes attestation)) — the contract's memberBindingFor. */
  function memberBinding(groupPublicKeyHex, attestationHex) {
    var encoded = ns.encode.params(['bytes', 'bytes'], [
      '0x' + String(groupPublicKeyHex).replace(/^0x/i, ''),
      '0x' + String(attestationHex || '').replace(/^0x/i, ''),
    ]);
    return ns.keccak.hash(fromHex(encoded));
  }

  /**
   * keccak256(abi.encode(uint256 chainId, address registry, string rpId,
   * bytes publicKey, bytes32 binding)) — the registry contract's challenge,
   * as `challenge_for` in p256-registrar computes it.
   */
  function memberChallenge(facts) {
    var encoded = ns.encode.params(['uint256', 'address', 'string', 'bytes', 'bytes32'], [
      BigInt(facts.chainId),
      facts.registry,
      facts.rpId,
      '0x' + String(facts.publicKey).replace(/^0x/i, ''),
      '0x' + hex(facts.binding),
    ]);
    return ns.keccak.hash(fromHex(encoded));
  }

  /**
   * The member challenge, computed HERE from the facts on screen.
   *
   * This replaces a pair of fetches to the registry (`/api/health` for the
   * deployment, `/api/challenge` for its answer). The published page cannot
   * make either: `default-src 'none'` is inside its hashed bytes (spec 076), so
   * a page whose hash a wallet accepts reaches no network — and creating a
   * wallet therefore failed at its second step with 「注册表没有应答」 (owner,
   * 2026-09-24).
   *
   * Nothing is weakened by where the facts arrive from, because the challenge
   * was never taken from the answer: the page computed it and refused anything
   * that did not match. Now it simply computes it. A requester that lies about
   * `chainId` or `registryContract` gets a challenge the WALLET did not ask for,
   * and the wallet refuses the assertion (`expected_member_challenge`).
   *
   * What it gives up: the registry is no longer asked whether it agrees. It was
   * never trusted for the value, only consulted — and a consultation that can
   * only ever be refused for disagreeing is not a check the page needs.
   */
  function memberChallengeFor(inputs, rpId) {
    var binding = memberBinding(inputs.groupPublicKey, inputs.attestation);
    var challenge = memberChallenge({
      chainId: inputs.chainId,
      registry: inputs.registryContract,
      rpId: rpId,
      publicKey: inputs.publicKey,
      binding: binding,
    });
    return { challenge: challenge, chainId: inputs.chainId, registry: inputs.registryContract };
  }

  // --- WebAuthn ------------------------------------------------------------------

  // `name\0<uuid v4>` as UTF-8, the user handle every Vela shell writes: the
  // core reads the wallet's name back out of it at sign-in (Assertion::
  // user_name). WebAuthn caps user.id at 64 bytes, 37 of which the uuid takes;
  // a name that does not fit gets 16 random bytes instead, and the wallet
  // simply finds no name in the handle.
  function userHandle(name) {
    var bytes = utf8(name);
    var uuid = typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : null;
    if (uuid && bytes.length > 0 && bytes.length <= 64 - 37) {
      return utf8(name + '\u0000' + uuid);
    }
    return crypto.getRandomValues(new Uint8Array(16));
  }

  /**
   * Create a key. ES256 only, discoverable, user verification required —
   * the same options as the web wallet's own create (`attestation: 'direct'`
   * keeps the authenticator's AAGUID for the registry's attestation bytes).
   * The challenge is 32 random bytes made here: a registration proves nothing
   * about a challenge, so nobody else's bytes have any business in it.
   *
   * `options.key` = resolve's `{place, hints}` for this request: the
   * authenticator attachment and the WebAuthn L3 hints, so the browser asks
   * for the place the person already chose rather than offering all three.
   *
   * Resolves with the answer's payload: `{registration, origin}`.
   */
  function create(options) {
    options = options || {};
    var key = options.key || null;
    var name = options.name || 'Vela';
    var rpId = ns.signer.relyingPartyId();
    var publicKey = {
      rp: { id: rpId, name: 'Vela Wallet' },
      user: { id: userHandle(name), name: name, displayName: name },
      challenge: crypto.getRandomValues(new Uint8Array(32)),
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      authenticatorSelection: {
        residentKey: 'required',
        requireResidentKey: true,
        userVerification: 'required',
      },
      attestation: 'direct',
      extensions: { credProps: true },
      timeout: CEREMONY_TIMEOUT,
    };
    if (key && ATTACHMENT[key.place]) {
      publicKey.authenticatorSelection.authenticatorAttachment = ATTACHMENT[key.place];
    }
    ns.signer.withHints(publicKey, key);
    var exclude = (options.excludeCredentialIds || []).map(unb64url);
    if (exclude.length) {
      publicKey.excludeCredentials = exclude.map(function (id) { return { type: 'public-key', id: id }; });
    }
    return navigator.credentials.create({ publicKey: publicKey }).then(function (credential) {
      var response = credential.response;
      var algorithm = response.getPublicKeyAlgorithm ? response.getPublicKeyAlgorithm() : -7;
      if (algorithm !== -7) throw new Error('the authenticator made a key that is not P-256');
      var credentialId = b64url(new Uint8Array(credential.rawId));
      var spki = response.getPublicKey ? response.getPublicKey() : null;
      // This device's own record of the key, so a later signing sheet can
      // name the account from here rather than from the request.
      ns.signer._remember({
        credentialId: credentialId,
        publicKey: spki ? b64url(new Uint8Array(spki)) : null,
        rpId: rpId,
        name: name,
        created: new Date().toISOString(),
      });
      return {
        registration: {
          credentialId: credentialId,
          attestationObject: hex(new Uint8Array(response.attestationObject)),
          clientDataJSON: hex(new Uint8Array(response.clientDataJSON)),
          authenticatorAttachment: credential.authenticatorAttachment || '',
          transports: (response.getTransports ? response.getTransports() : []).join(','),
        },
        origin: location.origin,
      };
    });
  }

  /**
   * An assertion over a challenge this page derived. `allow` = base64url ids
   * (empty: any discoverable key of this relying party, i.e. sign-in). When a
   * key was named, an answer from another key is discarded, not returned.
   * `key` = resolve's route for this request (place, hints, and the named
   * key's transports), as for a signature (`ns.signer.credentials`).
   *
   * Resolves with the answer's payload: `{assertion, origin}`.
   */
  function assert(challenge, allow, key) {
    allow = allow || [];
    var request = {
      challenge: challenge,
      rpId: ns.signer.relyingPartyId(),
      userVerification: 'required',
      timeout: CEREMONY_TIMEOUT,
    };
    if (allow.length) request.allowCredentials = ns.signer.credentials(allow, key);
    ns.signer.withHints(request, key);
    return navigator.credentials.get({ publicKey: request }).then(function (credential) {
      var response = credential.response;
      var credentialId = b64url(new Uint8Array(credential.rawId));
      if (allow.length && allow.indexOf(credentialId) < 0) {
        var wrong = new Error('wrong credential');
        wrong.wrongCredential = true;
        throw wrong;
      }
      return {
        assertion: {
          credentialId: credentialId,
          signatureDer: hex(new Uint8Array(response.signature)),
          authenticatorData: hex(new Uint8Array(response.authenticatorData)),
          clientDataJSON: hex(new Uint8Array(response.clientDataJSON)),
          userHandle: response.userHandle && response.userHandle.byteLength
            ? hex(new Uint8Array(response.userHandle))
            : null,
          authenticatorAttachment: credential.authenticatorAttachment || '',
        },
        origin: location.origin,
      };
    });
  }

  ns.ceremony = {
    METHODS: METHODS,
    DEFAULT_REGISTRY: DEFAULT_REGISTRY,
    kindOf: kindOf,
    signInChallenge: signInChallenge,
    proofChallenge: proofChallenge,
    memberBinding: memberBinding,
    memberChallenge: memberChallenge,
    memberChallengeFor: memberChallengeFor,
    create: create,
    assert: assert,
    _utf8: utf8,
    _hex: hex,
    _fromHex: fromHex,
  };
})(window.VelaCS);
