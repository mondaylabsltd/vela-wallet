# Vela Connect — Protocol v1

Vela Connect lets any web app use the **Vela web wallet** (`https://wallet.getvela.app`) the way it uses a
browser wallet: ask for the account, ask for a signature, ask for a transaction. Nothing is installed: the app
opens a Vela window, the person reviews and approves there with their passkey, and the answer comes back to
the app. A person who has no wallet yet creates one in the same window, in a few seconds.

There is no SDK. This document is the whole contract; implement it directly. The files in
[`examples/`](examples/) are working references to read and copy from, not a package to install.

The words **must**, **must not** and **should** are used as in RFC 2119.

---

## 1. Overview

```
your page                                   wallet.getvela.app/dapp
─────────                                   ───────────────────────
click ─► window.open(…/dapp) ─────────────► loads, then:
         ◄──────────────────────────────── {vela:1, type:"ready"}
         {vela:1, type:"request", id, method, params} ─►
                                            Vela asks the core, shows a consent card or the signing sheet,
                                            the person approves (passkey) or declines
         ◄──────────────────────────────── {vela:1, type:"response", id, result | error}
         … more requests, same window …
         popup.close()
```

Two ways to open Vela:

| Mode | When | Who the requester is |
|---|---|---|
| **Popup** (§3) | The default. Any browser that lets a click open a window | The browser's own fact: `MessageEvent.origin` |
| **Redirect** (§4) | A popup cannot be opened (`window.open` returned `null`; some in-app browsers) | The origin of the `return` URL. Vela says it cannot confirm who sent the link |

Requests and answers are **EIP-1193** requests and answers: `method` and `params` exactly as you would pass to
`provider.request(...)`, and the result or error a provider would give you.

## 2. The wallet address

