# Implementation Plan: 089 — the Chrome extension pass

**Branch**: `089-chrome-extension-pass` (docs) | **Date**: 2026-10-01 | **Spec**: [spec.md](spec.md) |
**Research**: [research.md](research.md) | **Results**: [results.md](results.md)

## Summary

A pass over the MV3 extension as a dApp meets it — functional, hostile, at its real sizes, and as a
store reviewer would read it — on `origin/main` + PRs #342/#343. Every defect with a small,
root-caused fix gets its own branch from `origin/main`; the rest are written down for the owner.

## Technical Context

- Extension: `app-web/vela-wallet/extension/` (manifest, `background.js` worker, `content.js`,
  `lib/protocol.js` twins, `lib/request-life.js`, doorways `panel.js` / `open.js`); the provider is
  the core's `rust/crates/vela-core/provider/inpage.js`. The wallet surfaces are the SvelteKit app
  (`src/lib/dapp/*`, `src/lib/signing/*`), packaged by `extension/build.mjs`.
- Rules: the core decides (`dapp_permissions`, `dapp_rpc`, `sign_request`); the worker keeps JS
  twins pinned by `instant.test.ts` / `core-table.test.ts` / `protocol.test.ts`.
- Gates per branch: `npx vitest run` (server + browser), `npx svelte-check --threshold error`,
  `pnpm build:extension`, every `e2e/extension-*.e2e.ts` (port-shifted copy, ports 5186–5189);
  core branch also `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures`,
  `cargo test -p vela-core --features i18n-all,crux`, clippy `-D warnings`, `cargo fmt --check`,
  `build-web --check`, `gen-core-types --check`.

## Design — one branch per concern

| Branch | Concern | Layer |
|---|---|---|
| `fix/089-lifecycle-e2e` | the three G35 e2e cases assert 083's "not confirmed yet" | e2e + comments |
| `fix/089-instant-connect` | a granted connect is answered by the worker; `grantMirror`; `instantConnectAnswer` twin | worker + protocol twin + unit + e2e |
| `fix/089-popup-sign-gate` | `PopupRequest` gains `origin`, `params_json`; `requested_address`; `InsecureOrigin` | core + wasm + TS mirrors + surface + e2e |
| `fix/089-origin-never-truncated` | the sheet header wraps the host | `SigningHeader.svelte` + browser test |
| `fix/089-sheet-narrow-fit` | whole address; slide label beside its knob | `PartyRow`, `SlideToConfirm` + browser tests |
| `fix/089-font-fallback` | font family tokens end in the platform sans | `gen-tokens.mjs` + generated CSS + test |
| `fix/089-toolbar-tab` | the toolbar reuses only a normal window's tab | worker + unit |
| `fix/089-manifest-war` | nothing web-accessible | manifest + package test + e2e |
| `fix/089-storage-trusted` | `storage.local` closed to content scripts (`setAccessLevel`) | worker + unit + e2e (isolated world over CDP) |

## Merge notes (for the owner)

The branches touch some of the same files as PRs #342 / #343 and as each other; the conflicts are
textual and small, and were resolved once in a local `089-integration-fixes` branch (never pushed)
on which every extension e2e and the web unit suite were run (results.md):

- `instant-connect` × #342: `background.js` `answerFromSnapshot` → keep 089's `grantedAccounts()`
  and put #342's rule inside it (`resolveGrantedAccounts(grant, signedInOf(snapshot))`); the storage
  listener keeps both 089's `noteGrantedChange` line and #342's `EXT_CACHE_KEY` branch;
  `instant.test.ts` takes #342's matrix and question shape (`signedIn`) for 089's connect loop.
- `popup-sign-gate` × #342: `dapp_permissions.rs` `PopupRequest` keeps #342's `signed_in` and adds
  089's `origin` / `params_json`; `popup_request` takes #342's `granted_to_signed_in`; the wasm and
  `pkg-web` are rebuilt after the merge (`node rust/scripts/build-web.mjs`), never hand-merged.
- `toolbar-tab` × #343: `openWallet` keeps #343's `openDoor()` URL and 089's `windowType: 'normal'`.
- `instant-connect` × `toolbar-tab`: both in `background.js`, different functions — no conflict.
- `instant-connect`, `toolbar-tab`, `storage-trusted` each add a `describe` block before "the page's
  own deadline" in `background.test.ts`; `manifest-war` and `storage-trusted` each add a test before
  "cannot use the wallet as an open RPC relay" in `extension-security.e2e.ts` — keep every block.
- **A merge that does NOT conflict and is still wrong**: `instant-connect` × #342 auto-merges
  `grantedAccounts` with the OLD rule (the snapshot's account list). Its body must become #342's
  `resolveGrantedAccounts(grant, signedInOf(snapshot))` — otherwise a granted site's repeat connect is
  answered by the pre-#315 rule. Likewise `instant.test.ts`'s connect loop must take #342's
  `signedIn` question shape (a type error flags it).
