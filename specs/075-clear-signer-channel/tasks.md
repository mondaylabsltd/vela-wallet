# Tasks — 075 The Clear Signer as a passkey route

## A — Core (lead)
- [x] T001 `KeyMethod::ClearSigner` through the wire (ts-rs, Kotlin, Swift mirrors); create + sign-in machines offer it
- [x] T002 Key records: `signer_origin`; `auto` routing follows the key; refusal for a foreign-origin key on another route
- [x] T003 `clear_signer` ceremony requests (create / signIn / proof / memberProof) + `verify_registration` / `verify_ceremony` + tests (each refusal)
- [x] T004 `ws::Connection`: several requests per session; `bye`; idle timeout
- [x] T005 `secure_session` (P-256 ECDH, HKDF, AES-GCM; labels `vela-relay/1`, `vela-ble/1`) + `tests/clear-signer/secure-session.json`
- [x] T006 UniFFI + wasm exports; wasm size gate

## B — Page (agent)
- [x] T010 Request kinds + cards; create only from wallet requesters
- [x] T011 Sessions of several requests (loopback WS, postMessage)
- [x] T012 `secure.js` shared by BLE and relay, against the vectors
- [x] T013 Relay transport, `rk` check, code screen
- [x] T014 Hostile tests: foreign challenge, create from a site, stand-in wallet

## R — Relay (agent)
- [ ] T020 `vela-relay` rules + unit tests
- [ ] T021 `vela-relay-server` + Dockerfile
- [ ] T022 `vela-relay-worker` (Durable Object, hibernation)
- [ ] T023 Conformance on native, Worker (wrangler dev), Docker

## A+ — after the contracts (lead)
- [x] T007 `sign_pref`: the relay is a preference (`vela.clearSignerRelay`), with its own rules and refusals
- [x] T008 i18n: where the signer is, the pairing sheet, the code, the relay row — all fifteen locales (pin 1717 → 1733)
- [x] T009 The page's ceremony suite also judged by the real core (`clearSignerVerifyCeremony`), 61/61

## C — Shells (agents)
- [x] T030 Web: the fourth route in create / sign-in / backup; postMessage sessions; relay pairing sheet
  — `AddMethodPicker` lists four (create's first key, "add another", the sign-in
  sheet); the onboarding executor routes `method = clear_signer` to the page and
  reports the core's verdict; one page visit per flow (create → member proof,
  sign-in → proofs), ended with `bye` when the flow is; the relay requester runs
  on WebCrypto, pinned to `tests/clear-signer/secure-session.json`; the sheet
  asks where, draws the QR + link + the six digits, and sends nothing before the
  confirm; a key with `signer_origin` is signed on ITS page, `auto` included;
  Settings' relay row in both layouts. 1577 unit tests; e2e: create through the
  page, sign in again, a signature for a key behind the page, and the same create
  across two pages over the mock relay.
- [ ] T031 Android: same, loopback WS sessions, relay pairing sheet, Settings relay row
- [ ] T032 iOS: same
- [ ] T033 Desktop: same; loopback WS replaces fragment + callback

## D — BLE
- [ ] T040 Android peripheral · T041 iOS peripheral · T042 desktop (macOS) peripheral · T043 real-radio pass

## E — Device passes
- [ ] T050 SC-002 on the Android phone · T051 SC-003 across devices (relay: native + Worker)
