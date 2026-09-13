// Enrolment — creating a passkey. A separate file on purpose.
//
// Creating a key mints a NEW ACCOUNT (the address is derived from the whole key
// set), so it can never be a step inside a signing flow: a key made during
// signing cannot own the account being signed for, and the prompt would train
// people to accept enrolment when they meant to approve a payment.
//
// The signing page does not load this file.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  function create(options) {
    options = options || {};
    var name = options.name || 'Vela';
    var rpId = ns.signer.relyingPartyId();
    return navigator.credentials.create({
      publicKey: {
        rp: { id: rpId, name: 'Vela Clear Signing' },
        user: {
          id: crypto.getRandomValues(new Uint8Array(16)),
          name: name + '@' + rpId,
          displayName: name,
        },
        challenge: crypto.getRandomValues(new Uint8Array(32)),
        pubKeyCredParams: [{ type: 'public-key', alg: -7 }], // ES256 / P-256
        authenticatorSelection: {
          residentKey: 'required',
          requireResidentKey: true,
          userVerification: 'required',
        },
        attestation: 'none',
        timeout: 60000,
      },
    }).then(function (credential) {
      var spki = credential.response.getPublicKey && credential.response.getPublicKey();
      if (!spki) throw new Error('authenticator returned no P-256 public key');
      var record = {
        credentialId: ns.signer._b64url(new Uint8Array(credential.rawId)),
        publicKey: ns.signer._b64url(new Uint8Array(spki)),
        rpId: rpId,
        name: name,
        created: new Date().toISOString(),
      };
      ns.signer._remember(record);
      return record;
    });
  }

  ns.enrol = { create: create };
})(window.VelaCS);
