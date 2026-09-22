# Contract — network readiness (FR-009)

**Core → shells**: `NetCompatibility` gains `multi_key_ready: bool`; `contracts[]` carries the 12-entry required set (adds SafeWebAuthnSignerFactory `0x1d31F259EE307358a26dFb23EB365939E8641195` and its singleton `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`; drops CompatibilityFallbackHandler, which Vela wallets never use — the 4337 module is the fallback handler).

**Shell obligations**: show single-key readiness and multi-key readiness distinctly; a chain that is `compatible && !multi_key_ready` must say that only a one-key wallet can sign there. iOS additionally gains the P-256 row it lacks today.

**Site mirror**: `app-web/getvela.app/src/lib/chain-setup/required-contracts.ts` keeps parity with the Rust list (existing drift test reads the Rust file), `deployment-data.json` gains CREATE2 payloads for the two new entries where anyone may deploy them, and the chain-setup verdict distinguishes the two readiness levels.
