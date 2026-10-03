# 097 part G — `wallet_sendCalls` answers in the shape its request declared (S2)

**Branch:** `097-sendcalls-v2`, on `origin/main` 231ca46ad. Not pushed.
**Evidence:** the desktop pass, 2026-10-03 09:09–09:15 (`scratchpad/desk097/`: `shots/r2-090-usdt-batch-log.png`, `logs/r2-drive.log`). Uniswap (app.uniswap.org, BNB Chain) sent `wallet_sendCalls` with `"version":"2.0.0"` (USDT approve → Permit2, Permit2 approve, swap); the wallet answered the bare string `"0x1d75e3a1…f694"`; Uniswap read `.id` (undefined), polled `wallet_getCallsStatus([null])` four times, got `-32602 Expected [id]` each time and showed "Failed to swap" over a batch that landed (tx `0xc4566172…821482`, block 125394263). `wallet_getCallsStatus` with the real id read 200.

## Plan

### The rule (EIP-5792)

EIP-5792 (ethereum/EIPs `EIPS/eip-5792.md`, status Final — the ERCs repo path `ERCS/erc-5792.md` is a 404; the EIP lives in the EIPs repo): `SendCallsParams.version: string`; `SendCallsResult = { id: string; capabilities?: Record<string, any> }` (example return value `{ "id": "0x…" }`); `GetCallsParams = [string]`; `-32602` for params the wallet cannot parse. Version "1.0" answered the bare id, and dApps built on it parse a string. PancakeSwap (also 2.0.0) worked with the bare string — its client accepts either shape; Uniswap reads `.id`.

