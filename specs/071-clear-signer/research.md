# Research — 071 The Clear Signer

## R1 — What the page is for (and what it is not)

`app-web/clearsigning` is a zero-dependency page that takes a signing intent,
decodes it from the operation's own bytes (`lib/resolve.js`, `lib/safeop.js`),
derives the digest itself (`lib/digest.js`), refuses what it cannot derive, and
only then runs the passkey ceremony (`lib/signer.js`). The trust split: the app
*proposes*; the page, which the person can host themselves, *shows and signs*.
A compromised app build can still propose something bad — the page then shows
the bad thing as it is. The page cannot make the wallet submit anything the
wallet did not assemble: the wallet verifies the answer against its own digest.

Not in scope: making the page a wallet (it never creates a key), or trusting a
self-reported origin (only postMessage/extension channels verify one).

## R2 — One channel per shell [D]

| Shell | Channel | Why |
|---|---|---|
| Android, iOS | **WebSocket on the app's own loopback** (`ws://127.0.0.1:<port>`), page opened in an in-app browser tab (Custom Tab / `SFSafariViewController`) | A redirect to a loopback URL needs the app to be *listening* when the browser navigates; a phone suspends an app whose screen is covered by the system browser. An in-app tab keeps the app process in the foreground, and a socket stays open for the whole ceremony. A custom-scheme callback could be claimed by any installed app. |
| Desktop | **URL fragment in, loopback callback out** (`url_launch` / `callback_query` + `parse_callback`) | The desktop app keeps running while the default browser is in front; a top-level navigation to `http://127.0.0.1:<port>/vela?…` is the most compatible return path (no socket from a public page is needed on the way back). |
| Web | **postMessage** to a `window.open`ed page | The only channel where the browser vouches for both origins. |
| (P3) Any | BLE GATT (phone peripheral, page central) | Specified in PROTOCOL.md §2–4 and implemented in the page; the shell peripheral is later work. |

## R3 — Local Network Access [finding]

Chrome (desktop 142+, Android 153 verified on the Xiaomi) asks the person before
an https page may open `ws://127.0.0.1`: "allow this site to access other apps
and services on this device". It is asked once per origin. A page that is
waiting on it looks hung, so:

- the page says `ui.waitingWallet` if the socket has not opened after 1.5 s;
- the app's waiting sheet says the same thing (`clearSignerWaitingHint`).

The official origin gets one prompt per browser, ever. WebKit (iOS) treats
`127.0.0.1` as potentially trustworthy and does not block `ws://` to it from an
https page; LNA is not implemented there (verify on device — quickstart C3).

## R4 — Who may talk to the listener [D]

- A random ephemeral port, bound to `127.0.0.1` only.
- The handshake's `Origin` must equal the signer page's origin (the one the app
  opened); anything else gets `403` before it can say a word.
- A 128-bit one-time token in the fragment (never sent to a server, wiped from
  history by the page) must come back in the first message; a wrong token is
  closed without ending the request, so a stray tab cannot cancel it.
- One intent, one answer. A page that had the intent and goes away = declined.
- The shell times the whole thing out after 5 minutes (the core has no clock).

## R5 — Passkeys and the rpId [finding]

The wallet's passkeys are made for `getvela.app`. A browser lets a page use a
passkey made for its own domain or a registrable parent of it, so only a page
under `getvela.app` (the official `sign.getvela.app`, or the extension build of
the page) can sign with them. A self-deployed copy elsewhere can decode and
show a request but its ceremony will find no key. Settings says so
(`settings.signing.pageForeign`) instead of letting the person discover it at
the worst moment. The on-chain verifier does not check the rpId hash, so a
loopback-served page with a key holding the same private key signs validly —
which is how the device pass tests the whole loop without the production
deployment.

The official page URL is `https://sign.getvela.app/` (the host PROTOCOL.md
already names). Deploying it is the owner's (a production domain is not changed
by this feature).

## R6 — The digest must be computed twice and agree [D]

The page derives the digest from what it rendered; the wallet computes its own.
The wallet accepts only an assertion whose `clientDataJSON.challenge` is its
own digest — so the two derivations must agree byte for byte:

- transactions: the Safe 4337 SafeOp hash over the assembled operation (pinned
  by `samples/safeop-test.mjs` against the core over wasm);
- `personal_sign`: EIP-191 then SafeMessage under the Safe's domain (pinned by
  the WebSocket section of `channels-test.mjs`);
- typed data: fixed here — the page now picks the data the way the wallet does
  (`eth_signTypedData` / `_v1` → `params[0]`, the rest `params[1] ?? params[0]`)
  and derives an undeclared `EIP712Domain` in EIP-712's canonical order, not the
  domain object's key order.

## R7 — The WebSocket server lives in the core [D]

Android has no WebSocket *server* in the platform; iOS has one
(`NWProtocolWebSocket`) with its own opinions about handshakes. Writing two
servers, two origin checks and two token checks would be three places for the
security boundary to drift. The core's `clear_signer::ws::Connection` is pure:
bytes in, bytes out, one outcome. Every shell's server is a TCP accept loop.
The same connection runs under Node over wasm in the page's own suite, so the
page and the phones are tested against the same code.

## R8 — Preferences in a machine [D]

`app::sign_pref` (shape of `fee_tier_pref`): the default "Sign with" every
signing sheet starts at, and the signer page. Keys `vela.signMethod`,
`vela.clearSignerUrl` (`vela.` prefix survives sign-out). The core validates the
page address (https anywhere, http only on loopback), treats an unknown stored
value as unset without rewriting it, and publishes whether the page can use the
wallet's passkeys.

## R9 — The wallet's own send has no site [D]

A send the person started in Vela is sent to the page as `wallet_sendCalls`
built by the core from the calls before the fee leg (`own_send_params`), with an
empty origin — which the page draws as the wallet itself. The fee leg's index is
`calls.len()` on every chain (the core appends it last for in-band and Tempo
fees).