| | URL |
|---|---|
| Production | `https://wallet.getvela.app/dapp` |
| Your local Vela (development) | `http://localhost:5173/dapp` (Vela's `bun run dev`) |

`/dapp` forwards to the page in the person's language (`/en/dapp`, `/zh/dapp`, …). The fragment survives the
forward. Messages come from, and must be sent to, the **origin** (`https://wallet.getvela.app`), never a path.

## 3. Popup mode

### 3.1 Open the window

Open it **synchronously inside the click handler** (a `click`/`pointerup`/`keydown` handler, with no `await`
before `window.open`). Browsers block a window opened later, and Safari blocks it after any `await`.

```js
const VELA = 'https://wallet.getvela.app';
const popup = window.open(`${VELA}/dapp`, `vela-${crypto.randomUUID()}`, 'popup,width=420,height=720');
if (!popup) { /* blocked: use redirect mode (§4) */ }
```

Give the window a **name of its own** (random, as above), never a fixed one: any page can re-point a window it can
name.

Your page **must not** send `Cross-Origin-Opener-Policy: same-origin` (it cuts the window off from yours).
`same-origin-allow-popups` and no header are both fine. Vela's page sends none.

### 3.2 Wait for `ready`

When Vela can take requests it posts to its opener:

```json
{ "vela": 1, "type": "ready" }
```

Send nothing before it. `ready` comes again if the window reloads or comes back to `/dapp` after the person
created a wallet (§3.6). Each `ready` means: "anything you sent before that I have not answered, send again".
Vela never answers the same `id` twice.

**Except a payment.** An `eth_sendTransaction` or `wallet_sendCalls` still unanswered when `ready` comes **again**
may have gone out before the reload: do not resend it. Treat it as unknown (§6) — check the chain, then ask again.

Accept a message only when `event.source === popup` and `event.origin === VELA`.

If no `ready` arrives in 30 seconds, close the window and show an error (Vela did not load).

### 3.3 Send requests

```js
popup.postMessage({ vela: 1, type: 'request', id: 'r1', method: 'eth_requestAccounts', params: [] }, VELA);
```

| Field | Type | |
|---|---|---|
| `vela` | `1` | Protocol version |
| `type` | `"request"` | |
| `id` | string, 1–64 characters | Yours; unique in this window |
| `method` | string | An EIP-1193 method (§5) |
| `params` | array, optional | As for `provider.request`. At most 512 KB of JSON |

Always pass `VELA` as the target origin, never `"*"`.

**Who is asking.** The first request fixes the requester: the `MessageEvent.origin` the browser reports for
it. Vela shows that origin to the person, grants permissions to it, and answers only to it. A window opened by
one site never serves another: a message from any other origin is ignored, and so is any message not sent by
the window's opener.

Vela takes requests **one at a time, in the order they arrive**; you may send several at once.

### 3.4 Receive answers

```json
{ "vela": 1, "type": "response", "id": "r1", "result": ["0x…"] }
{ "vela": 1, "type": "response", "id": "r2", "error": { "code": 4001, "message": "User rejected the request" } }
```

Exactly one answer per `id`, sent to your origin only. `error` has `code`, `message` and sometimes `data`, as
EIP-1193 defines (§6).

### 3.5 Closing

- **You close it** (`popup.close()`) when your flow is done: when the last answer you need is in. Vela keeps
  the window open after an answer, so that the next request (sign in right after connecting, the status of a
  payment) needs no new click.
- **The person closes it.** Watch `popup.closed` (poll every 500 ms). Vela answers what it owed when the window
  goes, if the browser lets it run (§6), but a window can close without a word: treat an unanswered request as
  **unknown** — see §6 for what that means per method.
- **Your page goes away.** Close Vela's window on `pagehide`, so the person is not asked anything for a page that
  is gone. (A page that navigates to another origin keeps its window open, and Vela cannot tell: an answer sent then
  is lost. Vela's activity still has what happened.) If the window outlives your page, Vela sees its opener gone and
  stops.

### 3.6 A person with no wallet

The window shows "Create a wallet" and "Sign in". Creating one takes a passkey and a few seconds, in the same
window. Then the window returns to `/dapp`, sends `ready` again (§3.2), and you resend what is unanswered. You do
not need to do anything special: your request simply waits a little longer.

## 4. Redirect mode

Use it only when a popup cannot open. It carries **one request** per round trip.

### 4.1 Go to Vela

```
https://wallet.getvela.app/dapp#request=<base64url(JSON)>&return=<URL>&state=<state>
```

| Parameter | |
|---|---|
| `request` | `{"method": "...", "params": [...]}` as UTF-8 JSON, base64url without padding |
| `return` | Where to come back to. `https:` only (`http:` only for `localhost` and `127.0.0.1`). No user name or password in it. Its **origin** is the requester |
| `state` | Your random value, 16–256 characters of `[A-Za-z0-9_-]`. Keep it (e.g. in `sessionStorage`) before you leave |

Everything is in the **fragment**, so none of it is sent to any server.

### 4.2 Come back

Vela replaces the fragment of `return` (keeping its path and query) and navigates there:

```
<return>#vela=<base64url(JSON)>
```

The JSON is `{"vela":1, "state":"…", "result": …}` or `{"vela":1, "state":"…", "error": {"code", "message"}}`.

You **must** compare `state` with the one you kept and **ignore** the answer if they differ. Then remove the
fragment (`history.replaceState`).

### 4.3 What redirect mode cannot prove, and what it therefore does not do

A link can be made by anyone. Vela cannot confirm that your site made it, only that the answer will go to
`return`'s origin. So:

- **Before the person decides anything** (a connect card, a transaction), Vela says it cannot confirm who sent the
  link, and the person continues or cancels.
- **Vela goes back on its own only to a site the person has connected.** Any other answer — even one that needed
  nobody, such as `eth_chainId` — waits for the person to press Continue: Vela's address is never a link that sends
  anyone somewhere without a click.
- **No message signatures** (`personal_sign`, `eth_signTypedData_v4`): a signature is a credential (Sign-In with
  Ethereum above all), and a fragment is carried along by any redirect at `return`. Sign messages in popup mode.
  Answered `4200`.
- **Nothing that changes a site's standing**: `wallet_revokePermissions`, `wallet_switchEthereumChain`,
  `wallet_addEthereumChain` are answered `4200`. A redirect request runs on the chain the site is on in Vela (the one
  it connected on); switch in popup mode.
- **Not inside a popup.** A window that has an opener takes popup requests only; a redirect request pointed into it
  is refused.
- `state` is required: an answer you did not ask for must be thrown away.

Your part: `return` should be an exact page of yours that handles the answer and **must not redirect onwards**
(an open redirect there would carry the fragment to someone else).

## 5. Methods

| Method | Needs the person | Answer |
|---|---|---|
| `eth_requestAccounts` | Once per site (a connect card); after that, no | `[address]` |
| `wallet_requestPermissions` | Same as above | EIP-2255 permissions |
| `eth_accounts` | No | `[address]`, or `[]` if not connected |
| `wallet_getPermissions` | No | EIP-2255 permissions |
| `wallet_revokePermissions` | No | `null`; the site is disconnected. Popup mode only |
| `eth_chainId` | No | The chain this site is on, hex |
| `wallet_switchEthereumChain` | No, unless the chain is in Vela's other space (§5.2) | `null`, 4001 or 4902 (§5.2). Popup mode only |
| `wallet_addEthereumChain` | No | Same as switch, for a chain Vela has; 4902 otherwise. Popup mode only |
| `wallet_getCapabilities` | No | EIP-5792 capabilities (atomic batches on every chain Vela has) |
| `personal_sign` | Yes, the signing sheet | Signature (§5.3). Popup mode only |
| `eth_signTypedData_v4` | Yes | Signature (§5.3). Popup mode only |
| `eth_sendTransaction` | Yes | Transaction hash, once included |
| `wallet_sendCalls` | Yes | EIP-5792 batch id (`{id}` for version `"2.0.0"`) |
| `wallet_getCallsStatus` | No | EIP-5792 status of a batch this window's site sent |

Anything else is refused with `4200`. Vela Connect is not a node: read the chain with your own RPC.
`eth_sign` is always refused.

Signatures, transactions and batches (`personal_sign`, `eth_signTypedData_v4`, `eth_sendTransaction`,
`wallet_sendCalls`) need the site to be connected first (`4100` otherwise); so does `wallet_getCapabilities` for an
address. `eth_accounts` answers `[]` to a site that is not connected.

### 5.1 The account

The account is the person's Vela account: a Safe smart account controlled by their passkey, with the same
address on every chain. Before its first transaction on a chain it is **not deployed** there (no code). Its
first transaction deploys it, in the same operation.

A site is connected to one account. If the person signs in to another account in Vela, the site is asked to
connect again.

### 5.2 Networks

A site is on one chain. A new site starts on the chain where the person's account is used, and moves only by
its own `wallet_switchEthereumChain`. Switch first, then send what you want on that chain; Vela signs on the
chain the site is on and refuses a request that names another.

Vela keeps test networks and real networks apart. A switch to a test network while the person uses real
networks (or the other way round) asks the person: Vela's window offers to change to the other space. Switched:
`null`, as any switch. Declined: `4001`, with a message saying which space has the network — do not follow it with
`wallet_addEthereumChain`, which would only ask again. A chain Vela does not have at all is `4902`.

Switching before `eth_requestAccounts` is allowed, and kinder: the connect card then names the network the site is
on.

### 5.3 Signatures

Signatures follow **ERC-1271** (the account checks them, `isValidSignature`). For an account not yet deployed
on the chain, the signature is wrapped as **ERC-6492**. To check a signature, use an ERC-6492 aware verifier
(viem's `verifyMessage` / `verifyTypedData`, or the ERC-6492 universal validator); a plain `ecrecover` is
never right for a Vela account.

For Sign-In with Ethereum (EIP-4361), the message's `domain` must be your site's host: Vela refuses a message
whose domain is not the requester's.

### 5.4 Transactions and batches

`wallet_sendCalls` is the recommended way to send: several calls (an `approve` and a `pay`, say) happen in one
operation, atomically, with one approval from the person. Its answer is the batch id. Follow it with
`wallet_getCallsStatus` on the **same, still open** window until `status` is `200` (confirmed) or `≥ 400`
(failed), then close the window. While it waits, Vela shows the person the payment landing.

A batch id is not a transaction hash. After you close the window, Vela can no longer be asked about it: read
the outcome from the chain itself (your contract's state, or the receipt `wallet_getCallsStatus` gave you).

`eth_sendTransaction` answers the transaction hash once the transaction is included.

Network fees are paid by the account, in the chain's fee currency. A person without enough for the fee is told so
before anything is signed.

## 6. Errors

| Code | Meaning | What to do |
|---|---|---|
| `4001` | The person declined, or closed the window before approving | Nothing happened. Asking again is safe |
| `4100` | Not connected (or connected to another account) | Send `eth_requestAccounts` |
| `4200` | Method not supported | — |
| `4900` | The window closed or the page went away while the request was being carried out | **Unknown.** It may have happened: check the chain before asking again |
| `4902` | The chain is not one Vela offers to this site now | Show the message |
| `5730` | `wallet_getCallsStatus`: unknown batch id | — |
| `-32602` | Invalid params | — |
| `-32603` | Internal; for a transaction, "submitted but not confirmed yet" names the operation | **Unknown**, as 4900 |
| `-32600` | Not a request this protocol understands (`vela` is not `1`, a bad `id`) | — |

A request with **no answer** when the window closed is **unknown** for `eth_sendTransaction` and
`wallet_sendCalls` (it may have been sent) and a refusal for everything else.

**Never resend a payment just because the answer is unknown.** Check first, or make the payment idempotent (an
order id the contract accepts once).

## 7. Security model

- The requester is the browser's fact (`MessageEvent.origin`), never a name the page gives itself.
- Answers go only to that origin (`targetOrigin`), and only to the opener.
- Nothing is signed blind: every signature and transaction is decoded, simulated and shown on Vela's own page,
  with the requester's origin.
- Permissions are per origin and per account, listed in Vela's Settings, and revocable there or by the site
  (`wallet_revokePermissions`).
- The person's key never leaves their passkey; Vela's page cannot be framed (`frame-ancestors 'none'`).

## 8. Versioning

`vela: 1` is in every message. A message with another value is answered with `-32600` (if it has an `id`) and
otherwise ignored. Fields are only ever added to v1; a change that would break an integration is v2, at a new
`vela` value, with v1 kept.
