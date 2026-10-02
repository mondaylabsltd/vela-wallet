# Results: 089 — the Chrome extension pass

**Status**: pass done 2026-10-01 on `089-integration` (`origin/main` @ `67e2d193d` + PR #342 + PR #343,
local, never pushed). Nine fix branches, each from `origin/main`, each gated on its own; none pushed.
All of them merged together with #342/#343 in a local `089-integration-fixes`: **every extension
e2e green (47/47)**, web unit green, svelte-check 0 errors. Evidence: [evidence/](evidence/) (curated
before/after shots and the JSON rows of each probe); the full 101-shot set stayed in the session
scratchpad.

## Success criteria

| SC | Verdict | Evidence |
|---|---|---|
| SC-001 extension e2e green on `main` | **Pass** with `fix/089-lifecycle-e2e`: 35/35 on `main` (was 32/35) | gates on the branch |
| SC-002 a connected site's repeat connect opens nothing | **Pass** with `fix/089-instant-connect`: 0 windows, 0 panels (was a window per call) | `extension-connect` e2e; `reshoot.json` (panel: `newPages: 0`) |
| SC-003 other-account transaction / public-http signature: 0 sheets, 4100 | **Pass** with `fix/089-popup-sign-gate` | `extension-signing` e2e ×2; `evidence/F11-*`, `F12-*` (before) |
| SC-004 360 px, en/zh/ru × light/dark: nothing clipped, nothing under the knob, no serif | **Pass** with the three sheet branches | `evidence/F14-*`, `F23-F24-*`, `F25-*` before/after |
| SC-005 no regression | **Pass** — per-branch numbers below | — |

## Findings

Severity: S1 money can move unseen · S2 security / a wrong answer a person or dApp acts on · S3 broken
or misleading experience · S4 polish / hygiene / information.