| Question | Rule | Where |
|---|---|---|
| Which shape? | `params[0].version` a string whose major (before the first `.`, trimmed) is all digits and ≥ 2 → `{ "id": <op hash> }`; "1.0", no version, a non-string, or a major that is not digits ("v2", "+2", "") → the bare id. | `dapp_rpc::send_calls_answers_object`, `send_calls_result` |
| `capabilities` in the answer? | None. Vela's only capability is `atomic`, which EIP-5792 carries in the request's `atomicRequired` and the status's `atomic`, not in this answer; the wallet has nothing else to attach. | `send_calls_result` (doc) |
| Read from what? | The params the page **sent** (`Pending.params_json` at approve), not the sheet's rewrite (`params_override_json`) — captured as `Inflight.batch_answers_object`. | `sign_request.rs` approve |
| Where is the answer formed? | Once: every `Ok` with a result for the in-flight request goes through `ok_answer(fl, result)` — at acceptance (`answer_batch_id`), the tracker's `Confirmed`, `ReceiptPending`, `Succeeded`, and after the §4 record (`AfterRecord::Result`). `SignResponsePayload::Ok.result` is now the page's JSON value (`Option<Value>`; a string serialises exactly as before). | `sign_request.rs` |
| Who reads the id back? | `SignResponsePayload::answered()` (`dapp_rpc::batch_id_of`: the string, or `{id}`'s `id`) — `ending_of`, the browser core's batch record, the desktop's `user_op_hash_of`, the web executor (`signAnswered` over wasm). The record keeps the bare id. | core |
| `wallet_getCallsStatus` params | `[id]` per the spec; `[{ id }]` (the 2.0.0 answer handed back whole) reads the same; `[null]`, `[]`, `[{id:null}]` → `-32602` as before. | `dapp_rpc::calls_status_id` |

### Shells (forward, never shape)

- **In-app browsers (desktop, iOS, Android):** the browser core delivers `SigningAnswered.payload.result` as it is (`dapp_browser::signing_answered`) and records the batch by the method from either shape. Desktop: `user_op_hash_of` reads `payload.answered()`. iOS: the payload dict passes through untouched (no code change; `opHashAnswer` names no op for an object — the browser core knows a batch by its method, 097 E). Android: `SignResponsePayload.Ok.result` is `JsonElement?` (was `String?`, which could not decode an object).
- **Web + extension:** the executor sends `payload.result` as formed; the hash for the RF3 chain lookup comes from the core (`signAnswered`, wasm). `dapp-submit.ts` `batchIdFor` returns the id to the core only (doc corrected: the core shapes the page's answer).
- **Extension worker (no wasm):** the answers it forms itself — a batch that may have been sent (`maybeSentPayload`, via `surfaceAnswer` / the recovery `ending`) — use the twin `sendCallsResult(record.params, hash)`; `opRecord` reads `{id}` (`batchIdOf`); `callsStatusId` takes `[{id}]`. Twins pinned to the core over wasm (`dappRpcSendCallsResult`, `signAnswered`, `dappRpcCallsStatusId`) in `calls-status.test.ts`.

## Tasks

- [x] T1 core `dapp_rpc`: `send_calls_answers_object`, `send_calls_result`, `batch_id_of`; `calls_status_id` takes `[{id}]`; unit tests.
- [x] T2 core `sign_request`: `Ok.result: Option<Value>`; `answered()`; `Inflight.batch_answers_object` from the sent params; `ok_answer` on every `Ok`-with-result path; `ending_of` reads `{id}`.
- [x] T3 core `dapp_browser`: deliver the result as formed; batch recorded from either shape.
- [x] T4 wasm exports `dappRpcSendCallsResult`, `signAnswered`.
- [x] T5 core tests in `tests/app_batch_id_097.rs` with Uniswap's batch (`req-uni-usdt-batch-v2.json`, calls decoded from the MultiSend it landed in) and that landing's receipt (`receipt-uni-usdt-batch.json`); the 097 E tests now expect `{id}` (PancakeSwap's batch is 2.0.0 too); test helpers adapted (`app_sign_request.rs`, `app_dapp_browser.rs`).
- [x] T6 regenerate wasm + pkg-web + TS mirror (`SignResponsePayload.ts`); Swift bindings unchanged (no UniFFI change).
- [x] T7 web/extension: executor, kernel, worker twins; vitest (executor, worker, twins); e2e `extension-lifecycle` strict on `{id}`.
- [x] T8 desktop: `user_op_hash_of`; wiring test through the real sign core + `BrowserDriver`.
- [x] T9 Android: `Ok.result: JsonElement?`; wiring test through the real sign core + browser core.
- [x] T10 iOS: wiring test through the real sign core + `BrowserController`.
- [x] T11 suites, CI scripts, this file.

## Results

| Suite | Command | Result |
|---|---|---|
| core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,513 passed, 0 failed, 2 ignored |
| core lint | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean |
| core, new | `tests/app_batch_id_097.rs` (10) + `dapp_rpc` unit tests | 8 of the 10 fail with the answer forced bare (the old behaviour); the `dapp_rpc` rule tests are new functions |
| i18n | `i18n_residency` | ja+en 137,675 B resident (budget 141,800) — **0 bytes added**, no corpus change |
| web | `pnpm build:extension`; `npx vitest run`; `pnpm check` | built (dev, release, store); 178 files, 2,637 passed (5 skipped); 0 errors / 0 warnings |
| web e2e | `npx playwright test -c playwright.isolated.config.ts e2e/extension-lifecycle.e2e.ts -g "wallet_sendCalls\|eth_sendTransaction: slid"` | 2 passed (the batch test now requires `{ id }` and reads `.id`) |
| desktop | `cargo fmt --all -- --check && cargo clippy --all-targets && cargo test` | 924 passed, 49 ignored; clippy: no warning on a changed line (all pre-existing) |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` (Kotlin bindings + host fixtures regenerated) | 977 tests, 0 failures (final tree, `--rerun-tasks`). One earlier full run under heavy parallel load (iOS + desktop suites at once) failed `DappSignMachineTest` "the tracker was handed a persisted row" — an `eth_sendTransaction` timing assertion this change does not touch; 3/3 isolated reruns and the clean full rerun passed |
| iOS | `build-ios-xcframework.sh` (`check-ios-core-fresh`: ok); `-only-testing:` `DappBrowserTests`, `BrowserMemoryTests`, `CoreWire082Tests`; then `VelaWalletTests` (own clone of iPhone 16 Pro, iOS 18.0) | 44 tests in 3 suites passed; full: 1,180 tests in 148 suites passed |
| CI scripts | reachability / event payloads / dead controls | reachable; 0 mismatches (539 sites); 0 dead controls |
| artefacts | `build-web --check`, `gen-onboarding-types --check` | current (`vela_core_bg.ea5be311a932.wasm`); Swift bindings byte-identical (no UniFFI change) |

No UI changed (the answer is wire-only), so there are no screenshots.

## Decisions (for the lead)

1. The shape is read from the request **as the page sent it**, not the sheet's rewritten params (the guard keeps `version` today; nothing should depend on that).
2. Version parsing is strict and conservative: only a digits-only major ≥ 2 gets the object. Anything unreadable keeps the 1.0 bare string, which a client that parses either shape (PancakeSwap's did) still accepts.
3. No `capabilities` key in the answer (nothing to report; `atomic` lives elsewhere in EIP-5792).
4. `wallet_getCallsStatus([{ id }])` is accepted (outside the spec's `[string]`, harmless, and what a client that echoes the answer would send); `[null]` stays `-32602`.
5. `SignResponsePayload::Ok.result` changed type (string → any JSON). The wire is unchanged for every string answer; the TS mirror is `unknown`; Kotlin `JsonElement?`; Swift was already untyped.

## Open questions / not done

1. EIP-5792 also says "If provided, the wallet MUST respect the `id` field and return it in the response" (an app-chosen id) and "MUST reject requests with duplicate `id`s". Vela ignores an app-provided `id` and always answers its op hash. Not in this fix's scope; a dApp that sends its own `id` and polls by it would read 5730. PancakeSwap's logged request carries none; the hook logged only the first 120 characters of Uniswap's, so whether it sent one is not known (`req-uni-usdt-batch-v2.json` reconstructs `chainId`/`from`/`atomicRequired` and has no `id`).
2. Device verification (Uniswap on desktop/iOS/Android/extension with real money) is the lead's.
