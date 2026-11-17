# 109 — Vela Connect: any web app connects to the Vela web wallet by URL

**Status:** approved by the owner (2026-11-17). Built on the web: the protocol, the wallet's window, the reference
examples, the suites. First integrator: sub3 (login and checkout).

## The problem

A person meets Vela through a site, not through an app store. Today a site can reach Vela only through the browser
extension (inject) or a phone over a QR code (WalletPair). Both ask the person to install something first, and most
people stop there. The owner's ask (2026-11-17): a site opens the Vela web wallet by URL — as Coinbase's Smart Wallet
opens `keys.coinbase.com` — the person approves there with their passkey, and the site has its answer. A person with
no wallet creates one in the same window. That is how users start: on the web.

## Owner decisions

| | Decision |
|---|---|
| D1 | The protocol is the standard. **No SDK**: integrators implement the protocol; Vela publishes it and working examples to read and copy (owner, 2026-11-17: "sdk 仅作参考，不能直接用 … 直接给接入示例来参考更好"). |
| D2 | sub3's login and checkout are the first integration. |

## Decisions taken by best practice (and why)

| | Decision | Why |
|---|---|---|
| B1 | **Popup first, redirect as the fallback.** | A popup gives the browser's own fact about who asks (`MessageEvent.origin`) and keeps one window for several requests (connect, then sign, then a payment's status). Redirect is for browsers that cannot open one (some in-app browsers). |
| B2 | **No iframe.** | Passkeys in a cross-origin iframe are restricted (Safari), storage is partitioned, and a framed wallet invites clickjacking. Vela's pages keep `frame-ancestors 'none'`. |
| B3 | **No encryption layer over `postMessage`.** | Origin checks on both sides and an exact `targetOrigin` are the browser's guarantee; a key exchange would not defend against script in the site itself, only add ways to fail. |
| B4 | **One request per id, answered exactly once; `ready` means "send what is unanswered".** | The window navigates (creating a wallet) and comes back: the site resends, the window never answers twice. |
| B5 | **The requester is pinned for the window's life** (`sessionStorage`). | A window opened by one site never serves another, not even after the trip through Welcome. |
| B6 | **Every decision is the core's.** `dapp_rpc::classify`, `dapp_permissions` (connect, who may be signed for, insecure origins), `dapp_rpc::capabilities` / `calls_status`, the one `sign_request` machine and 026's sheet. The window is a transport, like the extension's worker. | [[one-core-everywhere]]: one copy of the rules. |
| B7 | **Grants are kept per origin, listed and revocable in Settings** — the extension's `vela.perm.<origin>` keys, in this origin's `localStorage` behind the same interface (`src/lib/dapp/area.ts`). | A connection nobody can see is one nobody can revoke. |
| B8 | **`wallet_sendCalls` is the payment path; its status is asked of the still-open window.** The window remembers each batch's chain for a day, per site. | EIP-5792 is how a site pays in one approval (approve + pay). The id is the operation's hash; only the wallet knows its chain. |
| B9 | **Silence is never a refusal.** The person closing an undecided request: 4001. A window closed while a signature or payment was on its way: 4900 (it may have happened). | A site retries a 4001 freely; a payment that may have gone out must be checked first. |
| B10 | **Redirect mode says it cannot confirm the requester**, on every request that needs the person, before the card or the sheet; and requires a `state` the site checks. | A link can be made by anyone: the answer goes only to `return`'s origin, but the request may not have come from there. |
| B11 | **Vela Connect is not a node.** Read methods are refused (4200). | Sites have their own RPC; a wallet window must not become an open relay. |
| B12 | **v1 serves Vela accounts, not multisigs.** | A multisig's message signature needs its members' approvals; the proposal flow is the extension's and the in-app browsers', and comes to this window later. |

## Security review (2026-11-17)

An independent adversarial review of the first build found no origin spoof in popup mode; its findings were in
redirect mode and in the clients, and each is closed:

