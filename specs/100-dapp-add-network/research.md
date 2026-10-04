# 100 — Research (Phase 0)

The standing rule (099, the owner's): **「跨端同样要使用 crux core 来保持规则逻辑一致性」** — every rule is
decided once in `vela-core`; desktop, iOS, Android and web execute and draw. Facts are cited from
`main` @ `90bcb02b5` (2026-10-04).

The owner's defaults (2026-10-04) are D1–D7 below; each is recorded with what was built and where it
departs.

## R1 — Which machine owns what

**Decision**: three pieces, none new as a machine.

- `dapp_rpc` (pure): `add_chain_ask` reads the page's `AddEthereumChainParameter`;
  `usable_rpc_url` is the RPC-trust rule; `DappAddOutcome` + `add_outcome_error` are how a request
  ends and what the page is answered — one table for the in-app browsers and the extension.
- `dapp_browser` (the request): routes `Route::AddChain` — known chain → switch (unchanged);
  unknown → `ForwardToAddNetwork`; one at a time (-32002); answers from `AddNetworkAnswered`;
  `CancelAddNetwork` when the page goes; the record row.
- `network_admin` (the sheet): `DappAddRequested` → its own `dapp_add` state → the wizard's check →
  `NetView.dapp_add` → `DappAddApproved` (the wizard's save) / `DappAddDeclined` /
  `DappAddRetried` / `DappAddCancelled` → `NetOperation::DappAddSettled { outcome }`.

The shell carries the three messages between the machines, exactly as it already carries
`ForwardToSigning` → the signing sheet → `SigningAnswered`.

**Why**: the check and the save already live in `network_admin` with the custom-network ledger
(the dedup gate reads it; the save writes it). A second copy in `dapp_browser` would be the
duplication the owner ruled out, and the ledger would have two writers.

**Alternatives rejected**:
- *Re-use the wizard's state* (`ChainSelected` + `AddConfirmed`): one app-wide instance on every
  client; a page's request would clobber a person half way through Settings' wizard (and Settings
  resets the wizard when it opens — `WizardReset` — which would cancel the page's).
- *Re-use the scan path* (`AddByChainIdRequested`): it saves **without** asking and flattens the
  verdicts; a page must never add a network on its own.
- *The pipeline in `dapp_browser`*: a copy of the check, and a second writer of `vela.customNetworks`.

## R2 — Same check, not a copy

**Decision**: the wizard's check is refactored into step functions over a `WizardPhase` the caller
owns — `probe_candidates`, `begin_probes`, `probed_step`, `code_step`, `p256_step`,
`contracts_verdict`, `unverified` — each returning `Step::{Wait, Ops, Done(NetCompatibility)}`. The
wizard (`wizard_step`) and the page's add (`dapp_step`) each drive them with their own generation
(`wizard_gen`, `dapp_gen`, drawn from the one machine-wide counter, so answers never cross).
Settings' behaviour is byte-for-byte the old one: every existing `app_network_admin` test passes
unchanged.

**One difference, on the page's path only**: `probed_step(.., must_report)` — the wizard counts any
parsed `eth_chainId` answer as responsive (ported verbatim from `testRpcLatency`); a page's add
counts an RPC only when it answers the requested id (EIP-3085: "The wallet MUST reject the request
if the chainId does not match the value of the eth_chainId method for any of the RPC urls"). Applied
to catalog RPCs too on this path: strictly safer, and a catalog RPC on the wrong chain is "unable to
verify", never a verdict.

## R3 — Catalog first; the page's words only when it must (D3)

**Decision** (as the default): `FetchChainInfo` (the catalog Settings searches) answers first.
- **Known**: name, coin, explorer, RPCs from the catalog (`parse_chain_data`, the wizard's
  candidate assembly minus the typed RPC). The page's `rpcUrls`, `chainName` and symbol are not used
  — a core test asserts the probe list is the catalog's.
- **Unknown**: a network made of the page's words (`site_chain_info`, the parser's defaults where
  the page said nothing) and the page's RPC URLs, each tried only if `usable_rpc_url` passes and
  counted only if it answers the requested chain id. The sheet says "Not in Vela's network list — the
  name and coin are the site's."

**Known limit**: every shell's `FetchChainInfo` answers `None` for "not found" and for "could not
reach the catalog" alike. A catalog outage therefore falls to the page's words — labelled as the
site's, RPC still proven by `eth_chainId`, contracts still checked. Telling the two apart needs a
wire change in four executors; not done here.

## R4 — The answers (D4, D5)

EIP-3085 specifies no error codes: "The method MUST return `null` if the request was successful,
and an error otherwise." The codes below are the ones dApps and their libraries test for.

| Ending | Code | Why this code |
|---|---|---|
| added (incl. "the wallet already has it") | `null` | EIP-3085 |
| declined; closed while checking; closed after "unable to verify" | 4001 | EIP-1193 "User Rejected Request" — the person chose to stop |
| not compatible (contracts / P256 missing) | **4902** | MetaMask's "Unrecognized chain ID" ([docs](https://docs.metamask.io/metamask-connect/evm/reference/json-rpc-api/wallet_switchEthereumChain.md)); after the refusal the chain is still not one Vela has, which is the fact the page needs |
| the page's RPC unusable / on another chain; no usable RPC | -32602 | EIP-1474 "Invalid params"; MetaMask answers its rpcUrl/chainId mismatch with invalidParams too |
| malformed parameters (incl. decimals ≠ 18) | -32602 | as above, before any sheet |
| another add sheet open | -32002 | EIP-1474 "Resource unavailable" — what wallets answer a request of a kind already pending |
| page navigated / tab closed | 4900 | unchanged (070 R2: never 4001 — a dApp retries a 4001) |

**Not compatible**: alternatives considered — -32603 (internal error: says nothing actionable),
4200 (unsupported *method*: wrong), 4901 (chain disconnected: a provider state, not a refusal),
4001 (the person did not reject; the wallet did — and a 4001 invites a retry loop). The person sees
the verdict on the sheet and closes it; the page hears 4902 then (closing the sheet IS the answer,
whatever button — `DappAddDeclined` settles by the phase it was in).

**Library check**: wagmi's `switchChain` calls `wallet_addEthereumChain` after a 4902 from switch,
wraps ANY add error as `UserRejectedRequestError`, and after a `null` reads `eth_chainId` — if the
chain did not change it throws "User rejected switch after adding network". So the switch after an
add (D5) is what makes the flow work, even though EIP-3085 says the chain "MUST NOT be assumed to be
automatically selected". `inpage.js` already applies the asked chain on a `null` answer.

## R5 — What a page may give (D3, debug-mode rule)

**Decision**: `usable_rpc_url(url, debug_mode)` = no whitespace, no `user:pass@`, ≤ 512 chars, and
`offers_wallet(origin_of(url), debug_mode)` — the ONE rule spec 091 uses for which http pages get the
wallet: https always; http on loopback; http on this device's own network with debug mode on.
`wss://`, `file:`, public `http://` are refused (EIP-3085: "MUST reject any URLs that use the
`file:` or `http:` schemes" — loopback/LAN http is the deliberate debug exception). At most four
usable URLs are kept; the count refused is carried (`refused_rpc_urls`).

Also read by `add_chain_ask`: `chainName` and `nativeCurrency.symbol`, cleaned (trimmed, control
and bidi-override characters removed, ≤ 64 characters) because they are drawn; the first https
`blockExplorerUrls` entry. `nativeCurrency.decimals` other than 18 is -32602 (every native coin is
counted in 18 decimals; MetaMask refuses the same). `rpcUrls` absent/empty is NOT refused up front —
the catalog may know the chain (more dApps work); it ends as "no usable RPC" only when it does not.

## R6 — One sheet at a time (D2)

**Decision**: `dapp_browser` holds at most one `AddJob`; a second add request from any tab or origin
while it is open answers -32002 (record `wallet / consent_busy`). The consent and signing sheets keep
their own rules (consent: same origin merges, another origin 4001; signing: FIFO). Shells draw the
add sheet in the consent sheet's place; when both a consent and an add are open, the consent shows
first. `network_admin` defends itself the same way (`Busy` for a second `DappAddRequested`).

`consent_busy`'s line generalises from "Another site's request is open" to "Another request is
open" in all 15 locales: it now ends both.

## R7 — Approve, and what reaches the rest of the app (D5)

**Decision**: `DappAddApproved` builds the record with the wizard's `build_custom_network` and saves
it with `save_custom_network` — `WriteCustomNetworks`, `last_added_chain_id` — the exact
`AddConfirmed` path (no pool invalidation, as Settings' add has none). Then `DappAddSettled{added}`;
`dapp_browser` pushes the chain into its list at once (the shell's `networks_changed` follows and
agrees), `WriteSiteChain`, `chainChanged` to every tab of the origin, `null`.

How each shell's other consumers learn of the network is unchanged — the same as after a Settings
add: Android/iOS/web read `NetView.networks` (or the store) live; the desktop reads
`vela.customNetworks` on demand, and `follow_networks` re-reads it before every switch/add.

## R8 — Words and the residency budget (D7)

The sheet reuses Settings' words: title `settingsModals.addNetwork.modalTitle`, rows
`addToken.label{Name,ChainId,NativeToken,RpcUrl,Explorer}`, `checkingCompatibility`,
`compatible`, `incompatible`, `incompatibleHint`, `openChainSetupTool`, `unableToVerify`, `retry`,
`addNetworkBtn`, `singleKeyOnly`, `assets.rpcFixWrongChain` for a page RPC on another chain,
`connect.browser.cancel`, `common.done`. The record's `not_compatible` line is
`addToken.errorNotCompatible` ("Not compatible with Vela Wallet").

New (15 locales): `connect.browser.addLead` ("{{host}} asks to add a network"),
`connect.browser.addFromSite`, `componentsUi.browserStatus.reason.badRpc`.

Residency (`ja` + `en`, runtime route) was 144,350 of 144,400 — 50 bytes of room. The budget is the
owner's and is not moved. Removed instead: `onboarding.login.alertNotFound{Title,Body}` — git grep
finds neither the full path nor the leaf name outside the corpus and the generated tables (881
bytes of `en`+`ja` values; the three new lines and the shorter `consentBusy` add 334). Measured
after: **143,811** of 144,400.

## R9 — The extension (web)

The worker cannot run the core per request (070), and the side panel may only open inside the
page's gesture — decided synchronously.

**Decision**:
- The worker's in-memory mirror (089's `grantMirror`) also mirrors the published chain catalog
  (`vela.ext.chains`), so `route` decides at once: catalog says the chain is known, or cannot say,
  or the params name no chain → the switch path as before; known-unknown → -32002 if an add record
  is open, else the surface path (panel / window) like a connect.
- `content.js` holds `addChain` like `connect` (accepted now, answered by message; a dropped channel
  retried once — the worker dedupes by document).
- The surface (`DappRequestHost`) branches on `addChain` before the connect logic: reads the ask
  with the core (`dappAddChainAsk` over wasm), runs `network_admin`'s `dapp_add_*` like the native
  shells, draws the same sheet; on `added` it publishes the catalog (`publishExtChains`) and
  answers `null`; otherwise it answers `dappAddOutcomeError`'s code.
- The worker, answering an `addChain` record with `null`, writes `vela.chain.<origin>` through the
  existing `switchChain` (which re-reads the catalog just published) before delivering — the
  storage listener then sends `chainChanged`, as for a switch. So the worker stays the one writer of
  a site's chain.

**Rejected**: answering add in the worker by opening Settings — the dApp would wait on a page the
person may never open; and a worker-side copy of the check — the duplication R1 rules out.
