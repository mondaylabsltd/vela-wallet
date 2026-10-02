# Research: 089 — the Chrome extension pass

## R1. How the pass was run

- **Build under test**: local branch `089-integration` = `origin/main` @ `67e2d193d` + `fix/issue-315-
  extension-follow-account` (PR #342) + `fix/issue-317-extension-sheet-locale` (PR #343); never
  pushed. Each fix branch is cut from `origin/main` and gated on its own.
- **Browser**: Playwright's Chrome for Testing 151 (`chromium-1234`), `--headless=new`, a throwaway
  profile per run, `viewport: null` so windows keep their real size.
- **The side panel** is not a Playwright `Page`. It was screenshotted at its real size (360 × 765)
  through the browser's own CDP endpoint (`--remote-debugging-port`, `Target.attachToTarget` +
  `Page.captureScreenshot`) — the first time the panel itself was captured rather than inferred.
- **The request window** opens 1440 wide under headless; its screenshots are taken at 420 × 728,
  the client area of the 420 × 760 popup `background.js` asks for.
- **Ports**: only 5186–5189 (dApp A, dApp B / hostile page / chaos proxy, stand-in chain, CDP). The
  suites hard-code 88xx; they were run from a port-shifted, gitignored copy (`.pw-out-e2e089/`),
  serially, with no web server.
- **Fixtures**: the parallel-space wallet (`en/parallel.html` → "Enter (seed fixture wallet)"), the
  repo's test dApp, the lifecycle suite's stand-in chain + relay (nothing leaves the machine for any
  signing flow). No real funds moved — the dust sends went to the stand-in relay.
- **Network faults**: `scripts/device/chaos-proxy.py` on 5187 (`CHAOS_UPSTREAM` = the Mac's own
  proxy), given to THAT browser only with `--proxy-server`; no system setting was touched.
- **Evidence**: screenshots and JSON rows under the session scratchpad `agent089/` (`shots/`,
  `functional.json`, `security*.json`, `chaos.json`, `visual-audit.json`), named in results.md.

## R2. Decisions

| # | Decision | Why | Rejected |
|---|---|---|---|
| D1 | A granted connect is answered by the WORKER (F02) | `dapp_browser` (every in-app browser) answers it instantly; the surface only asked the core and closed, flashing a window | Answer it in the surface without showing it — the window still has to open to ask |
| D2 | The worker mirrors grants + snapshot in memory (`grantMirror`) | Whether to open the panel must be decided synchronously: `sidePanel.open` only works inside the page's gesture, which the first `await` spends. Same pattern as `surfacePreference` | An async read first — every first connect would lose its gesture and fall back to a window |
| D3 | `instantConnectAnswer` is a twin of `decide_popup_request`'s connect branch, pinned by `instant.test.ts` over the grant matrix | The worker cannot run wasm; the rule stays the core's | A new core export for the worker — the worker cannot load it |
| D4 | One place says what a site may see: `grantedAccounts` (eth_accounts and the instant connect) | When #342 lands, its rule change is made once and both answers follow | Two copies of the snapshot → address mapping |
| D5 | The request surface sends the core `origin` + `params_json` (F11–F13) | `requested_address` and `is_insecure_public_origin` are the core's and `dapp_browser` already asks them; the surface's by-shape guess was a second rule that had drifted | A JS twin in the worker — the surface CAN run the core |
| D6 | The sheet's host WRAPS instead of start-ellipsizing (F14) | Every character of the origin stays on screen; an RTL/start ellipsis still hides part of it and is bidi-fragile with `:` and digits | `direction: rtl` ellipsis |
| D7 | The slide label wraps beside the knob; the track grows (F24) | Nothing of the label is hidden and its words do not change; a one-line ellipsis would drop the action ("Confirm send") in ru | Smaller type; dropping the hint at narrow widths |
| D8 | Font family tokens end in `system-ui, sans-serif` (mono: its mono stack) (F25) | docs/design-system.md: platform sans, never a serif, no downloaded CJK face | Adding 'Noto Sans SC' as `--font-ui` has (that one IS a downloaded face) |
| D9 | Keep `tabs` (F16) | Without it the worker cannot see its own tabs (measured: `tabs.query({url: ext/*})` → `[]`), so the toolbar could never reuse the wallet tab | `runtime.getContexts({contextTypes:['TAB']})` — possible, more change for no user-visible gain (the all-sites host permission already subsumes the install warning) |
| D10 | Not fixed here: request flood / join (F03), EIP-5792 status (F04), batch WYSIWYS (F28) | Each is a core rule shared by every shell, or a lifecycle change in 082's machinery; they need the owner's call and a spec of their own | — |

## R3. Permissions — each one, and why