| | Finding | Now |
|---|---|---|
| M1 | A redirect fragment pointed into a site's popup (re-pointed by its window name) was served, ignoring the pin. | A window with an opener takes popup requests only; a redirect request in it is a bad link. The examples and sub3 name the window randomly. |
| M2 | An answer that needed nobody sent the person to `return` at once: the wallet's address was an open redirect. | Vela goes back on its own only to a site the person connected; otherwise the person presses Continue on the "Opened from a link" card first. |
| M3 | A link could revoke a site's grant or move its network, unasked. | Redirect mode answers `wallet_revokePermissions` / `switch` / `addEthereumChain` 4200. |
| M4 | A message signature from a redirect travels in a fragment, which an integrator's own open redirect hands on: a SIWE credential lost. | Redirect mode signs no messages (4200: popup mode only). PROTOCOL §4.3 tells integrators `return` must not redirect onwards. |
| L1 | "Resend on ready" vs "never resend a payment"; a request still being read when the window went was dropped. | Clients resend no payment after a second `ready` (unknown instead); the window answers requests it was still reading on close (4001: nothing shown). |
| L2 | A page that navigates away keeps its window. | Clients close Vela on `pagehide`; documented. |
| L3 | A redirect request abandoned with Back could be replayed later. | Kept 30 minutes at most; dropped when refused. |
| L4 | sub3's deployless check had no gas cap. | 5M cap; any execution failure is an invalid signature. |

## What was built

- `docs/vela-connect/PROTOCOL.md` — the contract, frozen at v1. `docs/vela-connect/examples/` — `vela-connect.js`
  (the protocol written out once: popup and redirect) and `index.html` (connect, sign, SIWE, switch, pay and follow,
  redirect). The e2e suite drives these very files.
- `app-web/vela-wallet`:
  - `/dapp` (307 to `/<locale>/dapp`, the fragment kept) and `src/routes/[locale]/dapp/` — the window.
  - `src/lib/connect/` — `protocol.ts` (the wire, pure), `channel.ts` (popup and redirect transports, the pin, the
    way back from Welcome), `answers.ts` (what needs nobody, from the core), `ops.ts` (batches → chain),
    `ConnectHost.svelte` (the lifecycle), `messages.ts` (`connect.velaConnect.*`, 14 words in 15 languages).
  - `src/lib/dapp/area.ts` — the grants' storage off the extension; `request.ts` answers through any transport.
  - The wallet page sends a window that left to create a wallet back to `/dapp`.
- A site asking for a network of Vela's other space (sub3 on Arc Testnet while Vela shows mainnets) gets a card in
  the window: "{site} wants to use {network}", over the spaces' own switch words. Switched: the site's network is
  set (`null`); declined: `4001` naming the space — not `4902`, which sends a site to `wallet_addEthereumChain`
  and asks again. Found walking sub3 on Arc Testnet with a virtual passkey (2026-11-17).
- Tests: `src/lib/connect/*.test.ts` (wire, channel pinning, batches), `e2e/vela-connect.e2e.ts` (connect, sign, pay
  and follow, read-only answers, revoke, decline by Cancel and by closing, redirect with its warning, a bad link,
  opened directly; Settings lists and revokes).

## sub3 (the first integrator)

- The wallet chooser offers **Vela** ("A passkey wallet in your browser. Nothing to install.") — its own client of
  the protocol (`website/src/lib/dapp/vela.ts`), as any integrator writes one.
- Sign-in: a new Vela account is not deployed on Arc until its first transaction, so its SIWE signature is ERC-6492.
  sub3's server now verifies those with the ERC-6492 reference validator as a deployless `eth_call` on the signed
  chain only — byte for byte what viem's `verifyMessage` sends.

## Later

- Multisig accounts through the window (the proposal flow).
- `wallet_addEthereumChain` for a network Vela lacks (spec 100's sheet).
- The phones: the redirect URL as a universal / app link, so an installed app takes the request.
