# Results: 094 — the Chrome Web Store submission

**Status**: done on `094-chrome-web-store` (from `origin/main` @ `ec033f231`), not pushed.
**The upload**: `app-web/vela-wallet/vela-wallet-extension-0.9.6-chrome-web-store.zip` — built from
`7d26a85ef` (the commits after it are docs only), 23,092,256 bytes, sha256
`d60b085ec5787852de45d1875f771f0221930a592ce46422d9aa1ef7fd9d338f`. Rebuild with
`pnpm package:extension` in `app-web/vela-wallet` (the zip is gitignored build output).

## The store zip, verified

| Check | Result |
|---|---|
| `manifest.json` at the root | yes |
| `key` | absent (the development zip keeps it) |
| `version` / `minimum_chrome_version` / `incognito` | 0.9.6 / 122 / `not_allowed` |
| permissions / hosts | `storage, tabs, sidePanel` / `https://getvela.app/*, *://*/*` (unchanged) |
| developer pages (`parallel`, `gallery`, `dev/`) | 0 files |
| the wasm the code names (`WASM_URL` `/vela_core_bg.96d7940c799b.wasm`) | present, 4,276,822 B |
| files / unpacked size | 1,328 / 37.4 MB |
| installed as is (`e2e/extension-store-package.e2e.ts`) | id assigned by Chrome, welcome opens, wasm compiles in the extension page, provider announced as "Vela Wallet"/`app.getvela`, no console line, EIP-5792 answered, site-access grant, install note — 6/6 |

## Items

