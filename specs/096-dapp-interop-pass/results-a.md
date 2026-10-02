# 096 part A — results (F1, F3, F8, F11, F1's class across shells)

Branch `096-dapp-native-value` from `origin/main` 07a7287c0, with
`origin/094-chrome-web-store` merged in (918795185): F3 is a fix to 094's
EIP-5792 answer (PR #388, not yet on main). If 094 lands first the merge is a
no-op. Not pushed.

| Commit | What |
|--------|------|
| 918795185 | merge 094 (EIP-5792 status/capabilities live there) |
| b6a0e82c6 | core: `tx_request`, held failures + `RetryTapped`, `calls_status` 400, `consent_address`; regenerated wasm, pkg-web, TS mirrors, Swift bindings |
| cc1c5f84d | web + extension: F1, F8, F3, F11 |
| 815710985 | desktop: F1, F8 |
| 02f0bd61f | iOS: F1, F8 |
| aaed3f51b | Android: F1, F8, F11 parity |
| a9d841717 | core self-call test, clippy, extension-connect e2e; regenerated wasm |

## F1 — native value in dApp calls (S2 blocker)

**Root cause.** `MultiSendCall.value` is "hex, `0x` optional". `dapp-submit.ts`
stripped the `0x` (`aa87bee538000`), and `innerCallsGasFloor`
(`safe-transaction.ts:1900`, spec 062, 16139e041) read it with
`BigInt(call.value)`, which reads hex only behind its prefix and reads bare
digits as DECIMAL. Every dApp contract call with a value threw `Cannot convert
aa87bee538000 to a BigInt` before the passkey; the catch called it "could not
estimate gas". Underneath was a reader of `value` per shell, no two alike:

| reader | `"1000"` | `1000` | `"0x"` | `"0X1f"` | bad batch leg |
|--------|----------|--------|--------|----------|---------------|
| card (RC4) | refused | refused | 0 | refused | — |
| web fee quote `weiOf` | 1000 dec | 1000 | 0 | 0 | — |
| web submit | 0x1000 hex | throws | 0 | throws | — |
| desktop `wei_of` | 1000 dec | 1000 | 0 | 31 | dropped, rest sent |
| iOS `callsOf` | 0x1000 hex | **0** | refused | refused | dropped, rest sent |
| Android `callsOf` | 0x1000 hex | refused | 0 | refused | refused; `"-1"` = −1 |

**Fix, in the core.** `rust/crates/vela-core/src/tx_request.rs`: one rule (the
card's RC4, plus prefix-less zero `"0"`/`0`, which reads the same in every
base): absent/null/`""`/`"0x"` → 0; `0x`+hex → that number ≤ 2²⁵⁶−1; anything
else refused. `sign_request` arrival runs `canonical_params_json`: a refused
value answers `-32602` before any sheet; a readable non-canonical one is
rewritten (`0x`, lower-case, minimal) so every later reader holds one text —
untouched when already canonical (viem/ethers/web3 always are).
`calls_of` (every call or none) is the submit's reading on all shells: wasm
`dappRequestCalls`, uniffi `dapp_request_calls`, desktop directly.

- web: `dapp-submit` sends `dappRequestCalls` → `toShellCall` (Send's codec,
  `0x`-hex); every `MultiSendCall.value` reader goes through `callValueWei`;
  `feeCallsOf` reads the same core calls.
- desktop / iOS / Android: `calls_of` / `callsOf` = the core's.

Open question for the owner: MetaMask reads a bare string as hex
(`addHexPrefix`). We refuse it (`-32602`) rather than guess; only zero is
accepted bare. One function changes if the owner wants MetaMask parity.

## F8 — a failure before signing is said, with Try again

**Root cause.** The core answered the page at the failure (`fail_inflight`);
the extension worker closes the request window the moment a request is
answered, so the failure the sheet drew was never seen (the spec 081 refusal
had already met this).

**Fix, in the core.** A failure while its sheet is up is held
(`Pending::held`): shown, not answered. The close answers it once (`-32603`,
the failure's words); a new request answers the one it replaces; nobody
looking → answered at once. `RetryTapped` drops the held answer and returns
the request to review when nothing was sent and it was no refusal
(`SignView::failure_retryable`). Wording reuses Send's: `send.txErrorGeneric`,
`send.txRetryBtn`, `componentsTx.receipt.done`; a relay refusal keeps
`componentsUi.signing.refused` and no retry.

Shells: web/extension (window + panel), desktop, iOS, Android draw Done +
Try again on the failed receipt; all four in-app browsers share the core.
Not done: an Activity row for a request that sent nothing (nothing happened;
Send writes none either). Closing the extension window from the OS while a
failure shows answers the window's generic settlement, not the held words.

## F3 — `wallet_getCallsStatus` after a relay rejection

`dapp_rpc::calls_status` takes the relay's `pimlico_getUserOperationStatus`
result when there is no receipt: `rejected` naming no bundle tx (the tracker's
terminal refusal, `refused_before_any_block`) → EIP-5792 **400**; `rejected`
with a tx, or anything else → 100; a receipt → 200/500. The in-app browsers
(`dapp_browser`) ask the relay as a second read; the extension worker's twin
(`protocol.js` `callsStatusResult`, `background.js`) does the same, pinned to
the core by `calls-status.test.ts`.

## F11 — the consent names what it shares

Core: `DpermPopupView::consent_address` (the signed-in account
`PopupApproved` pins). Web/extension: `ConsentFacts.svelte` under the title in
the window and the panel — identicon, name, short address; network name and
logo (`explore.account`, `explore.network`). Parity: desktop and iOS already
named the core's `DbrConsentView` account and chain; Android named the
session's active account and the front tab's chain logo — now the core's
`address` and `chain_id`.

## Tests

- Core: `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures`
  **2416 passed, 0 failed**; clippy `-D warnings` clean; fmt clean. New:
  `tx_request` (value table, canonicalization, every-call-or-none),
  `app_sign_request` (F1 arrival ×4, F8 hold/retry/supersede/unwatched ×5),
  `app_dapp_browser` (400 vs 100), `dapp_rpc` (relay rule),
  `app_dapp_permissions` (consent_address).
- Web: vitest **175 files, 2549 passed, 5 skipped**; `pnpm check` 0 errors;
  `pnpm build:extension` ok. `dapp-native-value.test.ts` drives PancakeSwap's
  swap, Aave depositETH and a batch with a native leg through the REAL submit
  on BNB Chain (native and USDC fee) and Tempo — all six fail on the old code
  with the console's exact `Cannot convert aa87bee538000 to a BigInt`.
- Extension e2e (connect, signing, lifecycle, store-package, live-provider):
  **38 passed**. Reruns on the final wasm hit 3-minute timeouts in three
  cases this part does not touch (per-site chain switch, typed data,
  plain-http refusal), a different one each run; each passes alone.
- Desktop: fmt ok, clippy (no new warnings in touched code), `cargo test`
  **897 passed, 0 failed, 49 ignored**.
- Android: `testDebugUnitTest` **957 tests, 0 failures**.
- iOS: `VelaWalletTests` **1150 tests in 147 suites passed** (own cloned
  simulator, deleted after).
- CI scripts: native reachability ok; event payloads 0 mismatches; dead
  controls 0.

i18n: 0 bytes added (no new strings). ja+en resident 136,431 B under the
141,800 budget 094 set.

## Screenshots

Session scratchpad, `096-shots/`: `096-consent-window.png` (the real
extension request window, e2e), `096-consent-facts.png`,
`096-failure-retryable.png` / `096-failure-refused.png` (web sheet),
`096-ios-failure-retryable.png` / `096-ios-failure-refused.png` (iOS receipt
body). Desktop and Android: no screenshot harness for a live signing receipt.
