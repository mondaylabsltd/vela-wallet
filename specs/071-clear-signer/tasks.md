# Tasks — 071 The Clear Signer

## Phase 0–2 — core, bindings, words, the page

- [x] T001 `clear_signer::request` / `verify` / `url_launch` / `parse_callback` / `callback_query` — tests
- [x] T002 `clear_signer::ws::Connection` (RFC 6455 server side, Origin pin, token, one intent, one answer) — tests byte for byte
- [x] T003 `signer_url` validation, `uses_wallet_passkeys`, `own_send_params`, `user_op_from_json`, error codes
- [x] T004 `app::sign_pref` machine (`vela.signMethod`, `vela.clearSignerUrl`) — 8 tests
- [x] T005 UniFFI: `ClearSignerConnection`, `clearSignerRequest` (calls → fee leg, empty method → own send), `clearSignerVerify`, `clearSignerWsLaunch`, `clearSignerDefaultUrl`, `SignPrefCore`
- [x] T006 wasm: `clearSignerRequest/Verify/Url/UsesWalletPasskeys/DefaultUrl`, `ClearSignerWs`, `SignPrefCore`; TS types regenerated
- [x] T007 i18n: 10 `componentsUi.signing.clearSigner*` leaves + `settings.signing` (10) in 15 locales; pin 1713; vectors
- [x] T008 Page: `ch=ws` channel; waiting / wallet-gone states (en, zh); typed-data pick + canonical `EIP712Domain`; samples resolve the wasm by pattern
- [x] T009 Page suites: safeop 9/9, identicon 9/9, ble-loopback 10/10, hostile 15/15, channels 35/35 (11 WebSocket checks against the core's own connection over wasm)

## Phase 3 — Android

- [ ] T010 `ClearSignerChannel`: ServerSocket on 127.0.0.1:0, accept loop → `ClearSignerConnection`, Custom Tab, bring-back, 5-min timeout, cancel, reopen
- [ ] T011 `UserOpSpine`: `clear_signer` branch in `submit` and `signMessage` (request from the core, digest, keys, verdict → the same `userOpSign` / `eip1271Signature`)
- [ ] T012 The dApp path passes the request's own method/params/origin; the wallet's own send passes calls only
- [ ] T013 Signing sheet: picker from `SignPrefView.offered` + the Clear Signer's line; start at the default; waiting state with hint, reopen, cancel; refusal sentences
- [ ] T014 Settings: "Sign with" + "Clear Signer page" rows (SignPrefCore, store keys)
- [ ] T015 JVM tests (channel against a raw socket client; spine branch; settings wiring); device pass C (quickstart)

## Phase 4 — iOS

- [ ] T020 `ClearSignerChannel` (NWListener TCP, `SFSafariViewController`), spine branch, sheet, Settings
- [ ] T021 Hermetic tests; device checks (C-rows that need no tap via Web Inspector)

## Phase 5 — desktop

- [ ] T030 URL + loopback channel (`url_launch`, `callback_query`, `parse_callback`, `verify`), spine branch, sheet, Settings
- [ ] T031 `cargo test` desktop; a headless-Chrome loop test with a virtual authenticator

## Phase 6 — web

- [ ] T040 postMessage channel, `SigningHost` picker + waiting state, Settings
- [ ] T041 Unit tests; an e2e with the page served on a second origin

## Later

- [ ] T050 BLE peripheral on Android (PROTOCOL.md §2–4; the page's central exists)
- [ ] T051 The official deployment at `sign.getvela.app` (owner)
