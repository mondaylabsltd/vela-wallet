# 100 — Results (2026-10-04)

Branch `100-dapp-add-network` (from `main` @ `90bcb02b5`), not pushed.

## Decisions as built (the owner's defaults, research R1–R9)

| Default | As built | Departures |
|---|---|---|
| D1 known chain → switch | unchanged (`dapp_browser` and the extension's worker) | — |
| D2 unknown → a sheet on that tab; one at a time (-32002); the asker named | `ForwardToAddNetwork` → `network_admin`'s `dapp_add` → each shell's consent-sheet slot; a second add from any tab -32002; the sheet's lead names the host, the shells bring the asking tab forward (iOS/Android) | The sheet follows a consent when both are open (desktop/Android draw the consent first; iOS keeps whichever is up) |
| D3 catalog RPC/name first; page RPC only when the catalog does not know; https only (http per the debug rule); must answer `eth_chainId` | `FetchChainInfo` first; `usable_rpc_url` = `offers_wallet`'s rule; on this path an RPC counts only when it reports the asked id (catalog RPCs too) | A catalog that cannot be reached reads as "unknown" in every shell's executor, so the page's words are used — labelled "the site's" (research R3, known limit). `rpcUrls` absent/empty is not refused up front (more dApps work). `decimals` ≠ 18 is -32602. |
| D4 Settings' compatibility check; incompatible → Settings' words, an error, nothing added | the wizard's check refactored into shared step functions — the same code, every old test unchanged; incompatible answers **4902** when the sheet closes | Code choice: 4902 (research R4); "unable to verify" offers Retry and its close is 4001 |
| D5 approve → the Settings add path, switch, `chainChanged`, `null`; decline → 4001 | `build_custom_network` + `save_custom_network`, then `DappAddSettled{added}` once the write is answered; the browser adds the chain, `WriteSiteChain`, `chainChanged`, `null` | — |
| D6 rules in the core; shells relay and draw; the record has layer/reason | `dapp_rpc` / `dapp_browser` / `network_admin`; shells carry three messages; record: `consent` class, `sheet/rejected_by_person`, `wallet/not_compatible`, `wallet/bad_rpc`, `wallet/consent_busy`, `wallet/bad_params`, `browser/navigated_away` | The extension's worker keeps the two synchronous decisions (known → switch, busy) from the core-published catalog, as it does for connect |
| D7 words in 15 locales; residency budget untouched | 3 new keys + `consentBusy` generalised; 2 dead keys removed | — |

## What each client shows (en · zh)

Title **Add Network · 添加网络**; lead **"app.example asks to add a network" · "app.example 请求添加网络"**; rows
Name / Chain ID / Native Token / RPC URL / Explorer (名称 / 链 ID / 原生代币 / RPC URL / 区块浏览器); then by phase:

| Phase | en | zh | Buttons |
|---|---|---|---|
| checking | Checking compatibility... | 兼容性检查中... | Cancel · 取消 |
| ready | Compatible (+ the check list) | 兼容 | Add Network · 添加网络 / Cancel |
| not compatible | Incompatible + "Some required contracts are not yet deployed on this chain. …" | 不兼容 + “此链上尚未部署所需合约。…” | Open Chain Setup Tool · 打开链配置工具 / Done · 完成 |
| unable to verify | Unable to verify — RPC request failed | 无法验证 — RPC 请求失败 | Retry · 重试 / Cancel |
| wrong RPC | That RPC serves a different network (chain 100, expected 987654321). | 该 RPC 属于其他网络（链 100，应为 987654321）。 | Done |
| no RPC | The site gave no usable RPC for this network | 网站未提供此网络可用的 RPC | Done |
| from the site | Not in Vela’s network list — the name and coin are the site’s. | 不在 Vela 的网络列表中——名称和币种由网站提供。 | — |

The record's line for an incompatible chain is Settings' "Not compatible with Vela Wallet · 与 Vela
Wallet 不兼容"; for a busy sheet, "Another request is open · 另一个请求尚未处理".

Where: desktop — the Connection column (third column); Android — a non-dismissable bottom sheet
over Explore; iOS — the Explore sheet slot; web — the extension's side panel sheet or request
window.

## Verification

- **Core** (`rust/`): `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets
  --features vela-core/dev-fixtures -- -D warnings` clean; `cargo test --workspace --features
  vela-core/i18n-all,vela-core/dev-fixtures` **2,583 passed, 0 failed, 2 ignored** (70 test
  binaries). New `tests/app_dapp_add_network_100.rs` **24** (one per rule, both machines end to
  end ×2); `app_network_admin` 74 and `app_dapp_browser` 74 green (one old assertion — "add for an
  unknown chain is 4902" — moved to the new decision); `i18n_residency` 6.
  `gen-core-types --check`: 416 types current. `build-web` then `sync-wasm`, `build-web --check`:
  current (wasm 4,677,531 bytes, `vela_core_bg.2dcb925a0da7.wasm`), built after the last core change.
- **Desktop**: `cargo fmt -- --check` clean; `cargo clippy --all-targets` 67 warnings, none in a
  changed line (every one predates this branch); `cargo test` **919 passed, 0 failed, 49 ignored** —
  new `explore::add_network` (3) and `browser_host::a_pages_add_network_is_checked_saved_and_answered`
  (both real machines and executors).
- **Android**: `build-android.sh` + `build-kotlin-bindings.sh`, then `:app:testDebugUnitTest`
  **1,001 / 1,001** (new `AddNetworkFromPageTest` 4, real cores over JNA; `CoreWireDriftTest` 77
  green with the new variants).
- **iOS** (`Vela-s100`, iPhone 17 / iOS 26.2, the CI job's commands): Swift Testing **1,213 tests in
  157 suites passed**; XCTest 16 executed, 2 skipped, 0 failures; the iOS 17 function-metadata
  check OK (25 Mach-O files). New `Spec100Tests` (4); the two operation-list pins moved (16 → 17,
  11 → 13). `vela_core_uniffi.swift` unchanged (no UniFFI surface change).
- **Web**: `pnpm check` — only the pre-existing `background.test.ts:1719` error. `pnpm test:unit`
  **2,630 passed, 24 failed, 5 skipped**; the 24 are `extension/package.test.ts`, which reads a
  built `extension/dist` this worktree did not have — after `pnpm build:extension` that file is
  **38 / 38**. New: `add-network.test.ts` (5), worker (5), content (2), protocol (2 assertions).
- **Wire / words**: `check-event-payloads` **0 mismatches** over 556 dispatch sites;
  `lint:i18n` no new defects; `verify:i18n` 76,700 comparisons, zero divergences.

## Residency

`ja` + `en` runtime route **143,811 / 144,400** (was 144,350); compiled route 139,585. Removed
`onboarding.login.alertNotFound{Title,Body}` (881 bytes of `en`+`ja`; no full path or leaf name
anywhere outside the corpus and the generated tables), added 334.

## Open

- The catalog fetch cannot tell "unknown" from "unreachable" (all four executors answer `None`);
  telling them apart is a wire change in four executors.
- No device pass was run here (the Xiaomi was in use; the iPhone was not to be touched): the
  quickstart is the owner's.
