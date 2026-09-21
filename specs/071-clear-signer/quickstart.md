# Quickstart — 071 The Clear Signer

## The page's own suites (≈2 min)

```sh
cd app-web/clearsigning
export CHROME_BIN=…/Google Chrome for Testing   # HANDOVER.md
export SB=<dir with tls-serve.py, cert.pem, key.pem>   # HANDOVER.md
node samples/safeop-test.mjs && node samples/identicon-test.mjs && \
node samples/ble-loopback.mjs && node samples/hostile-test.mjs && \
node samples/channels-test.mjs                  # 35/35, incl. "ws:" rows
```

The `ws:` rows run the phones' connection (`vela-core` over wasm, the same
Rust) against the shipped page in headless Chrome with a virtual passkey.

## C — Android device pass

The page on the phone's own loopback (a secure context, so WebAuthn runs):

```sh
cd app-web/clearsigning && python3 -m http.server 8140 &
adb reverse tcp:8140 tcp:8140
```

Settings → Clear Signer page → `http://localhost:8140/` (accepted: loopback).
The wallet's passkeys are `getvela.app` keys, so for the happy path the phone's
Chrome gets a virtual authenticator over CDP holding the parallel space's
software key under rpId `localhost` (the on-chain verifier does not check the
rpId hash — research R5).

| # | Do | Expect |
|---|---|---|
| C1 | Send dust with "Sign with" → Clear Signer | a Custom Tab opens the page; the page shows the transfer decoded from the operation, the fee leg as the fee |
| C2 | Chrome asks about apps on this device (first time) | allow; the page renders (the app's sheet said to) |
| C3 | Slide on the page | the tab closes itself, the app verifies, submits; the dust lands on Gnosis |
| C4 | Start again, close the tab without sliding | the sheet: "The Clear Signer was closed without signing"; the request can be signed another way |
| C5 | A virtual key that is NOT the wallet's | "…does not match this request, so nothing was sent" |
| C6 | Settings: `http://192.168.1.4/` | "Use an https page…"; nothing stored |
| C7 | A dApp's `personal_sign` through the Clear Signer | the page shows the message; the signature verifies on chain (EIP-1271) |