| ID | Sev | Area | Finding | Evidence | Status / branch |
|---|---|---|---|---|---|
| F01 | S2 | e2e | The extension e2e suite is red on `main`: the three G35 lifecycle cases expect `{ok, result: <op hash>}`; since 083 the worker (rightly) answers -32603 "not confirmed yet" | `e2e-int-baseline` run, 3 ✘ | **fixed** `fix/089-lifecycle-e2e` |
| F02 | S3 | connect | A connected site's `eth_requestAccounts` / `wallet_requestPermissions` went to a surface that only asked the core: a request window flashed open/shut and took focus, or the side panel opened — on every call. Every in-app browser answers it instantly | `functional.json` ("flashed": true) | **fixed** `fix/089-instant-connect` |
| F03 | S3 | lifecycle | Requests with no user gesture each open their own window: 12 concurrent `eth_requestAccounts` → 12 focused popups. `dapp_browser`'s rule: the same origin JOINS the one consent; another origin is told no | `security.json` (flood: 12) | **open** — owner: a join/queue for the window surface (082 lifecycle) |
| F04 | S2 | EIP-5792 | `wallet_sendCalls` is supported but `wallet_getCallsStatus` / `wallet_getCapabilities` answer 4200 — in the extension AND every in-app browser (`dapp_rpc::classify` → Unsupported). viem/wagmi `waitForCallsStatus` fails after a batch that went through | `functional.json` | **open** — owner: a core route + the worker's twin (`dapp-submit.ts` already has the WalletPair logic) |
| F05 | S3 | reads | With every node of a chain unreachable, the first read waits 3 × 8 s = 24 s, then a plain "Vela could not reach a node for chain Gnosis (100)"; the next ~6 s; recovers without a reload; `eth_chainId`/`eth_accounts` stay instant | `chaos.json` | **open** (by RF2's design; a total budget would cap it) |
| F06 | S3 | sheet, offline | With the network dropped, the send sheet's fee stays "Estimating…" (15 s+) with no failure words; the slide is (rightly) disabled | `evidence/F06-offline-fee-estimating.png` | **open** — shared sheet |
| F07 | S3 | wallet, offline | The wallet reloaded during an outage shows **$0.00** and "Deposit your first asset" under a "24 networks RPC unavailable" note | `evidence/F07-offline-wallet-zero.png` | **open** — shared wallet; check against the cached-balance rule |
| F08 | S4 | `wallet_watchAsset` | answers `false` (tokens are added in the wallet) — by design | `functional.json` | info |
| F09 | S4 | frames | No provider in iframes (`all_frames: false`); a cross-origin frame's channel messages reach nothing (good); dApps embedded in frames cannot use Vela | `security.json` | info |
| F10 | S3 | toolbar | With no wallet tab, the toolbar reused the REQUEST popup: navigated the pending request away (4900) and put the wallet in a 420 px popup (`tabs.query` returned only the popup) | `089-g-toolbar` row | **fixed** `fix/089-toolbar-tab` |
| F11 | S2 | sign gate | `eth_sendTransaction` / `wallet_sendCalls` whose `from` is another account was not refused: the sheet offered to send it from the granted account (the surface's by-shape pin never saw object params; the core's invariant says 4100) | `evidence/F11-before-send-from-other-account.png` | **fixed** `fix/089-popup-sign-gate` |
| F12 | S3 | sign gate | A public plain-http origin could ask for signatures (sheet shown); every in-app browser refuses: 4100 "Signing requires a secure origin" | `evidence/F12-before-insecure-origin-sign.png` | **fixed** `fix/089-popup-sign-gate` |
| F13 | S4 | sign gate | `personal_sign` of a 20-byte hex message was read as the account (by shape) and refused for the right account | core test | **fixed** `fix/089-popup-sign-gate` |
| F14 | S2 | origin display | The sheet header end-ellipsized long hosts: "app.uniswap.org.se…" — the registrable domain, the part that says who asks, was the part not drawn (panel and window) | `evidence/F14-before-*`, `F14-…-after-*` | **fixed** `fix/089-origin-never-truncated` |
| F15 | S4 | manifest | `inpage.js` web-accessible to every site (a free "is Vela installed?" probe at the pinned id) though nothing fetches it | `security.json` (fetch → 200) | **fixed** `fix/089-manifest-war` |
| F16 | info | manifest | `tabs` IS needed (without it the worker cannot see its own tabs); `*://*/*` needed (RPC forwarding, tab origin matching); `https://getvela.app/*` needed (rpId) — research.md R3 | `security-tabs.json` | keep, justify |
| F17 | info | isolation | No `externally_connectable` (`chrome.runtime` absent in pages); extension pages neither fetchable nor frameable; forged origin → true origin named; replayed id → one answer; IDN shown as punycode; CSP minimal | `security.json`, existing `extension-security` | holds |
| F18 | S4 | storage | `storage.local` (grants, signed-in address, RPC catalog incl. a person's own endpoints) readable by content scripts by default — i.e. by a compromised renderer | `extension-security` e2e (before: "read") | **fixed** `fix/089-storage-trusted` |
| F19 | S4 | consent | The Connect button is live the moment the card appears (a double-click on a page can land on it). Only the address leaks; every signature needs the slide AND the passkey | reasoning | info |
| F20 | S4 | console | Every page's console says "[Vela] EIP-1193/6963 provider installed" | any page | open (polish) |
| F21 | info | compat | `isMetaMask: true` on the legacy `window.ethereum` (EIP-6963 announces the truth) — a deliberate choice; be ready to explain it to a store reviewer | inpage.js | info |
| F22 | S3 | approval guard | Permit2 `PermitSingle` for the max amount: red "−Unlimited" hero, but no warning banner and no cap editor (an ERC-20 `approve` gets both) | `evidence/F22-permit2-unlimited.png` | **open** — shared guard |
| F23 | S2 | sheet @ 360 | The full recipient / spender address was clipped at the panel's edge in every locale (420 px too at the largest text) — its LAST characters, the ones checked against address poisoning | `evidence/F23-F24-before-*` / `after-*` | **fixed** `fix/089-sheet-narrow-fit` |
| F24 | S3 | sheet @ 360 | The slide label was centred across the track: under the knob in en at 360 px; ru's two lines hid their first letters | same | **fixed** `fix/089-sheet-narrow-fit` |
| F25 | S3 | fonts | Cyrillic in `--font-numeric` / `--font-display` (the hero "−Без лимита", "Одобрить") fell back to Times: the family tokens named one face, no fallback | `evidence/F25-before-*` / `after-*`; `reshoot.json` (computed families) | **fixed** `fix/089-font-fallback` |
| F26 | S3 | request window | Two dialects in one 420 × 760 window: the consent card is a bare page (no site mark, no account, a large empty middle), while signing sheets float as a bottom sheet over an empty grey scrim | `evidence/F26-*` | **open** — owner: design |
| F27 | S4 | consent | The consent card names neither the account the site will see nor the network (the desktop's consent gained both in 083 W14) | `evidence/F26-*` | **open** — design (drawable without new strings: avatar + name + short address) |
| F28 | **S1** | batch | A `wallet_sendCalls` batch is described by its FIRST call only — on every client (`first_call`, "a bundle decodes from its first leg"): [1 wei → A, 1 xDAI → B] reads "Send −0.000000000000000001 xDAI, Recipient A"; the 1 xDAI to B is only in the raw JSON under Technical details. A page can put a harmless call first | `evidence/F28-*` | **open** — owner: every shell's clear signing (a core summary of every leg) |
| F29 | S4 | origin rule | The core's insecure-origin rule exempts `localhost` and `.local` but not `*.localhost` (which Chrome always resolves to loopback): a dev dApp at `http://app.localhost` can no longer get a signature in the extension, as in every in-app browser. Left as is: not every shell's resolver hard-wires `*.localhost` | reshoot (the long-host shot moved to `.local`) | info |
| F30 | info | release | "Workers Builds: vela-wallet-web" failing since `67e2d193d`: not reproducible from the repo — research.md R5 lists what to check in the dashboard | research.md R5 | owner |

## Branches

All from `origin/main` @ `67e2d193d`; commit messages `fix: … (089)`; none pushed.

| Branch | Commit(s) | Fixes | New tests |
|---|---|---|---|
| `fix/089-lifecycle-e2e` | `2e786ec96` | F01 | — (3 e2e corrected) |
| `fix/089-instant-connect` | `cdd2b5644` | F02 | background +5, instant +12, e2e +1 |
| `fix/089-popup-sign-gate` | `0d6f2eef1` | F11, F12, F13 (core + wasm + TS mirrors) | core +5, web +5, e2e +2 |
| `fix/089-origin-never-truncated` | `a9b0348c0`, `d6a3f5e71` | F14 | browser +3 |
| `fix/089-sheet-narrow-fit` | `e66ea58c0` | F23, F24 | browser +6 |
| `fix/089-font-fallback` | `ea3f22d19` | F25 | tokens +1 |
| `fix/089-toolbar-tab` | `da44efea6` | F10 | background +2 |
| `fix/089-manifest-war` | `8731ff460` | F15 | package +1, e2e +1 |
| `fix/089-storage-trusted` | `1e2705131` | F18 | background +2, e2e +1 |

Per-branch gates (`npx vitest run`; `npx svelte-check --threshold error`; `pnpm build:extension`;
every `e2e/extension-*.e2e.ts`, port-shifted, serial). Every branch but the first still carries the
three F01 failures, which `fix/089-lifecycle-e2e` removes:

| Branch | vitest (files / passed / skipped) | svelte-check | extension e2e |
|---|---|---|---|
| `fix/089-lifecycle-e2e` | 157 / 2238 / 5 | 0 errors | **35 / 35** |
| `fix/089-instant-connect` | 157 / 2255 / 5 | 0 errors | 33 + the 3 F01 (36) |
| `fix/089-popup-sign-gate` | 158 / 2243 / 5 ¹ | 0 errors | 34 + 3 F01 (37) |
| `fix/089-origin-never-truncated` | 158 / 2241 / 5 | 0 errors | 32 + 3 F01 (35) |
| `fix/089-sheet-narrow-fit` | 159 / 2244 / 5 | 0 errors | 32 + 3 F01 (35) |
| `fix/089-font-fallback` | 157 / 2239 / 5 | 0 errors | 32 + 3 F01 (35) |
| `fix/089-toolbar-tab` | 157 / 2240 / 5 | 0 errors | 32 + 3 F01 (35) |
| `fix/089-manifest-war` | 157 / 2239 / 5 | 0 errors | 33 + 3 F01 (36) |
| `fix/089-storage-trusted` | 157 / 2240 / 5 | 0 errors | 33 + 3 F01 (36) |

¹ The first run, with the core's `cargo test` compiling on the same machine, had one timing failure
in an unrelated drag test (`BottomSheet.svelte.test.ts` "resists a long drag"); it passed 25/25
twice alone, and the whole suite passed on the re-run (158 / 2243).

Core (`fix/089-popup-sign-gate`): `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures`
2244 passed / 0 failed; `cargo test -p vela-core --features i18n-all,crux` 2201 / 0; clippy
`--workspace --all-targets -D warnings` clean; `cargo fmt --check` clean; `build-web --check` current;
`gen-core-types --check` current.

Integration (`089-integration-fixes`, local): vitest 162 files / 2306 passed / 5
skipped; svelte-check 0 errors, 0 warnings; **extension e2e 47 / 47** (with #342's
`extension-follow-account` and #343's `extension-locale`); core `app_dapp_permissions` 31 / 31;
`build-web --check` current. The fixed surfaces were shot again on it (`evidence/*-after-*`,
`reshoot.json`): the long host wrapped at its own seams (dots, hyphens, the port's colon), the whole address, the slide label beside
its knob in en and ru, the ru hero in sans; a connected site's repeat connect opened 0 pages.

## What needs the owner

1. **F28 (S1)** — a batch's sheet must describe every leg. Cross-shell clear-signing work.
2. **F04 (S2)** — `wallet_getCallsStatus` / `wallet_getCapabilities` for the extension and the in-app
   browsers (a core route; the worker's twin).
3. **F03 (S3)** — a request flood opens a window per request; adopt `dapp_browser`'s join rule.
4. **F22, F06, F07** — the shared approval guard and offline states.
5. **F26, F27** — the request window's design (one dialect; account and network on consent).
6. **R5** — the Cloudflare dashboard checks for "Workers Builds: vela-wallet-web".
7. Merge order: the plan's merge notes; `popup-sign-gate` and #342 both rebuild the wasm — rebuild it
   after the merge, never hand-merge `assets/wasm` or `rust/pkg-web`.