| Entry | Needed for | Verdict |
|---|---|---|
| `storage` | grants, chain picks, the snapshot, the catalog, the request ledger (`storage.session`) | needed |
| `sidePanel` | the side panel surface (`sidePanel.open`) | needed |
| `tabs` | `tabs.query` by URL to reuse the wallet tab (own extension URLs are invisible without it — measured) | needed; justify in the store form |
| host `https://getvela.app/*` | lets extension pages use the rpId `getvela.app` for the passkey ceremony — the same wallet as the web (README constraint 3; `passkey.ts`) | needed |
| host `*://*/*` | the worker forwards reads to the catalog's nodes and bundler (any RPC a person added, CORS or not); `tabs.query` needs the tab URL to tell each origin's tabs their events; content scripts on every site | needed; broad — triggers the store's in-depth review; justify |
| content scripts `*://*/*`, `inpage.js` in `world: MAIN`, top frame only | the provider must be in every page a dApp can be, before its scripts run | needed; iframes get no provider (F09) |
| `web_accessible_resources: inpage.js` | nothing (a MAIN-world content script is injected, not fetched) | **removed** (F15) |
| CSP `script-src 'self' 'wasm-unsafe-eval'; object-src 'self'` | the core is wasm; no inline script (README constraint 1, 2) | needed, minimal |
| `externally_connectable` | — (absent: no page can message the worker; measured `chrome.runtime` undefined in pages) | keep absent |
| `storage.local` access level | Chrome's default opens it to content scripts; content.js never reads it | **closed** to them at worker start (F18) |
| `key` | pins the unpacked id for development and the e2e | see CWS |

## R4. Chrome Web Store readiness (no submission)

| Item | State | To do before a submission |
|---|---|---|
| `manifest_version` 3, `minimum_chrome_version` 116 | ok | — |
| `name` "Vela Wallet" (≤ 75), `description` 84 chars (≤ 132) | ok | keep "24 networks" true to the catalog |
| `version` 0.9.5 | ok | bump per release (the release gate already checks it) |
| Icons 16/32/48/128 in the manifest | ok | the store also needs a 128 × 128 store icon (96 px art + 16 px padding), 1–5 screenshots 1280 × 800 (or 640 × 400), a 440 × 280 small promo tile |
| `key` in the manifest | dev-only | strip it from the store build: the store assigns the item's id (the dashboard refuses or ignores a `key`; to verify on the first upload). Nothing in the product depends on the id: the rpId is `getvela.app` by host permission (`passkey.ts` test), and the e2e derives the id from `key` |
| Localized name/description | none (`default_locale` absent) | optional `_locales/<l>/messages.json` + `__MSG_…__`; note `package.test.ts` forbids top-level `_` names — `_locales` would need an exception. The store listing itself can be localized in the dashboard |
| Single purpose | a crypto wallet | state it |
| Permission justifications | R3 | paste R3's "needed for" column |
| Remote code | none: every script is in the package; wasm is packaged (`'wasm-unsafe-eval'` is allowed) | declare "No remote code" |
| Data use (privacy tab) | the extension stores locally: grants (site origin → address), chain picks, a public-address snapshot, the RPC catalog; it sends: reads to the RPC/bundler a person configured, the operation to the relay; bug reports send counters only (`vela.sw.counts`) | disclose "Financial and payment information" (transactions), "Website content"? no; "Web history"? no (origins stay local); link the privacy policy URL |
| `isMetaMask: true` on the legacy `window.ethereum` | documented compat choice; EIP-6963 announces the true name | be ready to explain it to a reviewer |
| Package size 37.6 MB (120 prerendered pages, 4.2 MB wasm) | ok (well under the store limit) | — |
| Console line on every page ("[Vela] EIP-1193/6963 provider installed") | noise in every site's console (F20) | consider removing |

## R5. The "Workers Builds: vela-wallet-web" failure (lead's extra item)

- Config: `app-web/vela-wallet/wrangler.jsonc` — Worker `vela-wallet-web`, `main`
  `.svelte-kit/cloudflare/_worker.js`, assets `.svelte-kit/cloudflare` (binding `ASSETS`),
  `workers_dev`, `preview_urls`. Cloudflare runs `pnpm build` in `app-web/vela-wallet`
  (runbook 05); production branch is `released` (spec 064 N1), so `main` and PR builds are
  non-production builds (`wrangler versions upload`).
- Reproduced from a FRESH export of each commit (`git archive` of the build's inputs, `pnpm install
  --frozen-lockfile`, `pnpm build`, then `wrangler deploy --dry-run` and `wrangler versions upload
  --dry-run`, no login): **both `892c4839e` (passed on Cloudflare) and `67e2d193d` (failed) build and
  dry-run cleanly and identically** — 2148 asset files (limit 20,000), largest asset the 4.2 MB wasm
  (limit 25 MiB), Worker 2650 KiB / 601.5 KiB gzip (limit 3 MiB gzip on Free, 10 MiB on Paid), build
  ~27 s. The only differences between the two commits inside the build's inputs are a few TS files
  and the wasm's fingerprinted name; the new `app-web/trusted-signer/dist/b/<hash>/sign.html` files
  are not part of this Worker's build at all.
- **Conclusion**: not a repository issue that a local build can see. The owner must read the failed
  build's log in the dashboard: Workers & Pages → `vela-wallet-web` → Deployments / Builds → the
  failed build. Check, in order: (1) **build minutes / concurrency** on the plan (Workers Builds
  has a monthly build-minute allowance and a concurrent-build limit per plan — check the current
  numbers in the dashboard; a run of PR preview builds can exhaust it, and every build after the
  cut-off then fails whatever its content, which matches "passed at 15:20, failed from 15:52 on, PRs
  too"); (2) Settings → Build → **API token** (the build token) still valid; (3) the
  **GitHub app's access** to the repository; (4) the build image / Node version setting (local:
  Node 22.22, pnpm 10.11.1 from `packageManager`); (5) the non-production deploy command
  (`npx wrangler versions upload`). No branch `fix/089-workers-build` was made.
