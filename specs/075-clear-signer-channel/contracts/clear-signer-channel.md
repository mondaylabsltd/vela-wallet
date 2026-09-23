# Contract — the Clear Signer as a passkey route

Extends `specs/071-clear-signer/contracts/clear-signer.md`; everything there stands unless
changed here.

> **Narrowed 2026-09-23.** The cross-device channels — the tunnel and BLE — were cut, with
> the web shell's Clear Signer. `tunnel.md` is gone; PROTOCOL.md's BLE sections are marked
> historical. See spec.md, "What the owner cut".
§1–4.

## 1. The core

### 1.1 The route
- `KeyMethod::ClearSigner` (`"clear_signer"`) joins `Platform` / `Hybrid` / `SecurityKey`.
- It rides on every passkey operation that already carries a `method`:
  - `RegisterPasskey`
  - `AuthenticatePasskey`
  - `SignProof`
  - `SignMemberProof`
- The create and sign-in machines offer it wherever they offer the other three; every
  shell's choosers list it with `componentsUi.signing.clearSignerTitle` and its line
  `clearSignerBody`.

### 1.2 Where a key lives
A key minted or found through the Clear Signer records `signer_origin` (e.g.
`https://sign.getvela.app`). Its rpId is the origin's host, folded to `getvela.app` for any
`*.getvela.app`.

Routing a signature (`auto`) follows the key:
- a key with `method = clear_signer` goes to the Clear Signer at its `signer_origin`;
- any other key goes to its own route, as today.

Choosing "清晰签名器" by hand for a platform key is allowed only when
`uses_wallet_passkeys(signer page)` (071's rule). Choosing another route for a Clear Signer
key whose origin is not `getvela.app` is refused before anything opens: that key is
reachable only through that page, the way a security key's key is only in that key.

### 1.3 Requests the core builds (UniFFI `clearSignerCeremony…`, wasm, desktop direct)
Every request is `{id, intent: {method, params, origin: ""}, context}` — the 071 shape — so
channels need no new envelope.

| Operation | `intent.method` | `params[0]` | The page derives |
|---|---|---|---|
| `RegisterPasskey` | `vela_createPasskey` | `{name, excludeCredentialIds}` | a random 32-byte challenge; `user.id` random 16 bytes; ES256, resident, UV required |
| `AuthenticatePasskey` | `vela_signIn` | `{}` | challenge = UTF-8 `vela-signin-<ms>-<16 hex>`; discoverable (empty allow list) |
| `SignProof` | `vela_proof` | `{credentialId, purpose: verify\|recover_first\|recover_second}` | challenge = UTF-8 `vela-verify-<ms>` / `vela-recover-<ms>` (the shells' own form) |
| `SignMemberProof` | `vela_memberProof` | `{credentialId, publicKey, attestation, groupPublicKey, registry}` | challenge = `POST {registry}/api/challenge {rpId, groupPublicKey, publicKey, attestation?}` — the page's own fetch, shown as "confirm this key joins 〈wallet〉" |
| signing (071) | the dApp's / `wallet_sendCalls` | unchanged | the digest (071) |

`context` carries `{walletName}` (and 071's fields for signing). The page never signs a
challenge a requester supplied: none of these challenges can be 32 bytes that a Safe would
accept as an operation hash, except the member challenge, which the page fetches for inputs
it displays.

### 1.4 Answers the core verifies
Answers are `{v:1, t:"result", n, id, …}` over the socket channels, and
`{vela:'result', id, …}` over postMessage:
- **create** → `{registration: {credentialId, attestationObject, clientDataJSON, authenticatorAttachment, transports}, origin}`
- **ceremonies** → `{assertion: {credentialId, signatureDer, authenticatorData, clientDataJSON, userHandle}, origin}`

`clear_signer::verify_registration(json, signer_origin) -> Registration` checks:
- clientDataJSON `type = webauthn.create`;
- origin = `signer_origin`;
- the attestation parses to a P-256 key.

`clear_signer::verify_ceremony(json, kind, signer_origin, expected) -> Assertion` checks:
- `type = webauthn.get`;
- origin;
- the challenge's form for `kind` (the prefix, and never 32 bytes; for a member proof,
  equality with the challenge the WALLET fetched for the same inputs);
- the credential matches when one was named.

Refusals reuse `ClearSignerRefusal` (`Malformed` / `WrongChallenge` / `ForeignKey` /
`NotVerified`). The verified values go on as `PasskeyRegistered` / `PasskeyAuthenticated`
/ `ProofSigned` / `MemberProofSigned`, exactly as a platform ceremony's do.

### 1.5 Sessions
`ws::Connection` and the page's intake carry **several requests in order** in one session
(create → member proof; recover first → second): after an answer, the connection stays
open for the next `intent` until a `bye` or the session timeout (5 min idle). A shell opens
the page once per flow, not once per ceremony.

## 2. Channels

| Wallet | Signer page | Channel |
|---|---|---|
| Android / iOS / desktop | same device | loopback WebSocket (071 §2). The desktop moves off fragment + callback. |
| Web | same browser | postMessage (071 §4) |
| ~~any shell~~ | ~~another device~~ | ~~tunnel~~ — CUT 2026-09-23 |
| ~~Android / iOS / desktop~~ | ~~Chrome on another device~~ | ~~BLE GATT~~ — CUT 2026-09-23 |

The web row went too: the web wallet offers no Clear Signer at all.

The end-to-end session (`secure_session`, `lib/transport/secure.js`) existed for the two
cut channels and went with them. The loopback socket is plaintext JSON on `127.0.0.1`,
behind a one-time token and an `Origin` check (071 §2).

## 3. Words
Reuse:
- the 071 keys;
- the create and sign-in screens' own method keys.

New corpus keys (all fifteen locales):

| Key | Where |
|---|---|
| `componentsUi.signing.clearSignerCreate`, `clearSignerSignIn`, `clearSignerProof` | the page's card titles (the page carries its own zh/en copies in `lib/locales`) |

The pairing sheet's words, the six-digit code, the where-question and the Settings tunnel
row were added here and REMOVED on 2026-09-23 with the channels that drew them — 23 keys
across fifteen locales (`scripts/gen-i18n.mjs`, pin 1745 → 1722).
