# Feature Specification: The Clear Signer, a passkey channel of our own

**Feature Branch**: `075-clear-signer-channel` (on `074-polish`)
**Created**: 2026-09-22
**Status**: In progress
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
- "没有服务器。没有中继。跨设备走 BLE。" (no server, no relay; cross-device goes over BLE) →
  cross-device goes over a **relay** (WebSocket) or **BLE**. The relay is blind: it only
  sees ciphertext (contracts/relay.md).

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

### US4: Across devices (P2)
- **Relay.** The wallet shows a QR code and a link. Another device opens the signer page
  from it, and both screens show the same six-digit code; the person confirms it on the
  wallet, and the request goes through an end-to-end-encrypted relay. The relay is written
  in Rust and runs in Docker or as a Cloudflare Worker.
- **BLE.** A native wallet advertises; a Chrome signer page nearby connects over BLE GATT
  (PROTOCOL.md §1–4), with the same code check.

### US5: Any shell, same device (P1)
- Android, iOS and the desktop reach a signer page on the same device over a **loopback
  WebSocket** (the desktop moves off URL fragment + callback onto the phones' channel, the
  same Rust session).
- The web wallet uses `postMessage`, since a page cannot listen on a port.

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
- **FR-006** Cross-device: the relay (contracts/relay.md); BLE per PROTOCOL.md §1–4. Both run
  the same session code in the core (Rust) and in the page (`lib/transport/secure.js`),
  checked against shared vectors.
- **FR-007** Settings: "Clear Signer page" (071) plus "Relay" (default
  `wss://relay.getvela.app`, same URL rules).

## Success criteria

- **SC-001** On each of the four shells, the create, sign-in, send, dApp and backup choosers
  show five routes (auto + four) where they showed four; a test on each shell pins it.
- **SC-002** On the Android phone, a wallet is created through the Clear Signer page, signs
  a send that lands on chain, and signs in again after its local record is removed.
- **SC-003** A cross-device pass: the phone's wallet and the Mac's Chrome signer page pair
  through the relay (native host and Worker host), and a send signed there lands.
- **SC-004** The page refuses:
  - a sign-in or proof whose challenge it did not derive;
  - a create from a non-wallet requester;
  - a relay requester whose key does not match the link.

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
