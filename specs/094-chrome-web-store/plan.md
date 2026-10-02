# Implementation Plan: 094 — the Chrome Web Store submission

**Branch**: `094-chrome-web-store` (from `origin/main` @ `ec033f231`) | **Date**: 2026-10-02 |
**Spec**: [spec.md](spec.md) | **Results**: [results.md](results.md)

## Summary

Make the packaged extension uploadable (a keyless store package beside the development one, a
self-sufficient build), write the submission sheet and render its images from the real build, and
close the should-dos the audit found — each rule in the core, each shell drawing it.

## Technical Context

- Extension: `app-web/vela-wallet/extension/` (manifest, `build.mjs`, `background.js`, `lib/*`,
  doorways); the wallet surfaces are the SvelteKit app; the provider is the core's
  `rust/crates/vela-core/provider/inpage.js`.
- Rules: `dapp_rpc` / `dapp_browser` (EIP-5792), `approval_guard` (the unlimited flag), `fee_policy`
  (the quote deadline). The MV3 worker keeps JS twins pinned to the core by tests.
- Shells touched: web + extension, desktop, iOS, Android (guard flag, fee timer).

## Design

| Item | Decision | Where |
|---|---|---|
| B1 | Every build derives `dist-store` from `dist`: manifest minus `key`, `STORE_PRUNE = ['parallel']` (page, boot scripts, data dir). `--zip` writes both zips. CI uploads `chrome-extension` and `chrome-web-store` | `extension/build.mjs`, workflow, `package.test.ts` |
| B2 | `build.mjs` runs `gen-tokens --check` and `sync-wasm` before `vite build`; the package test greps the built JS for `vela_core_bg.<hash>.wasm` and requires each in the package and the current `WASM_URL` among them | same |
| B3/B4 | One sheet; images by `scripts/store-art/` (wallet tab; side panel via CDP beside a neutral demo dApp at `https://swap.example`; icon and tile from the canonical mark) | `docs/store-submission/` |
| S1 | Manifest 122; esbuild target 122; install docs ×15 | manifest, site |
| S2 | `chrome.permissions.contains/request` for the two host permissions; `ExtensionNotices` in the root layout; `SecurityError` in the packaged app → `SITE_ACCESS_WITHHELD` detail → corpus sentence | `src/lib/extension/*`, `passkey.ts`, `copy.ts` |
| S3 | `runtime.onInstalled` (install only) → `open.html?installed=1` when web tabs are open; `open.js` carries the mark in `sessionStorage`; the notice shows it once | worker, `lib/locales.js`, `open.js` |
| S4 | `location.assign(packagedHref(welcome))` | settings page |
| S5 | The window reads the session's `allowed_route`; not `wallet` → "Create a wallet first" + Create / Sign in (tabs opened in the last normal window); the session follows the other document's sign-in | `DappRequestHost.svelte`, `open-tab.ts` |
| S6 | Core: `Route::{Capabilities, CallsStatus}`, `calls_status`, `capabilities`, `UNKNOWN_BUNDLE_ID`; `dapp_browser` answers both (a CallsStatus read maps the bundler body); wasm exports; worker twins over the op record (RF3) | core, wasm, `protocol.js`, `background.js` |
| S7 | `windowHolder` / `windowOpening` decided before the first await; `releaseWindow` hands the window to the next queued request (navigates its tab) or closes it; queued connects answered from the fresh grant; a closed window settles its site's queue; restart promotes | `request-life.js`, `background.js`; page close becomes a backstop |
| S8 | `GuardView.unlimited_warning` = kept unlimited ∨ batch `any_uncapped` ∨ unbounded permit; every shell reads only it; permits keep no cap editor (the dApp redeems its own struct) and every shell says so | core + four shells |
| S9 | `QUOTE_DEADLINE_MS` 15 s, `StartDeadline` / `DeadlineElapsed`; each executor a timer; web bounds the pre-quote deployment read by the same figure | core + four shells |
| S10 | Privacy page text (English-only page) | site |
| S11 | 0.9.6 on all four shells | manifest, Cargo, gradle, pbxproj |
| S12 | Local runner `.pw-out-094/ext.config.ts` (no web server, one worker, Chrome for Testing, throwaway profiles); harness closes the install tab, finds a keyless id | `e2e/` |
| NICE | Provider console line removed (core test); `incognito: not_allowed`; parallel pruned from the store package | core, manifest, build |

## Not done here (see results.md)

- S9's balance half — the last known balance after an offline reload — needs a new shell fact (which
  chains answered) or a persisted holdings cache in all four shells, and an owner call on what a
  brand-new wallet with one dead network shows.
