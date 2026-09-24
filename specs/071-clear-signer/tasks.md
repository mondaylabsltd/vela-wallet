# Tasks — 071 The Clear Signer

## Phase 0–2 — core, bindings, words, the page

- [x] T001 `clear_signer::request` / `verify` / `url_launch` / `parse_callback` / `callback_query` — tests
- [x] T002 `clear_signer::ws::Connection` (RFC 6455 server side, Origin pin, token, one intent, one answer) — tests byte for byte
- [x] T003 `signer_url` validation, `uses_wallet_passkeys`, `own_send_params`, `user_op_from_json`, error codes
- [x] T004 `app::sign_pref` machine (`vela.signMethod`, `vela.trustedSignerUrl`) — 8 tests
- [x] T005 UniFFI: `ClearSignerConnection`, `clearSignerRequest` (calls → fee leg, empty method → own send), `clearSignerVerify`, `clearSignerWsLaunch`, `clearSignerDefaultUrl`, `SignPrefCore`
- [x] T006 wasm: `clearSignerRequest/Verify/Url/UsesWalletPasskeys/DefaultUrl`, `ClearSignerWs`, `SignPrefCore`; TS types regenerated
- [x] T007 i18n: 10 `componentsUi.signing.clearSigner*` leaves + `settings.signing` (10) in 15 locales; pin 1713; vectors
- [x] T008 Page: `ch=ws` channel; waiting / wallet-gone states (en, zh); typed-data pick + canonical `EIP712Domain`; samples resolve the wasm by pattern
- [x] T009 Page suites: safeop 9/9, identicon 9/9, ble-loopback 10/10, hostile 15/15, channels 36/36 (11 WebSocket checks against the core's own connection over wasm; 3d the two-leg tamper)
- [x] T009b Page fixes from the device pass: fiat only at the requester's rates; the chain's own coin; a refusal survives the legs

## Phase 3 — Android

- [x] T010 `ClearSignerChannel`: ServerSocket on 127.0.0.1:0, accept loop → `ClearSignerConnection`, Custom Tab (a named browser, never the chooser), bring-back, 5-min timeout, cancel, reopen
- [x] T011 `UserOpSpine`: `clear_signer` branch in `submit` and `signMessage` (request from the core, digest, keys, verdict → the same `userOpSign` / `eip1271Signature`)
- [x] T012 The dApp path passes the request's own method/params/origin; the wallet's own send passes calls only
- [x] T013 Signing sheet: picker from `SignPrefView.offered` + the Clear Signer's line; start at the default; waiting state with hint, reopen, cancel; refusal sentences
- [x] T014 Settings: "Sign with" + "Clear Signer page" rows (SignPrefCore, store keys)
- [x] T015 JVM tests (channel against a raw socket client with a real P-256 key; settings wiring; SIGN_METHODS pinned) — 637 pass; device pass C on the Xiaomi: C1–C3 a send signed on the page landed on Gnosis (`UserOperationEvent` success, 0xe3f7e00c…7117733), C4 closed tab, C5 a stranger's key, C6 an insecure page refused, C7 a dApp's `personal_sign` through the Clear Signer verified on chain by the Safe (`isValidSignature` → 0x1626ba7e)

## Phase 4 — iOS

- [x] T020 `ClearSignerChannel` (NWListener TCP on 127.0.0.1, `SFSafariViewController`), spine branch, sheet, Settings; closing the tab closes the app's end (WebKit may keep the page's socket)
- [x] T021 Hermetic tests (683); simulator end to end against the real page: request rendered, C4, C6, loopback-only

## Phase 5 — desktop

- [x] T030 URL + loopback channel (`url_launch`, `callback_query`, `parse_callback`, `verify`), spine branch, sheet, Settings; desktop message signing made to work at all (it never could)
- [x] T031 `cargo test` desktop (472); `scripts/clear-signer-e2e.sh` — the real page + virtual passkey: own send, personal_sign, closed tab, tampered operation (found the page's two-leg refusal bug, fixed)

## Phase 6 — web

- [x] T040 postMessage channel, `SigningHost` picker + waiting state, Settings (phone and wide layouts)
- [x] T041 Unit tests (1482 + the 3 known main failures); `e2e/clear-signer.e2e.ts` 3/3 with a virtual passkey — a real EIP-1271 signature, a closed popup, a stranger's key

## Later

- [ ] T050 BLE peripheral on Android (PROTOCOL.md §2–4; the page's central exists)
- [ ] T051 The official deployment at `sign.getvela.app` (owner)
