# Corpus delta — expected: zero

Spec 021 drew every screen this cut turns live, with its text. The keys are
already in `rust/crates/vela-core/src/i18n_catalogs/` and already rendered by
`WalletFlowFixtures`, which is why the fixtures can stay canon while the live
builder produces the same strings from the core's view.

| Surface | Where its text comes from |
|---|---|
| SD1 picker, SD2 form, SD3 confirm, SD4a/b/c receipt | `send.*`, `componentsTx.*` — 021 |
| The fee-token sheet (SD2F) | `send.feeToken*`, `componentsUi.fee.*` — 021 |
| The contact picker (SD2E) | `contacts.*` — 018 |
| Every refusal | the core's own `SendAlertKind` → the key `send.alert*` 021 drew |
| The parallel-space badge | **not a corpus string**: it is a developer marker, it must be conspicuous and untranslated, and it never ships in Release. The desktop and Android made the same call. |

**The gate**: `node scripts/verify-i18n-parity.mjs` unchanged, and
`git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/i18n_catalogs/`
empty at every phase boundary.

If a key proves missing, it is added by the corpus procedure — six steps,
`cargo` features `i18n-all,crux`, the wasm bytes and fingerprint re-pinned —
never hard-coded in Swift. That would also break this cut's "no
`vela_core_uniffi.swift` regeneration" property, so a missing key is a finding
worth reporting, not a detail.
