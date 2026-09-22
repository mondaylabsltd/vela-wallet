# Results — 081 (in progress)

Branch `081-audit-product-gaps`, worktree `vela-wallet-081`, from `main` at `746e2259`.

## Baseline (T001)

Measured before any change, so "did I break it" has an answer:

| Gate | Baseline |
| --- | --- |
| `cargo test --workspace --features vela-core/i18n-all` | green (48 suites) |
| web wallet `pnpm test` | **6 failures, pre-existing**: 3 × `extension/package.test.ts` (needs `pnpm build:extension` first), `explore/fixtures.test.ts`, `i18n/messages.test.ts` (FLOW_KEYS scan), `tokens/tokens.test.ts` (a `36px` literal in `SigningHeader.svelte`) |
| desktop `cargo check` | 7 warnings, pre-existing |

## User Story 1 — a dApp cannot take over the wallet (FR-005/006) — **done, device run pending**

**The rule** (`rust/crates/vela-core/src/app/self_call_guard.rs`, new): 13 Safe control selectors refused when the target is the signing account; recursion through `multiSend` and `execTransaction` payloads (depth 4); **any inner `delegatecall`, whatever its target** — a dApp cannot express one through `{to, value, data}`, so crafted calldata carrying one is hostile; `SafeTx` typed data refused because it is a valid EIP-1271 authorisation if this account owns another Safe. Empty-data self-calls stay allowed: the in-band fee leg and the gas estimator use exactly that shape.

**Two hooks** in `sign_request.rs`: at arrival, and again at the submit chokepoint (a shell can hand back rewritten `params_override_json`, and those are the bytes that get signed).

**The decision the e2e forced.** Answering the dApp at arrival closed the window that was explaining the refusal — the extension worker closes a request window the moment the request settles, because that window *is* the answer surface. So the request now stays pending and unsignable until the person closes the sheet, and the dismissal answers `self_call_blocked`, not 4001: the wallet refused it, not the person. A sheet that refuses also shows no fee row and no slider — a dead "Slide to confirm · Enable module" under a refusal reads as an option someone merely failed to use.

**Verification**

| Check | Result |
| --- | --- |
| `tests/app_self_call_guard.rs` | 13 tests: every selector, batch leg (names the step), nested multiSend, delegatecall, SafeTx; negatives (empty self-call, view selector, ordinary call); the machine tests (refused before answering, answered on dismissal, unsignable even if the shell asks) |
| core suite | green, no regressions |
| i18n gate | `gen:i18n` (path pin 1687 → 1691, reason recorded), `lint:i18n` clean, `verify:i18n` 74045 comparisons zero divergences, `dump:vectors`, `build:wasm`, `gen:core-types` |
| web `pnpm check` | 0 errors, 0 warnings |
| web `pnpm test` | 3 pre-existing failures remain (see baseline); the `tokens` one no longer includes any file this work touched |
| web e2e `extension-signing` | **5 passed**, including the new blocked-request test end to end |
| desktop `cargo check` | 7 warnings, unchanged |
| Android `assembleDebug` + `testDebugUnitTest` | green (the strict `when` over error kinds caught the missing branch at compile time — exactly what that pattern is for) |
| iOS build | xcframework built; app build pending |
| Device run — Android (FR-019) | **done**, Xiaomi `9d5f42fb`, 2026-09-22: the dev test dApp (served over `adb reverse`, opened through `velawallet://open?url=`) asked for `enableModule` on the wallet's own address; the sheet showed "Vela can't sign this" and the full sentence naming `enableModule`, with no confirm control in the view tree. Nothing was signed — the account on that phone holds real funds, and the refusal path never reaches a signature |
| Device run — iPhone 11 (FR-019) | **pending** |

## Still open in this feature

Everything else in [tasks.md](tasks.md): descriptor provenance, network readiness, forward-verified names, the iOS endpoints page and index-per-call, `X-Rpc-Url`, release provenance and the iOS privacy manifest, the dormant routes, feedback, erase, and the two service-repo PRs. The docs sync (FR-020) lands with each gap as it closes.

**Follow-ups noticed while working**, recorded so they are not lost:

- desktop, Android and iOS still render the decoded body and the fee row under the refusal (measured on the Xiaomi); only the web sheet was reshaped to show the refusal alone with a single Close. Same treatment needed for parity.
- `SignResponder.sendResponse` now carries the core's error `kind` — other shells could use it the same way the extension window does.
