# Implementation Plan: 075 — the Clear Signer as a passkey route

**Branch**: `075-clear-signer-channel` (on `074-polish`) | **Spec**: [spec.md](spec.md)
**Contracts**: [clear-signer-channel.md](contracts/clear-signer-channel.md)
**Narrowed 2026-09-23**: phases R and D were built and then CUT, with the web shell's
half of C. See spec.md, "What the owner cut".

## Phases

| Phase | Deliverable | Who | Gate |
|---|---|---|---|
| A | Core: `KeyMethod::ClearSigner`; the create and sign-in machines offer it; key records carry `signer_origin`; ceremony requests + verifiers; multi-request `ws::Connection`; UniFFI + wasm | lead | core tests, clippy, wasm size |
| B | Page: `vela_createPasskey` / `vela_signIn` / `vela_proof` / `vela_memberProof`; sessions of several requests; hostile tests | agent | page suites |
| ~~R~~ | ~~Tunnel~~ — built, then cut. The three crates had already moved to the `vela-tunnel` repository; nothing here calls them. | — | — |
| C | Shells: route `clear_signer` ceremonies to the session; the fourth option in create / sign-in / backup; desktop onto the loopback WebSocket. **Native only** — the web wallet has no Clear Signer. | one agent per shell | each shell's suites |
| ~~D~~ | ~~BLE peripherals~~ — built, then cut. | — | — |
| E | Device pass: SC-002 (phone) | lead | quickstart |

Order: A and B in parallel (the contract fixes the seam); C after A; E last.

## Risks
- ~~**A create through a stand-in page.**~~ Gone with the pairing link: there is no link to
  leak. `verify_registration` still pins the origin.
- **A compromised machine.** The page is a separate origin but the same computer. Malware
  that can drive the browser can show one thing and have another signed. BLE used to answer
  this; nothing does now. Recorded in spec.md rather than mitigated here.
- **Self-hosted keys.** A self-hosted page's keys can only ever be used through that page.
  The wallet must say so in the create flow's line when the page is not official
  (`settings.signing.pageForeign` is the existing sentence).
- **rpId on the phones.** Only `*.getvela.app` pages share the app's own passkeys (071 R5);
  the on-chain verifier does not check the rpId hash, so a foreign page's keys still sign
  valid operations.
