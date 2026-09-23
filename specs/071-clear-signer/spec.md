# Feature Specification: The Clear Signer — a fourth way to sign

**Feature Branch**: `071-clear-signer`
**Created**: 2026-09-22
**Status**: Core + page done; shells in progress
**Input**: Owner, 2026-09-22 — "帮我把 app-web/trusted-signer 整明白，desktop ios android 甚至是 web 版本都应该能和这个零依赖的签名模块连接，现在签名方式有三种选项（这台设备/手机或平板/USB 安全密钥），我觉得需要新增一种新的，名字叫什么我没想好，它应对的是一种可信运行环境，所签即所见的效果，连接到这个签名器的通道有一些，比如蓝牙（BLE GATT），比如 websocket（主要用在一个 ios app 或 android app 选择这种签名方式时，为了方便可以和本机上的一个网页通信，这个网页可以是 vela 官方部署，也可以用户自己部署的一个零依赖的 clearsigning），比如 URL 片段 + 回环回调。"

Owner decisions **[R]**; decisions made here **[D]**.

## The name [D]

**Clear Signer** — 「清晰签名器」 (zh-TW/zh-HK 「清晰簽署器」, following those
locales' 簽署; other locales keep "Clear Signer" as a name, as they keep
"Clear Signing"). The three existing
choices answer *where your passkey is* (this device, a phone, a USB key); this one
answers *where you check what you sign*: a separate, zero-dependency page that
decodes the request from its own bytes, derives the digest itself, and only then
asks the passkey. It reuses the corpus's existing term for the idea (清晰签名 /
"clear signing") so a person who met the words in the signing sheet recognises
them in the picker. Rejected: "Independent signer" (says nothing about what makes
it trustworthy), "Hardware-like signer" (it is not hardware), "Signing page"
(describes the mechanism, not the promise).

One-line explainer under the choice: "Check and sign on a separate page — what you
see is what you sign." / 「在独立的页面上核对并签名——所见即所签。」

## Why

Every other method signs a digest the wallet app computed and displayed. If the app
(or its build, or its update channel) is compromised, the screen can lie and the
passkey will still sign — the Bybit failure mode. The clearsigning page splits the
two: the request comes from the app; the decoding, the digest and the ceremony
happen on a page the person controls (official deployment or their own copy),
which renders what the signature authorises from the signed bytes and refuses what
it cannot derive.

The page exists and is well-tested in isolation (post, extension port, URL +
loopback, BLE central), but **no wallet talks to it**: no shell builds its request,
none verifies its answer, the "Sign with" vocabulary has no fourth value, and the
channel the owner asks for on phones (WebSocket to a local page) does not exist.

## User Scenarios & Testing

### US1 — Sign with the Clear Signer from the phone (P1)

On iOS or Android, in the signing sheet, a person opens "Sign with" and picks
**Clear Signer**. The app opens the signer page in the system browser's in-app tab
(SFSafariViewController / Custom Tab) and hands it the request over a local
WebSocket. The page shows the transaction decoded from the operation's own
calldata, the person slides, the browser's passkey sheet signs, the page returns
the assertion, the tab closes, the app verifies the signature is over the digest
it built and by one of this wallet's keys, and submits.

**Independent test**: device pass with the page served on the phone's loopback
(Android `adb reverse`), checking the channel, the rendering, the ceremony and the
wallet's verification; the refusal paths (tampered operation, wrong key, closed
tab) end the sheet cleanly with a clear message.

Acceptance:
1. Closing the tab without signing returns the sheet to "not signed" with the
   words "The Clear Signer was closed" — never a spinner.
2. An assertion whose challenge is not the digest the app computed, or whose key
   is not one of the wallet's, is refused before anything is submitted.
3. A page answering "refused" (e.g. unlimited approval, operation mismatch) is
   shown as the page's refusal, distinct from the person's cancel.

### US2 — Sign with the Clear Signer from the desktop (P1)

Desktop opens the default browser at the signer page with the request in the URL
fragment and a one-time token, listens on a loopback port, receives the answer by
top-level redirect (sign) or beacon (refusal), verifies, submits. The browser tab
says "You can close this tab."

### US3 — Sign with the Clear Signer from the web wallet (P2)

The web wallet opens the signer page with `window.open` and posts the request;
the page answers by `postMessage`. Origins checked on both sides.

### US4 — Choose the default and the page (P2)

Settings → Signing: the default "Sign with" (Automatic / This device / Phone or
tablet / Security key / Clear Signer), and the Clear Signer page address (default
the official one; a person may point at their own deployment). The address is
validated (https, or loopback http) and the rpId rule is explained in one line:
a page on another domain can only use passkeys made for that domain.

### US5 — Cross-device over Bluetooth (P3)

A phone app signs through a Clear Signer page open in Chrome on a computer nearby:
the phone advertises (GATT peripheral), the page connects (central), both show a
six-digit comparison code, and the rest is the same request/answer. Built in the
core (framing, handshake, encryption, replay), shell peripheral on Android first.

### Edge cases

