# Feature Specification: The Clear Signer, a passkey channel of our own

**Feature Branch**: `075-clear-signer-channel` (on `074-polish`)
**Created**: 2026-09-22
**Status**: In progress — **narrowed on 2026-09-23**, see "What the owner cut" below
**Input**: Owner, 2026-09-22, after testing 071:

> 但是我没看到呀 创建/登录/转账/dapp签名/公钥备份 等 都只有 这台设备 手机或平板 USB 安全密钥
>
> 清晰签名器，它的本质和 这台设备 手机或平板 USB 安全密钥 是平级的东西呀，相当于另一种 passkey 通道，而且是我们专有的通道
>
> 我觉得任何端都能接入 websocket 吧 … 要考虑跨端吧，跨端时，websocket 蓝牙是核心通道吧，不跨端的话，你现在定义的几种是对的。
>
> wss 中继需要支持 docker 和 cloudflare worker，采用 rust 去写

## What changes, and what it overturns

071 built the Clear Signer as a fourth way to *sign* (the "Sign with" row of the signing
sheet). The owner's model is broader: it is a fourth **passkey route**, beside the platform
authenticator ("这台设备"), a nearby device ("手机或平板") and a security key ("USB 安全密钥").
Wherever those three are offered, it is offered too: **create, sign in, sign (a send, a
dApp request), and the key backup**.

Two recorded rulings in `app-web/clearsigning/HANDOVER.md` / `PROTOCOL.md` are superseded by
the owner and are rewritten there:

- "签名页永远不创建 passkey" (the signing page never creates a passkey) → it creates one when
  the wallet's CREATE flow asks, as its own request, never inside a signature.

The second superseded ruling was itself superseded the next day; see below.

## What the owner cut (2026-09-23)

Everything cross-device is **gone**. The Clear Signer has ONE channel: the page in this
device's own browser, talking to a socket on this device's own loopback.

> 我觉得客户端支持回环 + 蓝牙就够了，不需要 websocket 隧道
>
> 我确定砍掉蓝牙
>
> web 就不支持清晰签名器好了

What went, and what it cost:

| Gone | Why |
|---|---|
| The WebSocket **tunnel**, its three host crates, the pairing link, the QR, the room | A service the wallet had to reach, for a path only some devices could use. The crates had already left for their own repository; now they have no caller. |
| **BLE**: both peripherals, the framing, the six-digit comparison code | The signer page needs Web Bluetooth, which Safari and every iPhone lack — and a page on another device is one this device never fetched. |
| The **end-to-end session** (`secure.rs`, `secure.js`, ECDH + HKDF + AES-GCM) and its vectors | It existed for the two cross-device channels. The loopback socket is plaintext on `127.0.0.1` behind a one-time token and an `Origin` check. |
| The Clear Signer in the **web wallet**, entirely | It had only `postMessage`; see the note below. |

**The reason, in one line.** Only a page THIS device fetched can be checked against what
it is supposed to be. That check — hash the bytes, compare against a list compiled into the
client — is what the Clear Signer's whole premise (you see what you sign) rests on, and it
is impossible when the page is on somebody else's device. It is specified separately.

**What this costs.** Loopback defends against a compromised *wallet*: the page is a
different origin the wallet cannot forge, so a compromised wallet still cannot sign by
itself. It does NOT defend against a compromised *machine* — malware that can drive the
browser can show one thing and sign another. BLE defended that and no longer does. Stated
here so nobody rediscovers it as a surprise.

