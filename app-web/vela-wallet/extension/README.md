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
must never be bundled by it. `inpage.js` in particular runs in the page's MAIN
world, where a bundler's module wrapper would be a bug.

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
   which is what makes the extension the SAME wallet at the same address. Remove
   it and the extension quietly becomes a different, empty wallet.
4. **A dApp request opens the asking tab's SIDE PANEL, or a dedicated window —
   never the action popup.** The popup closes when the passkey prompt takes
   focus, mid-signature; the side panel and a window both survive it.
   `chrome.sidePanel.open` needs a user gesture, and the gesture travels with
   the page's message only until the worker's first `await` — so the panel is
   opened synchronously in the message listener, and a request a page fired
   without a click (no gesture) falls back to the window. Both show the same
   `request.html`; the panel enters it through `panel.html`, because
   `side_panel.default_path` is one static path and the pages are per locale.
5. **No top-level name in `dist/` may start with `_`.** Chrome reserves that
   prefix and rejects the whole package — "Cannot load extension with file or
   directory name \_app … Could not load manifest." SvelteKit's `kit.appDir`
   defaults to `_app`, so `vite.config.ts` sets it to `app` for this target
   only. The automated suite cannot stand in for a real install here:
   Playwright's `--load-extension` loaded the reserved name happily while
   "Load unpacked" refused it, so `package.test.ts` asserts it directly.

## Layout (as phases land it)

```
manifest.json      the five constraints above, plus a pinned id (`key`)
icons/             the toolbar and store icons, rendered from docs/design/icon/app-icon.svg
inpage.js          MAIN world: the provider, its announcement, the legacy shim
content.js         isolated world: the page bridge
background.js      the service worker: routing, the per-site chain, reads
                   forwarded verbatim, and the page events — no authoritative state
panel.html/.js     the side panel's doorway: picks the locale, opens request.html
lib/protocol.js    the message shapes both sides agree on
lib/locales.js     the packaged locales, negotiated the same way everywhere
build.mjs          assembles the app's client build + these scripts into dist/
dist/              build output — gitignored
```

## What the worker answers, and from where

| Method                                                                 | Answered by                                                                                    |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `eth_accounts`, `eth_chainId`, `net_version`, `wallet_getPermissions`  | the wallet's snapshot + the site's grant and chain pick                                        |
| `eth_requestAccounts`, `wallet_requestPermissions`                     | the side panel (or window): `dapp_permissions`                                                 |
| `personal_sign`, typed data, `eth_sendTransaction`, `wallet_sendCalls` | the side panel (or window): `sign_request`, on the site's chain                                |
| `wallet_switchEthereumChain`, `wallet_addEthereumChain`                | the worker, against the catalog the wallet published (`vela.ext.chains`); unknown chain → 4902 |
| `wallet_watchAsset`                                                    | `false` — tokens are added in the wallet                                                       |
| node and bundler reads (allowlist in protocol.js)                      | forwarded to the catalog's endpoints for the site's chain                                      |
| anything else                                                          | 4200                                                                                           |

The page hears `accountsChanged` / `chainChanged` / `disconnect` when the
site's grant or chain pick changes in storage — on connect, when the wallet
switches accounts (the core re-pins the grant), on revoke, on a switch.