| Item | Status | What / where |
|---|---|---|
| B1 store zip without `key` | done | `extension/build.mjs` derives `dist-store`; CI uploads `chrome-extension` + `chrome-web-store`; package test covers both |
| B2 self-sufficient build | done | token check + wasm copy in `build.mjs`; package test fails on a named wasm not in the package (checked by deleting it: 4 failures) |
| B3 submission sheet | done | `docs/store-submission/chrome-web-store.md` |
| B4 images | done | `docs/store-submission/chrome-web-store/` (3 screenshots 1280×800, tile 440×280, icon 128×128); `scripts/store-art/` re-renders them |
| S1 Chrome 122 | done | manifest; site install doc ×15 |
| S2 restricted site access | done | `ExtensionNotices` + one-click `permissions.request`; `SecurityError` → corpus sentence |
| S3 install | done | `runtime.onInstalled` → welcome tab; reload note when web tabs were open |
| S4 erase | done | `packagedHref(welcome)`; e2e |
| S5 no-wallet window | done | create / sign in in a tab; the window turns into the consent; e2e |
| S6 EIP-5792 | done | core route + every in-app browser + worker twin; e2e: a batch read back as confirmed (200) |
| S7 one window per site | done | worker queue; e2e: 6 connects → 1 window, 1 decision |
| S8 Permit2 unlimited | done, with one deviation | the warning on every shell from one core flag. **No cap editor for a permit**: an off-chain permit is redeemed by the dApp with its OWN struct, so a capped signature only reverts the dApp's transaction (`GuardBlockReason::OffChainPermit`); every shell now says "can't be capped here" instead (desktop and web did not) |
| S9 offline | half done | **fee**: a 15 s core bound on every quote, every shell's timer — "Estimating…" becomes a failure with words and the retry schedule. **Balance after an offline reload: deferred** — see below |
| S10 privacy policy | done | any site's reads are forwarded; Limited Use statement (English-only page by design) |
| S11 version | done | 0.9.6 (rule allows 0.9.6 or 26.10.0; 26.10.0 would close the 0.9 line — owner's call); all four shells |
| S12 e2e | done | 59/59 on the final build; new: store package ×6, typed data v4, connect with no wallet, connect burst, a send that lands, a batch's status, erase |
| NICE console line | done | removed from the core provider (+ test) |
| NICE incognito | done | `not_allowed` |
| NICE parallel space | done | pruned from the store package only |

## Deferred: S9's balance half

Offline, every shell reports "settled, nothing found" (`FetchSettled` with every chain failed), and the
core reads that as a real zero; the cached total exists only after a round in which EVERY chain answered
(24 networks: one flaky chain blocks every write), and no shell persists the holdings list. Showing the
last known balance needs either a new shell fact (which chains answered — four shells) so the core can
call "nothing answered" unknown instead of $0.00, or a persisted holdings cache in four shells — and an
owner call: with the simple rule ("no holdings, some chain failed, nothing cached → unknown") a brand-new
wallet with one dead network would never see "Deposit your first asset". Pointers: `balance_dashboard.rs`
`accept(FetchSettled)` (~1221–1315), `display_total` (~1423); web `wallet/live.ts` `assetsMode`;
desktop `wallet/live.rs` `assets_strip_empty`; iOS `WalletLive.swift`; Android `WalletLive.kt`.

## Tests

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,325 passed, 0 failed |
| core clippy `--workspace --all-targets -D warnings` / `fmt --check` | clean / clean |
| i18n gen / lint / verify / dump | ok; verify 75,515 comparisons, 0 divergences |
| residency (ja + en) | 139,689 B (budget 140,800); the four strings add 1,037 B of ja + en JSON |
| web `npx vitest run` | 171 files, 2,454 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| extension package test | 28/28 |
| extension e2e (every `e2e/extension-*.e2e.ts`, Chrome for Testing, throwaway profiles) | 59/59 on `7d26a85ef` (a run under load average ~85 timed one test out at 180 s; it passed 3/3 alone and the full rerun was 59/59) |
| desktop fmt / clippy / test | clean / no new warnings / 880 passed, 49 ignored |
| Android `testDebugUnitTest` | 911 passed, 0 failed (an earlier run had `DappSignMachineTest` fail once under load — the CoreDriver race main fixes in #383) |
| iOS `VelaWalletTests` | 1,039 tests in 137 suites: 1 failure (a JSON fixture missing the new field — fixed; its suites re-run 24/24); the 4 feedback suites skipped — see below |
| `check-native-reachability` / `check-event-payloads` / `check-dead-controls` | clean / 0 mismatches / 0 dead controls |

**iOS, not 094's**: `FeedbackScreenshotTests`, `ScreenshotViewerTests`, `BugReportTests` and
`SettingsFeedbackRowTests` crash the test host (EXC_BAD_ACCESS in `FeedbackSender.add(datas:)`,
"type metadata accessor for nonisolated(nonsending)" — Xcode 26.3), in code 094 does not touch; the
crash reports are `~/Library/Logs/DiagnosticReports/VelaWallet-2026-10-02-1834*.ips`.

## Screenshots

- Store art: `docs/store-submission/chrome-web-store/*.png`.
- New web UI (review): `evidence/ui-notices-wide-light.png`, `evidence/ui-notices-narrow-dark.png`
  (site-access notice + install note), `evidence/ui-request-no-wallet.png` (S5).
- The permit sheet with the new line: `docs/store-submission/chrome-web-store/screenshot-3-signing.png`.
- Native shells: the change is one more warning block of an existing kind on a permit sheet; covered by
  unit tests, not shot (no device or gallery fixture for a live permit).

## Shells without the surface

- In-app browsers (desktop, iOS, Android) get S6 through the core's `dapp_browser`, with no shell code.
- S2, S3, S4's extension case, S5, S7 and the store package are the extension's own (Chrome APIs).
- S8 and S9 (fee) are on all four shells.

## For the owner

1. The category (Workflow & Planning vs Tools), visibility (Private / Unlisted), the trader phone number,
   and the four data-use judgement calls in the sheet §6.
2. Deploy getvela.app (the privacy text) before submitting — pre-flight P7.
3. One manual pass of the reviewer steps with a real passkey on the store zip — pre-flight P10.
4. S9's balance half (above).
5. Version: 0.9.6 chosen; 26.10.0 would start calendar versioning.
6. The GitHub-release zip (artifact `chrome-extension`, "Load unpacked") is still the development
   package (key + parallel space). Under the 2026-10-02 ruling it may need the store package's pruning too.