**And on the web.** A wallet whose ONLY key is a Clear Signer key cannot be signed with
from the web wallet at all. The web says so by name ("this key lives behind X, which only
the Vela app can open") rather than quietly asking a platform sheet for a key no
authenticator on that device holds.

## The page's invariant

The page's founding invariant stays and is extended: **it signs only what it derived
itself.** A sign-in or a proof is a challenge the page generates or fetches from the
registry, never bytes a requester supplied. Otherwise a "sign-in" could be a transaction
hash in disguise.

## User stories

### US1: Create a wallet with the Clear Signer (P1)
On any shell, the create flow's key choice offers "清晰签名器". Choosing it opens the
signer page (on this device, or on another device through a pairing code). The page shows
"create a key for 〈wallet name〉" and runs the passkey creation there. The wallet receives
the key and continues exactly as with any other route: address, member proof, registry.
A key made this way is remembered as a Clear Signer key, with the page it lives behind.

### US2: Sign in with the Clear Signer (P1)
The sign-in screen offers it too. The page asks the person to pick their key ("sign in to
Vela") and returns an assertion over a challenge it generated itself. The wallet resolves
the account from the credential exactly as for any passkey.

### US3: Everything that signs routes to where the key is (P1)
A send, a dApp request and the key backup use the Clear Signer:
- automatically, for a key created or signed in through it ("自动");
- by choice, for any key whose page can reach it (the official page for `getvela.app` keys).

The proofs inside create/recover (verify, recover ×2, member) run on the same route as the
ceremony that made or found the key.

### ~~US4: Across devices (P2)~~ — WITHDRAWN 2026-09-23
Built, then cut. See "What the owner cut" above.

### US5: Any shell, same device (P1)
- Android, iOS and the desktop reach a signer page on the same device over a **loopback
  WebSocket** (the desktop moves off URL fragment + callback onto the phones' channel, the
  same Rust session). This is now the ONLY channel.
- The web wallet does not offer the Clear Signer at all.

## Requirements

- **FR-001** Core: `KeyMethod::ClearSigner` (`"clear_signer"`) on every passkey operation;
  the create and sign-in machines offer it; the signing routes (`auto`) follow a key's
  recorded method.
- **FR-002** Core: a Clear Signer key record carries the signer page's origin (and so its
  rpId). A key created on a self-hosted page lives behind that page, the way a security key
  lives in one device; signing it anywhere else is refused before any page opens.
- **FR-003** Page: four request kinds besides the signing intents (contract §2):
  - `vela_createPasskey` (create; refused unless the requester is a Vela wallet — an app
    channel, or a `getvela.app` origin over postMessage);
  - `vela_signIn` (the page's own domain-separated challenge);
  - `vela_proof` (`verify` / `recover_first` / `recover_second`, the page's own
    `vela-verify-…` / `vela-recover-…` strings);
  - `vela_memberProof` (the challenge fetched by the page from the registry, for the
    inputs it shows).
- **FR-004** A session carries several requests in order (create → member proof, or
  recover ×2) without reopening the page. It ends on `bye`, on a timeout, or when the page
  closes.
- **FR-005** The core verifies every answer before it is used:
  - a registration parses to a P-256 key;
  - an assertion's clientDataJSON challenge is the one the page declared for that request
    kind, and its origin is the page's;
  - a signing assertion verifies against the key (as 071).
- **FR-006** ~~Cross-device~~ — WITHDRAWN 2026-09-23. There is one channel: the loopback
  WebSocket, plaintext on `127.0.0.1` behind a one-time token and an `Origin` check.
- **FR-007** Settings: "Clear Signer page" (071). ~~plus "Tunnel"~~ — withdrawn with FR-006.

## Success criteria

- **SC-001** On each of the three NATIVE shells, the create, sign-in, send, dApp and backup
  choosers show five routes (auto + four) where they showed four; a test on each shell pins
  it. The web wallet shows four, and is pinned to NOT offering the Clear Signer.
- **SC-002** On the Android phone, a wallet is created through the Clear Signer page, signs
  a send that lands on chain, and signs in again after its local record is removed.
- ~~**SC-003** A cross-device pass~~ — WITHDRAWN 2026-09-23 with FR-006.
- **SC-004** The page refuses:
  - a sign-in or proof whose challenge it did not derive;
  - a create from a non-wallet requester.

  Hostile-intent tests cover each refusal.

## Why a self-hosted page, and what it costs (ruling, 2026-09-23)

The Clear Signer earns three things, and the third is the one the design has
to protect:

1. **The passkeys live under a domain the person controls.** Not Vela's.
2. **A signer with no supply chain.** Zero dependencies: what is served is what
   was written, and it is short enough to read.
3. **The deployment is the person's own**, so nothing Vela ships can reach it.

The registry's unit is what makes (1) whole rather than half-true. The contract
stores ONE `rpId` per unit and every member's proof carries `sha256(rpId)` from
its own authenticator, so a wallet's keys must share one relying party. If they
could span two, half of a "self-hosted" wallet would still be `getvela.app`
keys, and whoever controls that domain could recover through them. So: the
first key decides, and the rest must come from the same place — enforced in the
view (`add_methods`), at the event (`add_key` refuses the rest), and at the
publish (`registry_unit_rp_id` refuses a mixed set rather than writing a unit
nobody can prove).

**Which routes those are depends on a SETTING, not on the route.** A key minted
through this device, a nearby device or a security key belongs to `getvela.app`;
a key minted on a Clear Signer page belongs to that page's domain — which is
whatever Settings names. So the four routes coexist exactly when the configured
page is a `getvela.app` page (the official `sign.getvela.app` is), and otherwise
the set's first key decides which single side stays open:

| First key minted by | The page Settings names | Offered afterwards |
| --- | --- | --- |
| any of the three, or a `getvela.app` page | a `getvela.app` page | all four |
| any of the three | somebody's own domain | the three |
| a page on `my.example.com` | `my.example.com` | the Clear Signer |
| a page on `my.example.com` | a `getvela.app` page | none — the setting has to change first |

The last row is why a ruled-out route is drawn dimmed with a sentence rather
than hidden: the person has to be told to point Settings back at their own page,
and a row that vanished cannot say so. The core reports the two facts the
sentence needs (`add_blocked`: the set's relying party, and the configured page
with its own), and the shell owns the words
(`onboarding.create.methodBlocked{Hint,Signer}`).

**The cost, which the create flow has to say out loud.** WebAuthn binds a
credential to its `rpId`, so a self-hosted page makes the domain the custody
boundary: if it lapses, is taken at the registrar, or has its DNS moved, those
passkeys cannot be used by any other page — the credentials still exist in the
person's password manager, and nothing can reach them. Self-hosting moves the
trust from Vela to the person's own DNS and renewal discipline. That is a
reasonable trade for somebody who wants it and a trap for somebody who does
not know they made it.

**What (2) and (3) do not yet buy.** The wallet checks the ORIGIN an answer
came from, never the page's content, so a tampered deployment is
indistinguishable from a good one. Zero dependencies removes the supply chain;
it does not prove that what is being served today is what was reviewed. A
content hash the wallet remembers ("I know this version") would close it — not
in 075.

### Pinning the page's contents — what it can and cannot prove

Tempting, and worth writing down before somebody builds the version that only
looks safe: **the wallet never sees the bytes the browser ran.** The page
executes in a browser — on another device entirely, over BLE or the tunnel — so
a hash the wallet computes is a hash of ITS OWN fetch. A server can answer the
wallet with the reviewed version and the browser with another, and a cached
copy in the browser was fetched before either. A page that reports its own hash
is no better: a tampered page reports whatever it likes.

Two things do work, and they are not equally strong:

1. **The app serves the page itself, over loopback, from bytes it ships.** Then
   the bytes the browser executes and the bytes the wallet trusts are the same
   bytes, and distribution is not part of the story at all. It only covers the
   same-device channel, and it trades "my own deployment" for "the app I
   installed" — the page then belongs to `localhost`, not to the person's
   domain.
2. **Cross-device: pin the origin, and treat a hash as a tripwire.** The page
   can report a manifest of what it loaded and the wallet can compare it with a
   value the person recorded. That catches drift and careless tampering. It
   does not catch a page that lies, and it must be named a tripwire rather than
   a guarantee.

Note what `rpId` already does here: a page on any other domain — byte-identical
or not — **cannot use these passkeys at all**, because WebAuthn refuses. So
content pinning is not defending against "somebody distributed a fake page"; it
defends against "the person's own domain served bad bytes", which is a smaller
and better-understood problem (a static file, SRI on every script, a strict CSP,
and a recorded hash as the tripwire).

### Where that argument stands after 2026-09-23

The half of it about cross-device is moot: there is no cross-device channel.
What is left is the case the owner then pushed on, and pushed correctly — if
the only channel is this device's own browser, the client can fetch the page
itself and check it before opening it.

Two of the objections above survive and two do not:

- **"The app serves the page itself" is ruled OUT, definitively.** Serving from
  loopback moves the origin to `127.0.0.1`, and the keys are bound to the page's
  origin. A wallet that can serve the page can serve a lying page and sign with
  the same key — which destroys the property the Clear Signer exists for. This
  is stronger than "it trades my own deployment for the app I installed": it is
  not a trade, it is a hole.
- **"The wallet's fetch is not the browser's fetch" survives, and is the crux.**
  A discriminating server can serve one thing to the check and another to the
  navigation.
- **A page that self-reports its hash is still worthless.** Unchanged.
- **The check is still worth doing.** The realistic attack is a replaced build
  served to everyone, which one fetch catches. It must be described as
  "detects a build that does not match the published list", not "prevents".

What was worked out afterwards, and belongs in its own spec rather than here:
a hash set compiled into the client (able to shrink, not only grow), a
content-addressed URL, a per-device allow-list for a self-hosted page and a
per-device deny-list that outranks everything, a check made from a hidden
WebView so its request is the browser's own shape, decoupled in TIME from any
signing so the server cannot correlate, and a Service Worker that pins the page
after first visit so the server leaves the loop entirely.
