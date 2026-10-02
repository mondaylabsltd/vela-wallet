# The Chrome extension (spec 027)

This directory is the MV3 artifact: the scripts that live in a web page and the
manifest that installs them. It is **not** a second wallet — it packages this
app's own client build (`build.mjs` assembles `dist/`), so everything the wallet
knows and decides comes from `src/`, unchanged.

It lives inside the app rather than beside `packages/safari-extension` because
it is a build target of THIS package: `pnpm build:extension` is its script, it
shares one package manager, one lint config and one gate suite, and it has no
life apart from app-web. The Safari extension is a genuinely separate artifact —
it talks to a native iOS app over `nativeMessaging` — and stays where it is.

These files sit outside `src/` on purpose: they are not SvelteKit modules and
must never be bundled by it. The page-side provider (`inpage.js` in `dist/`)
runs in the page's MAIN world; its source is not in this folder but in the
core crate, `rust/crates/vela-core/provider/inpage.js` (spec 070), because the
desktop, iOS and Android in-app browsers inject the very same bytes.

## Five measured constraints this directory has to respect

They are recorded with their evidence in [`research.md`](../../../specs/027-web-extension-provider/research.md)
(D31, D33, D34, D35); the short version, because breaking any of them is silent:

1. **The manifest must declare `'wasm-unsafe-eval'`** in
   `content_security_policy.extension_pages`. Under MV3's default CSP,
   `WebAssembly.compile` fails outright — and every decision this product makes
   lives in that binary.
2. **No inline `<script>` in any extension page**, and a `'sha256-…'` hash is not
   an escape hatch — Chrome refuses to load the extension at all. This is why
   `dist/` carries a client-rendered shell rather than the site's prerendered
   pages.
3. **`host_permissions` must include `https://getvela.app/*`.** That entry is
   what lets the passkey ceremony run under the hosted site's relying party,
   which is what makes the extension the SAME wallet at the same address. The
   packaged app always asks for `getvela.app` (`passkey.ts`), so without the
   entry — or when a person withholds it in Chrome's "Site access" — every
   ceremony fails with a `SecurityError`; the wallet says so in plain words and
   offers the one-click grant (`ExtensionNotices`, spec 094). Chrome allows an
   extension page this relying party only from Chrome 122
   (`minimum_chrome_version`).
4. **A dApp request opens the asking tab's SIDE PANEL, or a dedicated window —
   never the action popup.** The popup closes when the passkey prompt takes
   focus, mid-signature; the side panel and a window both survive it.
   `chrome.sidePanel.open` needs a user gesture, and the gesture travels with
   the page's message only until the worker's first `await` — so the panel is
   opened synchronously in the message listener, and a request a page fired
   without a click (no gesture) falls back to the window. The panel is the
   wallet (`wallet.html?panel`) with the request raised over it; the window
   shows `request.html`. Each enters through a doorway — the panel through
   `panel.html`, because `side_panel.default_path` is one static path and the
   pages are per locale; the window (and the toolbar's tab) through
   `open.html`, because the worker cannot read the language the person pinned
   (issue 317). Both doorways pick the locale by the one rule in
   `lib/locales.js`.
5. **No top-level name in `dist/` may start with `_`.** Chrome reserves that
   prefix and rejects the whole package — "Cannot load extension with file or
   directory name \_app … Could not load manifest." SvelteKit's `kit.appDir`
   defaults to `_app`, so `vite.config.ts` sets it to `app` for this target
   only. The automated suite cannot stand in for a real install here:
   Playwright's `--load-extension` loaded the reserved name happily while
   "Load unpacked" refused it, so `package.test.ts` asserts it directly.

## Layout (as phases land it)

```
manifest.json      the five constraints above, plus a pinned id (`key`) — kept
                   in the development package only (below)
icons/             the toolbar and store icons, rendered from docs/design/icon/app-icon.svg
(inpage.js)        MAIN world: the provider — bundled from rust/crates/vela-core/provider/inpage.js
content.js         isolated world: the page bridge
background.js      the service worker: routing, the per-site chain, reads
                   forwarded verbatim, and the page events — no authoritative state
panel.html/.js     the side panel's doorway: picks the locale, opens the wallet
open.html/.js      the request window's and the wallet tab's doorway: picks the
                   locale the same way (lib/locales.js), opens the page
lib/protocol.js    the message shapes both sides agree on
lib/locales.js     the packaged locales, and the ONE rule every surface opens by:
                   the pinned language, else Chrome's (the worker never picks)
build.mjs          assembles the app's client build + these scripts into dist/,
                   then derives dist-release/ and dist-store/; `--zip` zips them
dist/              the DEVELOPMENT package: `key` (the e2e computes the id from
                   it) and the parallel space — gitignored
dist-release/      the GITHUB RELEASE package ("Load unpacked"): `key` kept, so
                   a tester's id stays the same; no developer pages (owner
                   ruling 2026-10-02) — gitignored
dist-store/        the CHROME WEB STORE package: no `key` (the store assigns the
                   id and refuses an upload carrying one), no developer pages
                   — gitignored
```

`pnpm package:extension` builds all three and zips the two release packages
beside `package.json`: `vela-wallet-extension-<version>.zip` (the GitHub
release) and `vela-wallet-extension-<version>-chrome-web-store.zip` (the
upload); `src/lib/extension/package.test.ts` reads all three. The submission
sheet is `docs/store-submission/chrome-web-store.md`.

A fresh install opens the wallet's welcome in a tab (`runtime.onInstalled`).
Chrome injects content scripts only into pages loaded after the install, and
this extension does not ask for `scripting`, so the welcome asks the person to
reload the pages that were already open (spec 094).

## What the worker answers, and from where

| Method                                                                 | Answered by                                                                                    |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `eth_accounts`, `eth_chainId`, `net_version`, `wallet_getPermissions`  | the wallet's snapshot + the site's grant and chain pick                                        |
| `eth_requestAccounts`, `wallet_requestPermissions`                     | granted: the worker (089); else the side panel (or window): `dapp_permissions`                 |
| `personal_sign`, typed data, `eth_sendTransaction`, `wallet_sendCalls` | the side panel (or window): `sign_request`, on the site's chain                                |
| `wallet_switchEthereumChain`, `wallet_addEthereumChain`                | the worker, against the catalog the wallet published (`vela.ext.chains`); unknown chain → 4902 |
| `wallet_watchAsset`                                                    | `false` — tokens are added in the wallet                                                       |
| `wallet_getCapabilities`, `wallet_getCallsStatus` (EIP-5792)           | the worker: twins of `dapp_rpc::capabilities` / `calls_status` (094), the op hash's receipt    |
| node and bundler reads (allowlist in protocol.js)                      | forwarded to the catalog's endpoints for the site's chain                                      |
| anything else                                                          | 4200                                                                                           |

A request answered in a WINDOW (no gesture to open the panel on) gets one
window per site: requests of the same origin queue behind it and the window
passes from one to the next; connects the person's decision already answered
are answered without one; closing the window settles its site's queue (094).

The page hears `accountsChanged` / `chainChanged` / `disconnect` when the
site's grant or chain pick changes in storage — on connect, when the wallet
switches accounts (the core re-pins the grant), on revoke, on a switch — and
`accountsChanged` when the wallet signs out or back in (its snapshot).

A grant is answered only while its account is the one the wallet is signed in
to (`granted_to_signed_in`, spec 086, issue 315): `eth_accounts` reads the
snapshot's `address`, and no snapshot — the wallet removes it on sign-out —
is nobody.
