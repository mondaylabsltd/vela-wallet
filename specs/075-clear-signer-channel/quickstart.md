# Quickstart — 075 checks

## The core's own

```sh
cd rust
cargo test -p vela-core --features crux --test clear_signer_ceremony   # the four ceremonies, one test per refusal
cargo test -p vela-core --features crux --test clear_signer            # the loopback session
cargo test -p vela-core --test secure_session                          # the vectors are current
cargo test -p vela-core --features crux --test app_sign_pref           # the page as a preference
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
node samples/ceremony-test.mjs        # the four kinds, sessions — and vela-core judging every answer
node samples/secure-vectors-test.mjs  # secure.js against the core's vectors (Node + Chrome)
node samples/hostile-test.mjs         # 39 refusals
node samples/channels-test.mjs && node samples/safeop-test.mjs
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

## ~~SC-003 — across devices~~ — WITHDRAWN 2026-09-23

The cross-device channels were cut (spec.md, "What the owner cut"). There is
one channel: the page in this device's own browser, over the loopback socket.
