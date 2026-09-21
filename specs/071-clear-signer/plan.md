# Implementation Plan: 071 — The Clear Signer

**Branch**: `071-clear-signer` | **Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md) | **Contract**: [contracts/clear-signer.md](contracts/clear-signer.md)

## Summary

Make the zero-dependency signing page a fourth "Sign with" on every wallet.
The core builds what the page receives, speaks the phones' loopback WebSocket
byte for byte, verifies what comes back, and keeps the preferences; each shell
owns a socket (or a popup), an in-app browser tab and the words.

## Technical context

Same stack as 070 (Crux machines + free functions over UniFFI / wasm / direct
crate). New dependency: `sha1` (RustCrypto, the WebSocket accept key) and
`miniz_oxide` (deflate for the URL channel), both pure Rust, no_std-capable.
Devices: the Xiaomi (Chrome 153 for Custom Tabs), the iPhone 11.

## Constitution check

Rules move down (request, verification, framing, validation in the core), the
security boundary (who may connect, what is accepted) has one implementation
and tests on both sides (core suite + the page's suite running the core over
wasm). Risk **High**: a signing path. Evidence: core tests, the page's
end-to-end suite with a real WebAuthn ceremony, a device pass per phone.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 | `clear_signer` (request, verify, URL channel, `ws::Connection`, url validation, own-send intent) + `sign_pref` machine | core tests (19 + 8) |
| 1 | Bindings: UniFFI `ClearSignerConnection`, `clearSignerRequest/Verify/WsLaunch/DefaultUrl`, `SignPrefCore`; wasm exports; TS types; i18n (20 leaves + `settings.signing`) | `gen:core-types`, `gen-i18n` pin 1713, vectors |
| 2 | The page: `ch=ws`, waiting / gone states, typed-data + domain fixes, samples glob | page suites green; channels 35/35 |
| 3 | Android: channel (ServerSocket + Custom Tab), spine hook, sheet states, Settings rows | JVM tests; device pass C1–C6 |
| 4 | iOS: channel (NWListener TCP + SFSafariViewController), same hooks | hermetic tests; device checks |
| 5 | Desktop: URL + loopback, same hooks, Settings | `cargo test` desktop |
| 6 | Web: postMessage popup, same hooks, Settings | web unit + e2e |
| 7 | BLE peripheral (P3) | — (later) |

## Project structure (touched)

```
rust/crates/vela-core/src/clear_signer.rs, clear_signer/ws.rs, app/sign_pref.rs
rust/crates/vela-core/tests/{clear_signer,app_sign_pref}.rs
rust/crates/vela-core-uniffi/src/{clear_signer_bridge,onboarding_bridge,lib}.rs
rust/crates/vela-core-wasm/src/{clear_signer,wallet_state,lib}.rs
app-web/clearsigning/{lib/intake.js,lib/digest.js,sign.js,lib/locales/*,samples/*}
app-android/…/feature/signing/clearsigner/*, send/core/UserOpSpine.kt, settings
app-ios/…/Features/Signing/ClearSigner/*, Core/UserOpSpine.swift, settings
app-desktop/vela-wallet/src/executor/clear_signer.rs, user_op.rs, settings
app-web/vela-wallet/src/lib/signing/clear-signer.ts, passkey.ts, settings
```
