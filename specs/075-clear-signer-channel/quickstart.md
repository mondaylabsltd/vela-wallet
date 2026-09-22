# Quickstart — 075 checks

## The core's own

```sh
cd rust
cargo test -p vela-core --features crux --test clear_signer_ceremony   # the four ceremonies, one test per refusal
cargo test -p vela-core --features crux --test clear_signer            # the loopback session, the relay link
cargo test -p vela-core --test secure_session                          # the vectors are current
cargo test -p vela-core --features crux --test app_sign_pref           # the page and the relay as preferences
cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures
```

The session vectors are `rust/crates/vela-core/tests/clear-signer/secure-session.json`.
An independent WebCrypto reading of them (the page's own crypto, not this crate's):

```sh
node <scratch>/check-secure.mjs rust/crates/vela-core/tests/clear-signer/secure-session.json
```

## The page (zero-dependency, `app-web/clearsigning`)

```sh
export CHROME_BIN="…/Google Chrome for Testing"     # HANDOVER.md
export SB=<dir with tls-serve.py, cert.pem, key.pem>
node samples/ceremony-test.mjs        # the four kinds, sessions, the relay — and vela-core judging every answer
node samples/secure-vectors-test.mjs  # secure.js against the core's vectors (Node + Chrome)
node samples/mock-relay-test.mjs      # the mock relay against relay.md §1
node samples/hostile-test.mjs         # 39 refusals
node samples/channels-test.mjs && node samples/ble-loopback.mjs && node samples/safeop-test.mjs
```

## The relay

```sh
cargo run -p vela-relay-server                       # 127.0.0.1:8787
node rust/crates/vela-relay/tests/conformance.mjs ws://127.0.0.1:8787
cd rust/crates/vela-relay-worker && wrangler dev --local
node ../vela-relay/tests/conformance.mjs ws://127.0.0.1:8787            # the Worker host
docker build -t vela-relay rust/crates/vela-relay-server && docker run -p 8787:8787 vela-relay
```

## SC-002 — the Android phone (a wallet created through the Clear Signer)

The page on the phone's own loopback is a secure context, so WebAuthn runs there:

```sh
cd app-web/clearsigning && python3 -m http.server 8140 &
adb reverse tcp:8140 tcp:8140
```

Settings → Clear Signer page → `http://localhost:8140/` (accepted: loopback).

| # | Do | Expect |
|---|---|---|
| E1 | Create a wallet, key method "清晰签名器" | a Custom Tab opens the page; its card says "create a key for 〈name〉"; the key is made there |
| E2 | The same session continues | the member proof card follows without reopening the tab; the wallet finishes creating and the address appears |
| E3 | Settings → keys | the new key's row says it lives behind the Clear Signer page |
| E4 | Send dust from that wallet | "Sign with" is already the Clear Signer (the key lives there); the page shows the transfer; it lands on chain |
| E5 | Sign out, sign in with "清晰签名器" | the page asks which key; the wallet comes back with the same address |

## SC-003 — across devices (the relay)

The wallet on the Android phone, the signer page in Chrome on the Mac:

```sh
cargo run -p vela-relay-server                        # or wrangler dev --local
```

Settings → Relay → `ws://<mac-lan-ip>:8787` (a relay under test; the phone reaches
the Mac on the LAN). Settings → Clear Signer page → the Mac's page over https
(the sandbox cert) or the official page once it is deployed.

| # | Do | Expect |
|---|---|---|
| F1 | Sign with the Clear Signer → "在另一台设备上" | the phone shows a QR code, a link and "waiting" |
| F2 | Open the link in Chrome on the Mac | both screens show the same six digits |
| F3 | Confirm on the phone | the page shows the request; sliding there signs it; the phone submits, and it lands |
| F4 | Change one digit's worth: point the page at a different room | the page refuses a requester whose key does not match `rk` |
