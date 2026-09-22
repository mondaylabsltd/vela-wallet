# Tasks — 075 The Clear Signer as a passkey route

## A — Core (lead)
- [ ] T001 `KeyMethod::ClearSigner` through the wire (ts-rs, Kotlin, Swift mirrors); create + sign-in machines offer it
- [ ] T002 Key records: `signer_origin`; `auto` routing follows the key; refusal for a foreign-origin key on another route
- [ ] T003 `clear_signer` ceremony requests (create / signIn / proof / memberProof) + `verify_registration` / `verify_ceremony` + tests (each refusal)
- [ ] T004 `ws::Connection`: several requests per session; `bye`; idle timeout
- [ ] T005 `secure_session` (P-256 ECDH, HKDF, AES-GCM; labels `vela-relay/1`, `vela-ble/1`) + `tests/clear-signer/secure-session.json`
- [ ] T006 UniFFI + wasm exports; wasm size gate

## B — Page (agent)
- [ ] T010 Request kinds + cards; create only from wallet requesters
- [ ] T011 Sessions of several requests (loopback WS, postMessage)
- [ ] T012 `secure.js` shared by BLE and relay, against the vectors
- [ ] T013 Relay transport, `rk` check, code screen
- [ ] T014 Hostile tests: foreign challenge, create from a site, stand-in wallet

## R — Relay (agent)
- [ ] T020 `vela-relay` rules + unit tests
- [ ] T021 `vela-relay-server` + Dockerfile
- [ ] T022 `vela-relay-worker` (Durable Object, hibernation)
- [ ] T023 Conformance on native, Worker (wrangler dev), Docker

## C — Shells (agents)
- [ ] T030 Web: the fourth route in create / sign-in / backup; postMessage sessions; relay pairing sheet
- [ ] T031 Android: same, loopback WS sessions, relay pairing sheet, Settings relay row
- [ ] T032 iOS: same
- [ ] T033 Desktop: same; loopback WS replaces fragment + callback

## D — BLE
- [ ] T040 Android peripheral · T041 iOS peripheral · T042 desktop (macOS) peripheral · T043 real-radio pass

## E — Device passes
- [ ] T050 SC-002 on the Android phone · T051 SC-003 across devices (relay: native + Worker)
