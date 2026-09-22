# Implementation Plan: 075 — the Clear Signer as a passkey route

**Branch**: `075-clear-signer-channel` (on `074-polish`) | **Spec**: [spec.md](spec.md)
**Contracts**: [clear-signer-channel.md](contracts/clear-signer-channel.md), [relay.md](contracts/relay.md)

## Phases

| Phase | Deliverable | Who | Gate |
|---|---|---|---|
| A | Core: `KeyMethod::ClearSigner`; the create and sign-in machines offer it; key records carry `signer_origin`; ceremony requests + verifiers; multi-request `ws::Connection`; `secure_session` (ECDH/HKDF/AES-GCM) + vectors; UniFFI + wasm | lead | core tests, clippy, wasm size |
| B | Page: `vela_createPasskey` / `vela_signIn` / `vela_proof` / `vela_memberProof`; sessions of several requests; `lib/transport/secure.js` (shared by BLE and relay) against the vectors; relay transport + the code screen; hostile tests | agent | page suites |
| R | Relay: `vela-relay` (rules), `vela-relay-server` (tokio, Docker), `vela-relay-worker` (workers-rs + Durable Object) | agent | conformance on both hosts |
| C | Shells: route `clear_signer` ceremonies to the session; the fourth option in create / sign-in / backup; pairing sheet (QR + link + code); Settings "Relay"; desktop onto the loopback WebSocket | one agent per shell | each shell's suites |
| D | BLE peripherals: Android `BluetoothGattServer`, iOS `CBPeripheralManager`, desktop (macOS) | agents | loopback vectors + a real-radio pass (phone ↔ Mac Chrome) |
| E | Device passes: SC-002 (phone), SC-003 (relay across devices) | lead | quickstart |

Order: A and B and R in parallel (the contracts fix the seams); C after A; D after A+B; E last.

## Risks
- **A create through a stand-in page.** If the relay link leaks, somebody else's page could
  answer a create with its own key. Mitigations:
  - the code check on the wallet (relay and BLE);
  - `rk` in the link (the page refuses a stand-in wallet);
  - `verify_registration` pins the origin.
- **Self-hosted keys.** A self-hosted page's keys can only ever be used through that page.
  The wallet must say so in the create flow's line when the page is not official
  (`settings.signing.pageForeign` is the existing sentence).
- **rpId on the phones.** Only `*.getvela.app` pages share the app's own passkeys (071 R5);
  the on-chain verifier does not check the rpId hash, so a foreign page's keys still sign
  valid operations.
