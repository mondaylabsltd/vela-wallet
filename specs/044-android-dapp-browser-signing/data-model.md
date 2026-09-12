# Data Model — 044

## Grant (`dapp_permissions`, stored `vela.perm.<origin>`)

`DpermGrant` as the core serialises it (the desktop's key and shape):
origin, addresses, chain_id, granted_at_ms, source. One document per
origin; an unreadable document is "no grant" (never a guessed one).

## Site memory

- `vela.explore` — `ExploreDoc { favorites[], groups[], tabs[] }` (the
  web/desktop shape; `ExploreSite { id, origin, title, url, icon }`,
  `ExploreGroup`, `ExploreTab`).
- `vela.browserHistory` — `BhistEntry[] { origin, title, favicon, url,
  last_visit_ms, visits }`, capped by the core.

## Request (transient)

`Incoming { id, method, params_json, origin, tab_id }` — id and origin are
the shell's (the bridge's page-local id; the WebView's URL through the
core's `origin_of`). Routed by `DappRpc.route(method)`; a `Sign` route is
forwarded by the core (`ForwardToSigning`) to a `SigningController` born
for it with `transport_id = tab_id`.

## Signing record (`vela.transactions`)

- dApp transaction: the feed row with `type: "dapp_tx"`, `userOpHash`,
  `txHash` (patched at receipt), `status`, `from`, `to`, `value`, `chainId`,
  `timestamp`, `dappOrigin`.
- Connection: `type: "connect"`, `id: dapp-<now_ms>-connect`, empty hashes,
  zero value, `from: <address>`, `chainId`, `dappOrigin`.

## Views the screens read

- `DpermView { current_origin, connected_addr, consent: DpermConsentView?,
  popup: DpermPopupView?, … }` → connection sheet / consent card.
- `ExploreView` + `BhistView` + engine state → `ExploreScreenModel`.
- `SignView` + `ClearSigningView` + `GuardView` + `FeeView` →
  `SigningScreenModel`.