- The app is backgrounded while the tab is open (iOS SFSafariViewController keeps
  the app foreground; Android Custom Tab + keep-alive): the local server must
  survive the ceremony; a timeout (5 min) ends it with a clear message.
- Two requests at once: the Clear Signer serves one request at a time (the
  browser core already serialises signing).
- The page is unreachable (offline, wrong URL): the tab shows the browser's
  error; the app offers "Try another way" (the other methods).
- The wallet's passkeys are `getvela.app` keys: only a page served under
  `getvela.app` (or the Chrome extension build of the page) can use them. The
  settings line says so; the default URL is the official one.

## Requirements

### Core — `vela_core::clear_signer` (pure, no machine)

- **FR-001** `request(input) -> json`: the page's `{intent, context}` from what
  the shell already holds when it would sign: the RPC method + params (for the
  wallet's own sends, a synthesised `eth_sendTransaction` / `wallet_sendCalls`),
  origin, chain id + name, account address + name, the ASSEMBLED user operation
  and the fee leg's index, the account's credential ids (hex → base64url for the
  page), and the entry point / module.
- **FR-002** `verify(answer_json, digest, keys) -> Assertion`: parses the page's
  answer; checks `clientDataJSON.type == "webauthn.get"`, `challenge ==
  base64url(digest)`, the UV flag, that the credential is one of `keys`, and that
  the P-256 signature over `authenticatorData ‖ sha256(clientDataJSON)` verifies
  under that key; returns the core's `Assertion` (DER signature, hex credential
  id) so the existing `user_op_sign` / `eip1271_signature` build the Safe
  signature unchanged. Anything else is a typed refusal.
- **FR-003** Channels: `url_launch(base, request, callback, token)`
  (deflate-raw + base64url fragment, PROTOCOL.md §7.2), `callback_query(head)` +
  `parse_callback(query, token)`; and `clear_signer::ws::Connection` — the
  loopback WebSocket, server side, byte for byte (RFC 6455 handshake + frames,
  `Origin` pinned to the signer page, the one-time token, one intent, one
  answer, a close after the intent = declined) so every shell's server is a raw
  TCP socket and nothing more. Over UniFFI it is `ClearSignerConnection`, which
  also verifies, so its outcome is already the verdict.
- **FR-003b** `app::sign_pref` machine: the default "Sign with" and the signer
  page (`vela.signMethod`, `vela.trustedSignerUrl`), validated by the core.
- **FR-004** BLE session (P3): frame split/join, ECDH P-256 + HKDF + AES-GCM,
  comparison code, monotonic `n` — PROTOCOL.md §2–4, byte-compatible with
  `lib/transport/ble.js`.

### The page (`app-web/trusted-signer`)

- **FR-005** A `ws` channel in `lib/intake.js`: `?ch=ws#p=<port>&t=<token>`,
  connects `ws://127.0.0.1:<port>`, says hello with the token, receives one
  intent, answers once; a closed tab closes the socket (the app reads that as
  "closed"). Fragment wiped from history at once.
- **FR-006** One result shape on every channel: `{t|vela: 'result', id, result:
  {credentialId, signature (raw r‖s), authenticatorData, clientDataJSON, …}}`
  (PROTOCOL.md §4 and `samples/ble-peripheral.mjs` corrected).
- **FR-007** Correctness fixes found while mapping: `eth_signTypedData` /
  `_v1` take the typed data from `params[0]`; `EIP712Domain` is built in the
  canonical field order the core uses; the samples resolve the committed wasm by
  glob, not a stale fingerprint.

### Shells

- **FR-008** "Sign with" gains `clear_signer` on the signing sheet on all four
  shells; `sign_route` knows it; Kotlin/Swift decoders accept it.
- **FR-009** The signing spine's sign step, for `clear_signer`, builds the request
  with the core, runs the platform channel (Android/iOS: WebSocket + in-app
  browser tab; desktop: URL + loopback; web: postMessage), verifies with the core,
  and continues exactly as today.
- **FR-010** Settings → Signing: default method + Clear Signer page URL
  (validated), on all four shells.

## Success criteria

- **SC-001** The core has one request builder, one verifier and one set of channel
  encoders; no shell parses a WebAuthn answer or frames a WebSocket itself.
- **SC-002** The page's own suites (`safeop`, `hostile`, `channels`, `identicon`,
  `ble-loopback`) run green again, plus a `ws` channel test.
- **SC-003** On the Android phone: the Clear Signer opens, receives the request
  over WebSocket, renders it, runs a real passkey ceremony (a localhost key),
  answers, and the app's verification accepts a matching key and refuses a
  foreign one. The same on the iPhone.
- **SC-004** A tampered operation, a closed tab, and a wrong key each end with a
  clear, specific message and nothing submitted.

## Assumptions / owed

- The official deployment (`https://sign.getvela.app/` **[D]**, the host PROTOCOL.md
  already names) is the owner's to make — a production domain is not changed by
  this feature. Until it exists, the setting accepts any https / loopback URL and
  the device pass uses a loopback-served page.
