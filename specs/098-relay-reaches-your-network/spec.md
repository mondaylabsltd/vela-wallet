# 098 — The relay reaches the network you send on

**Status**: design AGREED 2026-10-03 (§0); BUILT 2026-10-03 — see `results.md`. One
requirement was measured and not built: FR-007 (§3.3). Where the build departed from the
design below, the section says so under **As built**.
**Origin**: founder, 2026-10-03, on the web wallet sending without a word on a network
whose relay had no gas.
**Repos**: `vela-wallet` (core, web, desktop, iOS, Android, getvela.app) and `vela-relay`
(both deployments: the Docker service in `src/`, the Cloudflare one in `vela-relay-cf/`).
**Reverses**: spec 081 FR-007 ("the apps MUST NOT send the user's RPC endpoint addresses to
the relay") — by ruling, with disclosure in its place (§0.1).

> 「当依赖的 vela relay 在某个网络上没有 gas coin 时，web 版 vela wallet 是可以继续发送的，
> 而且没有提示……用户一旦用自己的自定义网络，就 100% 会碰到 vela relay 没有钱无法打包，
> 这个时候需要用户自己去充值启动」
>
> 「rpc 虽然带有 key 但是还是得发……在用户设置 rpc 或者设置 rpc 供应商的时候就告知用户，
> 隐私政策里也一样，这样 vela relay 不论是官方的部署的还是自己的部署的，都能拿到客户端配置的
> rpc，然后再把充值引导做好一点」
>
> 「并且文档中要记录着设计」

## 0. Rulings (2026-10-03)

1. **The wallet sends the relay the RPC it uses for the chain**, API key included, on every
   relay request. It is said where the person sets an RPC or an RPC provider key, and in the
   privacy policy. This replaces 081 FR-007's "never send" with "send, and say so".
2. **The relay's broadcaster may use the client's RPC only for a chain whose directory entry
   offers no usable RPC.** Reads (probe, quote, estimate) already prefer the client's RPC; the
   broadcaster does not, and keeps not doing so for any chain the directory serves (§3.3).
3. **Private and loopback RPC hosts stay refused by default**, and a self-hosted relay may
   allow them with an explicit setting. The official deployment never sets it (§3.4).
4. **The wallet never sends silently into a relay that cannot serve the chain** (§2).
5. **The funding sheet is made good** (§4).
6. **The design lives in the documents, not only in this spec** (§6).

## 1. What is wrong today — measured, not recalled

### 1.1 The path that works

The core asks the relay `GET /v1/treasury/{chainId}` at **Continue**. `bootstrapNeeded: true`
opens the treasury sheet and confirm is never entered (`send.rs precheck_settle`). It is
re-asked before the passkey and after a classified failure. All four shells implement the
probe; the sheet has two leads — "Vela's operator runs this relayer, tell them" on a shipped
network, "starting its relayer is up to you" on one the person added.

Proven 2026-10-03: an e2e drives a funded account on a chain whose treasury the relay reports
empty; the sheet opens and confirm never arrives (`e2e/send-relay-empty.e2e.ts`). Live, the
relay reports exactly that shape today for Sepolia, Base Sepolia and Zora.

### 1.2 The paths that do not

The core opens the sheet for `LowFloat` only. **`Uncovered` (404) and `Unknown` (any other
failure) both proceed to confirm, the passkey and submission**, which then fails — after the
person has signed, with no word about why.

And on exactly the networks a person adds, the relay answers neither 200 nor 404:

| Chain | What the relay's directory holds | Treasury answer (live, 2026-10-03) |
|---|---|---|
| 1, 56, 100, 137, 8453, … | public `https` RPCs | 200, funded |
| 7777777, 84532, 11155111 | public `https` RPCs | 200, `bootstrapNeeded: true` |
| 1337 | `http://127.0.0.1:8545` only — refused as loopback | **503** "treasury RPC is unavailable" |
| 123456789 | nothing — the directory answers **200 with an HTML page** | **503** |
| 31337 | "GoChain Testnet", a public RPC — not the local Anvil/Hardhat chain that usually carries this id | 503 (that RPC does not answer) |

Every shell routes 503 as `Unknown` — transient — so the core proceeds. That is the report.

### 1.3 Why funding alone could not fix it

The relay has **never** received the client's RPC. Before 081 the apps sent `X-Rpc-Url`; the
relay reads only `x-vela-rpc-url` (`vela-relay/src/utils/rpc.rs:14`). 081 found the header
inert and removed it. So for a chain the directory cannot reach, the relay cannot read the
treasury, cannot quote, and cannot broadcast — however much gas is sent to it.

## 2. The wallet never sends silently into a relay that cannot serve the chain

The probe has three answers that matter and the core treats them differently:

| Probe | Meaning | Core |
|---|---|---|
| `LowFloat` | relay serves the chain; its float is below the floor | funding sheet (§4), confirm not entered — **unchanged** |
| `Uncovered` | relay says it cannot serve this chain (404 + reason, §3.1) | **new**: the "relay can't reach this network" sheet, confirm not entered |
| `Unknown` | the relay did not answer (5xx, timeout) | proceeds — transient — **unchanged**; a submission that then fails re-probes, as today |

The "can't reach" sheet says which of two things is true, from the relay's reason:

- **the relay has no usable RPC for this chain** — on a network the person added: *its RPC
  must be a public `https` address the relay can reach; a node on this computer or this
  network can only be served by a relay running beside it* (link: Settings → Service
  endpoints → Vela Relay). On a shipped network: *report it* (the operator's problem).
- **the relay does not serve this chain at all** (a self-hosted relay with a chain list):
  *point Settings → Service endpoints → Vela Relay at one that does.*

The core owns the decision and the words (keys under `componentsUi.relayUnreachable.*`);
the shells draw it.

**As built.** The wallet reads only the status: any `404` is `Uncovered`, and the words
split on one thing, whether the network is one Vela ships (`operator_served`, from
`network_admin::is_builtin_chain`) — "report it" on a shipped network; "its RPC must be a
public `https` address, or run a relay beside it" on one the person added. The relay's
`reason` (§3.1) is for whoever reads the response, not for the sheet: a self-hosted relay
with a chain list answers the same `404` as one that cannot reach the RPC, and the advice to
a person who added the network covers both. The sheet's buttons are "Try again" (re-runs the
pre-check) and "Close" (`DismissRelayUnreachable`, back to the form, facts kept).

## 3. The relay

### 3.1 The treasury answer separates "cannot" from "not now"

`GET /v1/treasury/{chainId}`:

| Case | Status | Body |
|---|---|---|
| read succeeded | 200 | as today |
| no usable RPC: no client RPC (or it was refused) **and** the directory has no entry, or an entry whose RPCs are all refused | **404** | `{"error":"…","reason":"no_rpc"}` |
| a usable RPC exists and none answered | 503 | as today (transient) |

"No entry" includes the directory's SPA fallback: a 200 that is not chain JSON is *not
listed*, not a failure.

**As built** (vela-relay PR #15; the decision is the core's `treasury::unreadable`, both
shells render the same bytes):

| Case | Status | `reason` |
|---|---|---|
| balance read | 200 | — |
| the directory does not list the chain — a 404, or its HTML page | 404 | `not_listed` |
| no endpoint the relay may use: no usable header, no Alchemy, none in the entry (1337) | 404 | `no_rpc` |
| the wallet's RPC was refused (private or `http`) **and** nothing else answered (31337: the person's Anvil; the directory's GoChain endpoint is dead) | 404 | `no_rpc` |
| a usable endpoint did not answer, the directory did not answer, or not a balance | 503 | — |

Two departures from the table above, both found while building it:

- **The directory is asked first, and a chain it does not list is `404` even when the
  wallet's RPC answers.** The relay's quote and its executor both need the chain's native
  asset from the directory, so a balance read for an unlisted chain would only have sent the
  person on to a send that fails after signing. (123456789 would have read `200` once the
  header arrived.)
- **A refused wallet RPC plus silent directory endpoints is `404`, not `503`.** Without it
  the commonest local-chain case — Anvil on 31337, whose id the directory gives to a dead
  public testnet — kept answering "not now" and the wallet kept carrying on.

The directory's HTML page is no longer retried three times: it is definitive.

### 3.2 Reads already prefer the client's RPC

`rpc::call` tries `x-vela-rpc-url` first, then Alchemy, then the directory. Unchanged; it
becomes useful because the wallet now sends the header (§5).

### 3.3 The broadcaster may use the client's RPC — only where the directory cannot

Broadcasting signs and sends with the relay's own keys, from a lane that runs after the
request has returned. A caller-supplied RPC could lie about nonces or receipts and make the
relay re-send, burning the float. So:

- a chain the directory **can** reach is broadcast through the directory, exactly as today —
  the client's RPC is never used there;
- a chain the directory **cannot** reach (§3.1's `no_rpc`) is broadcast through the RPC the
  client sent with the submission, which is **persisted with the operation** so the lane
  that runs later has it.

The blast radius is the float of that one chain — which, on a network the directory does not
serve, the person put there themselves (§4).

**As built: not built — measured first.** On the chain list the directory serves
(ethereum-lists, snapshot of 2026-05-06), 198 of 2,602 chains have no public `https`
endpoint, nearly all deprecated testnets or local development ids (1337). That is the whole
population this would serve, because:

- a chain the directory does not list cannot be served either way — the quote and the
  executor need its native asset from the directory (§3.1 as built answers it `not_listed`);
- a private chain is served by a relay run beside it (§3.4), with the operator's own
  `VELA_RELAY_EXECUTOR_RPC_URLS` — never a stranger's URL.

Persisting a caller's URL with each operation and broadcasting through it would have touched
both executors (docker's engine and Iggy envelope, Cloudflare's lane Durable Object) for that
population. The broadcaster stays as it was: it never uses `x-vela-rpc-url`. Recorded in
vela-relay `docs/rpc.md`.

### 3.4 Private RPC hosts: refused, unless the operator opts in

The SSRF guard stays: only `https`, never `localhost`, loopback, RFC1918, link-local or `::1`
— on any URL a caller supplies, so a stranger cannot make the relay fetch its own cloud
metadata or the Redis beside it. A self-hosted relay serving a chain on its own network sets
`VELA_ALLOW_PRIVATE_RPC=true` (also allows `http`). The official deployment never sets it.

**As built.** The setting is `VELA_RELAY_ALLOW_PRIVATE_RPC` — every relay setting carries the
`VELA_RELAY_` prefix. The rule moved into the relay's core (`rpc_host`) and covers every URL
the relay did not choose: the header, and the directory's endpoints for reads and for the
executor alike; the operator's own `VELA_RELAY_EXECUTOR_RPC_URLS` stays exempt. Building it
found two gaps: the docker check never refused `[::1]` (it parsed the bracketed form a URL
parser reports, which is not an IP literal), and the Cloudflare shell accepted **any** `http`
or `https` header URL. Both now apply one rule, which also refuses IPv4-mapped IPv6, IPv6
unique-local and link-local, and `100.64/10`. It judges the host as written, not where a name
resolves.

## 4. The funding sheet, made good

Today: an address, a suggested amount, a disclaimer, "Copy address", "I've funded · Retry".

Made good:
- **a QR of the treasury address** — the money usually comes from another wallet or phone;
- **the balance and the floor** it has to reach, in the chain's own coin — "has 0, needs
  0.0001 ETH" — from the probe the core already holds;
- **it resumes by itself**: while the sheet is open the core re-probes every 10 s; when the
  relay reports the float covered, the sheet closes and the send moves to confirm. "Retry"
  stays for the impatient;
- **the network is named**, so a person funding from a second wallet funds the right chain;
- the two leads (operator / added network) stay as they are.

Desktop still shows the pre-028 "your fee reserve" wording here (080 research) — replaced by
the same sheet.

**As built**, per shell:

| | Web | Desktop | iOS | Android |
|---|---|---|---|---|
| Where | sheet, opened at Continue | notice card on the form and confirm page | notice on the form and confirm page | notice on the form and confirm page |
| Address | text + QR | text + copy | text (selectable) + copy | text (selectable) + copy |
| Has / needs, in the chain's coin | ✓ | ✓ | ✓ | ✓ |
| Network named | ✓ (the sheet's header) | — (the form already names it) | — (same) | — (same) |
| Resumes by itself | ✓ | ✓ | ✓ | ✓ |

The re-probe is the core's (`TREASURY_WATCH_MS` = 10 s); on the form a covered float moves the
send to confirm, **on the confirm page it only closes the stop — it never starts a signature**.
The phones never showed the address at all — on `main` either; found while writing the docs
and fixed here. A QR on desktop and the phones is not built: the phones are usually where the
money already is, and desktop has text to copy.

## 5. The wallet sends the RPC

- **Header**: `x-vela-rpc-url` — the name the relay reads. On every request to the relay:
  treasury probe, quote, gas price, estimate, submit, receipt.
- **Value**: the RPC the wallet itself uses for that chain — the core's `best_rpc_url`
  verdict (kept since 081 for the web simulator): the person's own endpoint or provider when
  they set one, the shipped pool otherwise. Absent when the core has no verdict.
- **All four shells**, at the sites 081 removed it from (081 research §FR-007).

### 5.1 Said where it is set, and in the policy

- **Settings → RPC endpoints** and **the provider-key screens**, in all four shells, one
  sentence under the field: *"The relay that submits your transactions is sent this address
  for this network, including any API key in it, so it can reach the network you use."*
- **The privacy policy** (getvela.app, every locale): the relay receives the RPC address the
  app uses for a network, including an API key that may be part of it; why; that the relay is
  replaceable.
- The in-app privacy summary, where one exists, agrees.

**As built.** The privacy policy page exists in English only (`/privacy` has no localized
route), so FR-003 is the English page; the docs that reach every locale say it too
(networks-and-fees, 15 languages). No app has an in-app privacy summary.

## 6. Where the design is written down

- this spec;
- `vela-relay/docs/` — a page on RPC selection: reads, broadcasting, the `no_rpc` 404, the
  private-host setting, and why broadcasting is restricted;
- `vela-wallet/docs/project-takeover/` — the services page: what the wallet sends the relay
  and why; the treasury sheet's three cases;
- the privacy policy (§5.1).

**As built:** vela-relay `docs/rpc.md` (plus README, `.env.example`, `wrangler.jsonc`,
`docs/cloudflare.md`); `docs/project-takeover/03-core-flows.md` and `07-maintenance-guide.md`
(the cross-repo coupling); getvela.app `networks-and-fees` (both stops) and `self-hosting`
(a chain on your own machine), each in 15 languages; the privacy policy.

## Requirements

- **FR-001** Every relay request from every shell carries `x-vela-rpc-url` = the core's
  `best_rpc_url` for the chain, when the core has one.
- **FR-002** Every place a person sets an RPC endpoint or an RPC provider key says, in their
  language, that the relay receives it (§5.1).
- **FR-003** The privacy policy says so, in every locale.
- **FR-004** `Uncovered` stops the send before confirm with the "can't reach" sheet; its words
  depend on the relay's reason and on whether the network is shipped (§2).
- **FR-005** The relay answers 404 `reason:"no_rpc"` when it has no usable RPC for the chain,
  and 503 only when a usable RPC did not answer — both deployments (§3.1).
- **FR-006** A self-hosted relay can allow private and `http` RPCs with
  `VELA_ALLOW_PRIVATE_RPC=true`; refused otherwise — both deployments (§3.4).
- **FR-007** For a chain with no usable directory RPC, the relay broadcasts through the RPC
  sent with the submission, persisted with the operation; never for a chain the directory
  serves (§3.3).
- **FR-008** The funding sheet shows a QR of the treasury address, the balance against the
  floor, the network's name, and closes into confirm by itself when the relay reports the
  float covered (§4); desktop's old reserve wording is gone.
- **FR-009** The design is written in the documents of §6.
- **FR-010** Each FR has a test that fails without it; the e2e of §1.1 stays.

### Status (2026-10-03)

| FR | Status |
|---|---|
| FR-001 | done — four shells, at the bundler, account, sponsor and treasury sites |
| FR-002 | done — four shells, RPC endpoints and provider keys |
| FR-003 | done — the English policy, the only one there is (§5.1 as built) |
| FR-004 | done — words split on `operator_served`, not on `reason` (§2 as built) |
| FR-005 | done in vela-relay PR #15 — needs a deploy of both shells |
| FR-006 | done in vela-relay PR #15, as `VELA_RELAY_ALLOW_PRIVATE_RPC` |
| FR-007 | **not built** — measured, §3.3 as built |
| FR-008 | done — QR on the web only; address with copy on all four (§4 as built) |
| FR-009 | done — §6 as built |
| FR-010 | done — `results.md` lists each test and the mutation that fails it |

## Not in this spec

Paying the relay's float from the same Vela account (it would need the relay that has no gas);
an RPC that the relay can reach but that serves a different network under the same chain id
(31337) — recorded in §1.2, not solved here.
