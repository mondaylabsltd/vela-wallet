# Results: 094 — the Chrome Web Store submission

**Status**: done on `094-chrome-web-store` (from `origin/main` @ `ec033f231`), not pushed.
**The upload**: `app-web/vela-wallet/vela-wallet-extension-0.9.6-chrome-web-store.zip` — built from
`b04dc0943` (the commits after it are docs only), 23,084,462 bytes, sha256
`e8884fa8e70c7cdf042e550ae21eea98cf835f9ecacc822326aa33bcc0102865`. The GitHub-release zip beside it,
`vela-wallet-extension-0.9.6.zip`: 23,084,811 bytes, sha256
`7a5939b0451b8c5d8139903ca0c97234cf4366eebb7e47d0c7a01be553ecd58d` (`key` kept, no parallel space).
Rebuild with `pnpm package:extension` in `app-web/vela-wallet` (the zips are gitignored build output).

## The store zip, verified

| Check | Result |
|---|---|
| `manifest.json` at the root | yes |
| `key` | absent (the development zip keeps it) |
| `version` / `minimum_chrome_version` / `incognito` | 0.9.6 / 122 / `not_allowed` |
| permissions / hosts | `storage, tabs, sidePanel` / `https://getvela.app/*, *://*/*` (unchanged) |
| developer pages (`parallel`, `gallery`, `dev/`) | 0 files |
| the wasm the code names (`WASM_URL` `/vela_core_bg.83692ef6d37b.wasm`) | present, 4,276,822 B |
| unpacked size | 37.3 MB |
| installed as is (`e2e/extension-store-package.e2e.ts`) | id assigned by Chrome, welcome opens, wasm compiles in the extension page, provider announced as "Vela Wallet"/`app.getvela`, no console line, EIP-5792 answered, site-access grant, install note — 6/6 |

## Items

| Item | Status | What / where |
|---|---|---|
| B1 store zip without `key` | done | `extension/build.mjs` derives `dist-release` (the GitHub release's zip: `key` kept, no parallel space — follow-up 3) and `dist-store` (no `key`, no parallel space); CI uploads `chrome-extension` + `chrome-web-store`; package test covers all three packages |
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
| S11 version | done | 0.9.6 on all four shells (lead, follow-up: keep 0.9.6, no calendar versioning now) |
| S12 e2e | done | 59/59 on the final build; new: store package ×6, typed data v4, connect with no wallet, connect burst, a send that lands, a batch's status, erase |
| NICE console line | done | removed from the core provider (+ test) |
| NICE incognito | done | `not_allowed` |
| NICE parallel space | done | pruned from both release packages (store and GitHub release); the development / e2e package keeps it |

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

| Suite | Result (after the follow-ups) |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,325 passed, 0 failed |
| core clippy `--workspace --all-targets -D warnings` / `fmt --check` | clean / clean |
| i18n gen / lint / verify / dump | ok; verify 75,515 comparisons, 0 divergences |
| residency (ja + en) | 139,394 B (budget 141,800); the four strings are 742 B of ja + en JSON |
| `build-web --check` / `gen-onboarding-types --check` | current / current |
| web `npx vitest run` | 171 files, 2,464 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| extension package test | 38/38 (development, release and store packages) |
| extension e2e (every `e2e/extension-*.e2e.ts`, Chrome for Testing, throwaway profiles) | 59/59 on `b04dc0943` (a run at load average ~60 timed one discovery test out at 120 s; it passed 3/3 alone and the full rerun at normal load was 59/59) |
| desktop fmt / clippy / test | clean / no new warnings / 880 passed, 49 ignored |
| Android `testDebugUnitTest` | 911 passed, 0 failed |
| iOS `VelaWalletTests` (iOS 26.2 simulator) | 1,101 tests in 141 suites, all passed |
| `check-native-reachability` / `check-event-payloads` / `check-dead-controls` | clean / 0 mismatches / 0 dead controls |

**iOS**: the four feedback suites crash on an iOS 17.5 simulator on `origin/main` and on this branch
alike, and pass on iOS 26.2 on both — see follow-up 4.

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

## Follow-ups (lead, 2026-10-02 evening)

1. **Strings trimmed**, all 15 locales, plainer: ja + en JSON of the four strings 1,037 → 742 B (−295 B);
   the residency test's runtime ja + en is now **139,394 B** (was 139,689) against the budget the owner
   raised to **141,800**.
2. **Version** stays 0.9.6.
3. **GitHub-release zip** (`vela-wallet-extension-0.9.6.zip`, artifact `chrome-extension`) is now
   `extension/dist-release`: `key` kept (a tester's id stays the same), no parallel space. The
   development / e2e package keeps everything.
4. **The four iOS feedback suites** (`FeedbackScreenshotTests`, `ScreenshotViewerTests`,
   `BugReportTests`, `SettingsFeedbackRowTests`): the crash is the SIMULATOR'S RUNTIME, not the disk and
   not 094. They crash identically on `origin/main` @ `7392b9b61` and on this branch on an **iOS 17.5**
   simulator (EXC_BAD_ACCESS in "type metadata accessor for nonisolated(nonsending) ()" called from
   `FeedbackSender.add(datas:)`), and pass **62/62 on both** on an **iOS 26.2** simulator. The first
   runs used an iPhone 15 Pro clone on iOS 17.5. The app's deployment target is iOS 17.4, so this
   very likely affects real iOS 17 devices too: any path that materialises
   `nonisolated(nonsending) () async -> Data?` metadata — `FeedbackSender.attach` / `add`, i.e.
   attaching a screenshot to a report — would crash there (not verified on a device).
5. Main not merged.

## For the owner

1. Decided 2026-10-02 and recorded in the sheet: category Productivity › Workflow & Planning;
   visibility Unlisted; J1 not ticked, J2 Yes, J3 no extra tick, J4 note kept; Non-trader.
2. Deploy getvela.app (the privacy text) before submitting — pre-flight P7.
3. One manual pass of the reviewer steps with a real passkey on the store zip — pre-flight P10.
4. S9's balance half (above).
5. The iOS 17 runtime crash above (pre-existing on main).
