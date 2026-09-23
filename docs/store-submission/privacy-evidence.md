# Privacy evidence — Vela Wallet iOS & Android

**Purpose.** This is the evidence base behind the Apple App Privacy ("nutrition label") and
Google Play Data Safety answers in `privacy-and-review.md`. Every claim here traces to a file
and line that was read, or to an endpoint whose request shape was read out of the client code.
It is written so that it can be handed to an App Review or Play Policy reviewer and defended
line by line.

**Scope.** The two store binaries: `app-ios/VelaWallet` and `app-android/vela-wallet`, plus the
shared Rust core in `rust/crates/vela-core`, plus every server those binaries contact by
default. The web wallet, the browser extension, the desktop app and the `getvela.app` marketing
site are **out of scope for the store forms** but are noted where they share infrastructure,
because a reviewer reading the published privacy policy will see them.

**Audited on** 2026-09-23, against `vela-wallet` `main` @ `746e2259`, `p256-index` @ `e6a4af3`,
`vela-relay` @ `a2f255f`, `vela-currency` @ `bd779f8`, `ethereum-data` (live worker checkout).

**Operator / data controller** for every first-party service below: **MONDAY LABS LTD** (UK).
Evidence: `p256-index/LICENSE:3`, `vela-relay/LICENSE:3` — `Copyright (c) 2026 MONDAY LABS LTD`.

---

## 0. The one-paragraph answer

The apps **do** collect, in Apple's sense. Apple defines "collect" as transmitting data off the
device in a way that lets you or your partners access it for longer than is needed to service
the request in real time. Vela's own relay **logs the user's wallet address on every accepted
transaction** (`vela-relay-cf/src/admission.rs:203-205`) and **stores the complete signed
UserOperation — sender address, calldata, signature — for up to 14 days** on the deferred path
(`vela-relay-cf/src/lane_do.rs:1400-1412`, `:1784-1787`). Vela's own passkey index **stores the
wallet address, the user-typed wallet/key names, every passkey public key, credential ID and
AAGUID for 7 days after success and 30 days after failure**
(`p256-index-cf/src/submitter.rs:399-411`, `:57-58`) and then **publishes all of it permanently
to a public blockchain** (`p256-registrar/src/protocol.rs:27,88`). None of this is optional;
all of it is the product working normally. The convenient answer — "the wallet is
self-custodial, so we collect nothing" — is false on the evidence, and the app's own published
privacy policy already says so (`app-web/getvela.app/src/routes/privacy/+page.svelte:102-117`).

What is *not* collected is equally clear and should be stated positively: no private key, no
seed phrase, no name, email, phone or government ID, no advertising identifier, no analytics
SDK, no crash reporter, no location, no contacts, no cross-app tracking of any kind.

---

## 1. Network destination inventory

### 1.1 The four configurable service endpoints

Defaults: `rust/crates/vela-core/src/app/network_admin.rs:152-155`. All four are editable in
Settings → the endpoint is persisted per install and read back at
`network_admin.rs:2784-2795`; the apps ship the constants only as a fallback.

| # | Host | Operator | Request carries | Persisted? |
|---|------|----------|-----------------|------------|
| 1 | `ethereum-data.getvela.app` | Monday Labs | chain ids; **token contract addresses** (logo paths); **contract addresses being signed** (ERC-7730 descriptor paths) | No app-side store; served as static assets. Cloudflare edge sees path + IP. |
| 2 | `p256-index-v2.getvela.app` | Monday Labs | **wallet address, wallet/key names, P-256 public keys, credential IDs, AAGUIDs, rpId, WebAuthn proofs** | **Yes — 7 / 30 days, then published permanently on-chain** |
| 3 | `vela-relay-cf.getvela.app` | Monday Labs | **wallet address, full UserOperation (calldata + signature), fee-quote requests naming the address** | **Yes — 1 h to 14 days; address logged on every submit** |
| 4 | `vela-currency.getvela.app` | Monday Labs | nothing user-specific (`?base=USD` only) | No per-requester state. Workers Logs **enabled**. |

### 1.2 Every other host the two apps contact

| Host | Operator | When | What it carries | Evidence |
|------|----------|------|-----------------|----------|
| **24 default chain RPC endpoints** — see §1.3 | **Third parties** | Every balance read, every simulation, every `eth_call` | **Wallet address + IP** | `rust/crates/vela-core/src/app/network_admin.rs:206` (`BUILTIN_CHAINS: [NetBuiltinChain; 24]`), mirrored verbatim at `app-ios/VelaWallet/VelaWallet/Core/ChainCatalog.swift:53-136` |
| **Curated public fallbacks** for 9 chains: `*-rpc.publicnode.com`, `1rpc.io/*`, `bsc.drpc.org`, `xlayer.drpc.org` | **Third parties** | When the default endpoint fails | Same | `app-ios/VelaWallet/VelaWallet/Core/ChainCatalog.swift:170-180`; `app-desktop/vela-wallet/src/executor/pool.rs:60-108` |
| `aaguid-explorer.awesometools.dev` | Founder-operated, **not on the `getvela.app` domain** | Only when the compiled catalog cannot name the authenticator (i.e. hardware keys) | An AAGUID — the **model** of the user's passkey vault or security key. Not user-unique. | Default constant `rust/crates/vela-core/src/passkey.rs:247`; uniffi binding uses the constant with **no settings override** on iOS/Android — `rust/crates/vela-core-uniffi/src/lib.rs:596`; clients `app-ios/.../Onboarding/Core/PasskeyDirectory.swift:75-95`, `app-android/.../core/passkey/PasskeyDirectory.kt:57-70` |
| `api.openchain.xyz`, then `www.4byte.directory` | Third parties | A transaction the wallet cannot otherwise decode | A 4-byte function selector + IP. **Not** the wallet address. | `app-ios/.../Signing/Core/ClearExecutor.swift:182,193`; `app-android/.../feature/signing/core/ClearExecutor.kt:69,89` — the two store apps ask these two, sequentially. Desktop additionally asks `api.4byte.sourcify.dev` (`app-desktop/vela-wallet/src/executor/clear_signing.rs:156`); that host is **not** in the iOS/Android path. |
| dApp sites opened in the in-app browser, **and the connected site's own origin at signing time** | Third parties | User navigates there / a signing sheet opens | Normal web browsing; the site learns the wallet address once the user connects. The signing sheet fetches the site's icon **from the site itself** — `{origin}/apple-touch-icon.png` then `{origin}/favicon.ico` — so **the dApp's server sees the user's IP at the moment they are asked to sign**. | `app-ios/.../Features/Signing/SigningLive.swift:70` + `Components/Wallet/RemoteLogoView.swift:31-38`; `app-android/.../feature/signing/SigningLive.kt:70-72` + `core/marks/RemoteLogo.kt:56-69`. No favicon proxy and no Google favicon service is used anywhere. Explore tiles deliberately draw a letter avatar instead of fetching (`app-ios/.../Features/Explore/ExploreLive.swift:17`). Browser history is **on device only** (`rust/crates/vela-core/src/app/browser_history.rs:11-16`). |
| Blockchain explorers (`etherscan.io`, `basescan.org`, …) | Third parties | User taps "view on explorer" | Nothing — **the URL is handed to the OS browser; no shell has an HTTP client pointed at an explorer host** | `app-android/.../feature/flows/FlowHost.kt:300-310` (`Intent.ACTION_VIEW`) |
| `github.com` (system browser, not the app) | Third party | User taps Settings → Feedback → Send | A prefilled `issues/new` URL containing app version+commit, platform, language, failed chain names, recent failure strings. **Handed to the OS browser; the app sends nothing.** | `app-android/.../navigation/VelaNavHost.kt:1693-1699`; contents built at `app-android/.../feature/settings/SettingsLive.kt:569-578` |
| `api.telegram.org` | Third party, **server→server only** | Operator alerts | Never contacted by the app. Relay/index alerts carry operation hashes and the relay's own address, never a user address. | `vela-relay/vela-relay-cf/src/arms/telegram.rs:41`; `p256-index/p256-index-cf/src/telegram.rs:35-37` |

**Not contacted by the apps:** no ad network, no analytics host, no crash-report host, no push
service, no remote-config host, no update-check host. See §4.

### 1.3 Who the default RPC endpoints actually are

This matters for the "third-party partner" judgement in §6.5 #3, so the composition is worth
stating exactly. `BUILTIN_CHAINS` (`rust/crates/vela-core/src/app/network_admin.rs:206-435`)
ships 24 chains with one default endpoint each:

- **20 are the chain foundation's or operator's own public endpoint** —
  `bsc-dataseed.binance.org`, `arb1.arbitrum.io`, `mainnet.optimism.io`, `mainnet.base.org`,
  `api.avax.network`, `rpc.gnosischain.com`, `mainnet.unichain.org`, `rpc.mainnet.tempo.xyz`,
  `rpc.monad.xyz`, `rpc.mainnet.arc.io`, `rpc.xlayer.tech`, `rpc.stable.xyz`, `rpc.soneium.org`,
  `rpc.mainnet.chain.robinhood.com`, `rpc.mantle.xyz`, `public-en.node.kaia.io`,
  `forno.celo.org`, `rpc-gel.inkonchain.com`, `rpc.plume.org`, `rpc.xrplevm.org`.
- **4 are commercial aggregators** — `ethereum-rpc.publicnode.com` (:212),
  `polygon-bor-rpc.publicnode.com` (:230), `worldchain.drpc.org` (:311),
  `megaeth.drpc.org` (:365).
- **No Infura, no Alchemy, no Ankr, no llamarpc, no BlockPI in the defaults.** Alchemy, dRPC and
  Ankr are reachable **only** after the user enters their own API key
  (`network_admin.rs:544-546`, gated at `:665-683`). `eth.llamarpc.com` appears only in design
  fixtures, never at runtime.
- The curated second tier (9 chains) adds `*-rpc.publicnode.com`, `1rpc.io/*` (Automata) and two
  `*.drpc.org` hosts.

**All of them are replaceable.** Resolution order is user override → user's keyed provider →
built-in default → curated public → chain index
(`app-desktop/vela-wallet/src/executor/pool.rs:57-59, 737-744`), and the override is stored
device-locally under the key `vela.serviceEndpoints` on every platform — iOS
`app-ios/VelaWallet/VelaWallet/Core/VelaStore.swift:68`, Android
`app-android/.../core/data/VelaStore.kt:88`. The four service endpoints are replaceable through
the same record, so a self-hoster can point the app entirely at their own infrastructure.

---

## 2. What each Vela-operated service persists

This is the section that decides the form. A stored address is collection. A log line with an
address is collection.

### 2.1 Passkey index — `p256-index-v2.getvela.app` — **STORES, then PUBLISHES**

**What the app sends.** Wallet creation POSTs `/api/register` with `rpId`, `groupPublicKey`,
`groupProof`, `metadata`, and for each founding passkey `publicKey`, `credentialId`,
`attestation`, `authenticatorAttachment`, `transports`, `proof`.
Client evidence: `app-android/.../feature/onboarding/core/RegistryClient.kt:113-156`,
`app-ios/.../Features/Onboarding/Core/RegistryClient.swift:229`.

**What is inside `metadata`.** `rust/crates/vela-core/src/registry_metadata.rs:39-53`:

```rust
pub struct RegistryMetadata {
    pub version: u8,
    pub address: String,          // "The counterfactual Safe wallet address, EIP-55 checksummed."
    pub wallet_version: String,
    pub key_names: Vec<String>,   // user-typed labels, one per founding key
    pub created_at_iso: String,
}
```

`key_names` is free text the user types (`rust/crates/vela-core/src/app/create_wallet.rs:442`,
`:383`) — the crate's own fixtures use `"主钱包"`, `"备用手机"`, `"ledger"`
(`registry_metadata.rs:90,107-111`). A user can type a real name or a device name.

**Server-side persistence.** One Durable Object with SQLite; no D1, no KV, no R2
(`p256-index-cf/wrangler.toml:39-45`). Tables at
`p256-index-cf/src/submitter.rs:232-292`. The two that matter:

- `tasks.payload` — **the entire register request serialized as JSON**, i.e. the wallet address,
  the key names, every public key, every credential ID, every AAGUID and every WebAuthn proof
  including the raw `clientDataJSON`. Written at `submitter.rs:399-411`; shape at
  `p256-registrar/src/task.rs:44-62, 73-106`.
- `key_placeholders.public_key` — **a raw P-256 public key as a primary key**. Despite the
  parameter being named `key_hash` throughout, it is not hashed; the code says so:
  `p256-registrar/src/lookup.rs:535-537` — *"To keep the Core pure we use the normalized key
  itself as the logical identity here."*

**Retention.** 7 days after a successful registration
(`submitter.rs:57` `TASK_DONE_TTL_MS = 7 * 24 * 60 * 60 * 1000`), 30 days after a failed one
(`submitter.rs:58`). Swept by `purge_expired()` (`submitter.rs:296-308`) on every fetch and by a
one-minute cron (`wrangler.toml:36-37`). **Gap:** a task that never reaches a terminal state is
inserted with `expires_at = NULL` (`submitter.rs:404`) and is retained indefinitely.

**Edge cache.** `GET /api/query` responses — which include `unit.metadata`, i.e. **the wallet
address and key names** — are written to the Cloudflare Cache API for 24 hours
(`p256-index-cf/src/edge.rs:736-756`, `:53`).

**IP handling.** The client IP is read once and immediately salted-hashed, truncated to 64 bits
(`edge.rs:758-766`, `:977-985`); only the hash crosses into the DO
(`p256-index-cf/src/proto.rs:8-14`). It is persisted as `rate_counters.key` =
`"create:{ip_hash}"` / `"read:{ip_hash}"` for a 60-second window
(`submitter.rs:147`, `:769-773`, `:630-649`). The salt is derived from the deployment's private
key; when unset it falls back to a hardcoded constant (`edge.rs:983`) — for the hosted default
the key must be set, so this is a self-host risk, not a production one. Under GDPR the stored
value is pseudonymised personal data, not anonymous — but it lives ≤60 s.

**On-chain publication.** The service pays the gas and broadcasts `register(...)` to **Gnosis
Chain (id 100)**: `p256-registrar/src/protocol.rs:27,88`, `:366-384`;
`p256-index-cf/src/chain.rs:465-470`, `:612-639`. What lands on-chain, permanently and publicly,
per member: `publicKey`, `credentialId`, a 20-byte `attestation` whose bytes 1..17 are the
**AAGUID** (`p256-index/contracts/src/WebAuthnP256PublicKeyRegistry.sol:135`),
`authenticatorAttachment`, `transports`, the full proof, and the group `metadata` — i.e. the
wallet address, the wallet name and each key's label. There is **no delete path**; the contract
is append-only (`protocol.rs:5-6`). If Alchemy is configured as the broadcast RPC
(`p256-index-cf/src/config.rs:57-59`) the same payload also passes through Alchemy.

**Correction to an older note:** there is **no commit-reveal** in the v2 codebase. Registration
is a single plaintext `register()` call.

**Logging.** Seven `console_*` calls in the whole worker (`lib.rs:53`, `telegram.rs:62`,
`submitter.rs:1041,1052,1058,1149-1151,1208`). **None can emit an address, public key,
credential ID or IP.** But note that `GET /api/query?publicKey=0x04…` puts a public key **in the
URL**, so any platform-level Cloudflare request log correlates IP ↔ public key even though the
application code never does.

### 2.2 Relay / bundler — `vela-relay-cf.getvela.app` — **STORES and LOGS the address**

**What the app sends.** Both shells:

- `GET /v1/account/{chainId}/{safeAddress}` — **the wallet address is in the URL path**.
  `app-android/.../feature/send/core/RelayClient.kt:89-106`;
  `app-ios/VelaWallet/VelaWallet/Core/RelayClient.swift:235`.
- `vela_getInBandGasQuote` with `{"safeAddress": …}` — **before the user signs anything**, just
  to quote a fee. `RelayClient.kt:129-134`; `RelayClient.swift:269-280`.
- `eth_estimateUserOperationGas` and `eth_sendUserOperation` with the full UserOperation: sender
  address, `call_data` (which encodes recipient addresses and token amounts) and signature.
  Wire type `vela-relay-core/src/task.rs:60-81`.
- An `X-Rpc-Url` request header naming the user's top-ranked RPC endpoint — **which contains the
  user's own provider API key if they added one**.
  `app-android/.../feature/wallet/core/RpcPoolExecutor.kt:287`; `RelayClient.kt:535`.

**Server-side persistence.** No D1, no R2, no Analytics Engine
(`vela-relay-cf/wrangler.jsonc:18-47` declares 1 KV, 3 Durable Objects, 2 Queues).

- **RecordDO**, one per `{chainId}:{userOpHash}` — stores `StoredUserOperation`
  (`vela-relay-core/src/task.rs:163-187`), i.e. sender + calldata + signature verbatim, plus the
  full on-chain receipt including ERC-20 `Transfer` logs with counterparty addresses and amounts
  (`record_do.rs:47`, `lane_do.rs:2543-2560`). **TTL 3600 s**, alarm then `delete_all()`
  (`record_do.rs:20`, `:97-114`). Served **unauthenticated to anyone who knows the hash**
  (`wire.rs:658-670` returns `sender`, `logs`, `receipt`).
- **LaneDO `delayed:{userOpHash}`** — `RoutedUserOperation` (`task.rs:126-147`), sender **and**
  the whole UserOperation, retained for **`max(attempt_ttl, 14 days)`**
  (`lane_do.rs:55` `QUEUE_RETENTION_MS`, applied `:1784-1787`, written `:1400-1412`). This is
  the longest-lived copy and it is on a **normal** path (fee-hold or future-nonce), not an
  exceptional one.
- **LaneDO `intent`** — the signed RLP of `handleOps`, embedding every member UserOperation.
  **No TTL, no sweep**; deleted only on the happy path (`lane_do.rs:411`, `:1146`).
- **Dead-letter queue** — receives the whole payload (`shell.rs:225-229`,
  `lane_do.rs:1502-1507`) and **has no consumer configured** (`wrangler.jsonc:33-46`), so those
  messages sit until Cloudflare's default queue retention expires. No `message_retention_period`
  is set.
- KV holds only `chainmeta:{chainId}`, `usdprice:{SYMBOL}`, `gasprice:*` — no address is ever a
  KV key or value.

**Logging.** One line in 86 emits a user address, and it fires on **every** accepted submission:

> `vela-relay-cf/src/admission.rs:203-205` —
> `"UserOperation accepted into the record store and the durable queue: chain_id={chain_id} entry_point={entry_point} sender={sender_hex} user_operation_hash={user_operation_hash} settlement=in_band"`

Everything else logs the userOp hash, chain id, lane, tx hashes and the relay's own address. No
calldata or signature is ever logged. Caveat: `lastExecutorError` stores upstream RPC error
strings (bounded to 512 bytes, `task.rs:192`) without hex stripping — an upstream that echoes an
address into an error message puts that address into the record and into a `console_warn`.

**IP handling.** **Nothing in the relay reads the client IP.** Repo-wide grep for
`cf-connecting-ip`, `x-forwarded-for`, `request.cf` across `vela-relay-cf/src/` returns zero
hits. There is no rate limiting and no abuse store. Cloudflare's own edge logs are outside the
application.

**Fee accounting.** There is **no per-address fee ledger**. The fee is in-band: the relay parses
the user's own calldata for a transfer to the settlement recipient
(`vela-relay-core/src/settlement.rs:45-48`, markup default 1.4× at `config.rs:169`). Per-operation
amounts live in the record keyed by userOp hash for 1 h — and that record also holds the sender,
so within that hour a joinable "this address paid this amount" row exists. It is not durable and
not indexed by address.

**On-chain linkage.** The relay's own EOAs (HKDF-derived pool of up to 100,
`vela-relay-core/src/vault.rs:56-80`) broadcast `handleOps`, permanently publishing the user's
address. Additionally `vault.rs:104-121` routes a sender to one of 10 relayer lanes
**deterministically by the last 8 hex characters of its address**, so an observer can cluster
which lane a wallet uses, computable offline. That is a service-design choice, not an inherent
property of wallets, and it is worth knowing before claiming the relay adds no linkability.

**Third parties that receive the address and calldata via the relay:** Alchemy, when
`ALCHEMY_API_KEY` is set (`arms/rpc.rs:40-48`, `arms/trusted.rs:388-390`) — tried **before** the
public fallbacks; whatever public RPCs the chain directory lists (`arms/market.rs:66-73`); and
any URL the client names in `x-vela-rpc-url` (`arms/rpc.rs:29-35`).

### 2.3 Chain data — `ethereum-data.getvela.app` — **does not store, but the path is revealing**

The Worker handles exactly one route, `/api/health`, returning a constant
(`ethereum-data/rust/worker/src/lib.rs:28-36`). Everything else is served by Cloudflare Workers
Static Assets, so the Worker is never invoked (`wrangler.toml:21-26`). It declares no KV, D1,
DO, R2 or Analytics Engine, reads no headers and logs nothing.

**But the request paths disclose content:**

- Token logos: `{base}/assets/eip155-{chainId}/{tokenContractAddress}/logo.png`. Built in all
  four shells from the token the user actually holds — Android
  `app-android/.../core/marks/Marks.kt:32-41`, iOS `app-ios/VelaWallet/VelaWallet/Core/Marks.swift:79-82`
  and `Features/Wallet/ActivityExecutor.swift:233-237`. The store carries 7,847 token
  directories on chain 1 alone, so the sequence of logo paths one IP requests is a readable
  approximation of that wallet's holdings. An obscure token is a strong signal. **Do not claim
  "no holdings data is disclosed."**
- ERC-7730 clear-signing descriptors:
  `{base}/erc7730/calldata/eip155-{chainId}/{contract}.json` and
  `/erc7730/eip712/eip155-{chainId}/{contract}.json` —
  `rust/crates/vela-core/src/app/clear_signing.rs:1643-1651`; fetched at
  `app-ios/.../Signing/Core/ClearExecutor.swift:136-141`. **The contract the user is about to
  sign against is in the URL.**
- Chain metadata and chain logos: `/chains/eip155-{id}.json`,
  `/chainlogos/eip155-{id}.png` (`network_admin.rs:1698`).

The wallet address is never sent to this host. Cloudflare's edge sees path + IP.

**Second live deployment:** `ethereum-data.awesometools.dev` still serves installed wallets
(`ethereum-data/wrangler.toml:34-38`). Declare both or retire the old one.

### 2.4 Exchange rates — `vela-currency.getvela.app` — **clean**

`/v2/rates` parses only `base` and `quotes` (`crates/core/src/api.rs:123-135`); the worker reads
no headers at all (`crates/worker/src/lib.rs:53-70`). The apps send the URL verbatim with no
per-user parameter and filter currencies **client-side**, so **the user's chosen fiat currency is
not disclosed** (`app-web/vela-wallet/src/lib/services/fiat-fx.ts:59,83-89`;
`app-ios/VelaWallet/VelaWallet/Core/FiatFx.swift:34,65`). The only persisted state is one global
KV blob `ecb:latest` (`crates/worker/src/lib.rs:17-18,105-111`). No D1, DO, R2 or Analytics
Engine; four `console_*` lines, none carrying a request, IP or header.

One flag: `"observability": { "enabled": true }` (`vela-currency/wrangler.jsonc:25`) turns on
Cloudflare Workers Logs, which retains invocation metadata (URL, method, outcome) account-side.

### 2.5 Authenticator directory — `aaguid-explorer.awesometools.dev` — **UNVERIFIED**

The apps ask it for the human name of an authenticator model when the compiled catalog cannot
answer. The request carries an AAGUID and nothing else; an AAGUID identifies a **product**, not a
person — thousands of users share one. On iOS and Android the origin is the compiled constant and
is **not** settings-overridable (`rust/crates/vela-core-uniffi/src/lib.rs:596`), unlike desktop
and web.

The code asserts *"It is OUR service and stores nothing (founder, 2026-08-26)"*
(`rust/crates/vela-core/src/passkey.rs:243-247`, and verbatim in both shell clients). **No
checkout of that service exists on this machine, so the "stores nothing" claim could not be
verified from source.** It is also on a different domain from every other first-party service,
which a reviewer may read as a third party. See judgement call §6.5.

---

## 3. The bug reporter

**Nothing in either store app POSTs a bug report to any Vela server.** A repository-wide grep
for `api/bug-report` across `rust/`, `app-android/`, `app-ios/`, `app-desktop/` and
`app-web/vela-wallet/` returns **zero call sites** — only i18n keys, specs and docs. Spec 080's
own audit records this: *"Web feedback 'Send' does nothing; the site's `/api/bug-report` has no
caller"* (`specs/080-site-content-accuracy/audit-report.md:85`).

**What Android actually does.** Settings → Feedback → Send builds a `github.com/.../issues/new`
URL and hands it to the system browser via `ACTION_VIEW`
(`app-android/.../navigation/VelaNavHost.kt:1693-1699`). Same for the crash sheet
(`app-android/.../core/diagnostics/CrashSheet.kt:69-74`). The body is the user's typed text plus
five device lines built at `app-android/.../feature/settings/SettingsLive.kt:569-578`: app
version + commit, platform, language, failed chain names, recent failure strings. **No address,
no balance, no key.** The lines are shown in the sheet before the user taps Send, with the
consent string *"Only what you see is sent — never keys, seed phrase, or balances."*

**What iOS does.** `FeedbackSheetBody` renders the preview and has **no send action at all**
(`app-ios/.../Features/Settings/SettingsSheet.swift:307-330`). iOS sends nothing.

**The proxy, for completeness.** `app-web/getvela.app/src/routes/api/bug-report/+server.ts` is
live and unauthenticated. It accepts six optional strings (`:31-38`), validates only body size
(16,000 chars), JSON validity, and a non-empty `what` (`:26,94-107`), sanitizes **only**
`fingerprint` (`:64-67`), and copies `what`/`steps`/`area`/`environment`/`diagnostics` verbatim
into a **public** GitHub issue in `mondaylabsltd/vela-wallet` (`:22`, `:112-122`). The PAT comes
from `$env/dynamic/private` and is never exposed (`:17,21,69-75`); errors log status codes only
(`:179-183`). Rate limiting is an **in-memory `Map` keyed on the client IP**, 5 per 10 minutes
(`:28-29,40-55,83-90`) — ephemeral, isolate-local, never written to storage; the IP is never
logged and never forwarded to GitHub.

**For the form:** because no shipped store binary calls it, `/api/bug-report` is **not** an app
data flow today. It should be stated in the App Review notes only if a shell starts using it.
The scrubbing guarantee the old header comment made lived entirely in a client that no longer
exists, so if a shell is re-wired to this route, the scrubbing must be re-implemented
server-side first.

---

## 4. Tracking — proof of absence, from manifests

| Shell | Manifest read | Finding |
|-------|---------------|---------|
| iOS | `app-ios/VelaWallet/VelaWallet.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved` | **Exactly one third-party package: `lottie-ios` (airbnb).** No Firebase, Sentry, Bugsnag, Amplitude, Mixpanel, Segment, AppsFlyer, Adjust, Branch, Facebook SDK. No Podfile. |
| iOS | `app-ios/VelaWallet/VelaWallet/Info.plist` | **No `NSUserTrackingUsageDescription`** → no ATT prompt, no tracking SDK. |
| Android | `app-android/vela-wallet/app/src/main/AndroidManifest.xml` + merged manifest `.../build/intermediates/merged_manifest/debug/processDebugMainManifest/AndroidManifest.xml` | **No `com.google.android.gms.permission.AD_ID`.** No Firebase / Crashlytics / GMS ads. No `google-services.json`. |
| Android | `app-android/vela-wallet/app/build.gradle.kts:231-279`, `gradle/libs.versions.toml`, `settings.gradle.kts:19-26` | Dependencies are androidx + kotlinx + zxing + lottie + okhttp + jna. The **only** `com.google.android.gms` entry is `play-services-fido` (passkeys). No `google-services.json` anywhere in the repo. |
| Android | `app-android/vela-wallet/app/build/outputs/apk/debug/app-debug.apk` (19 dex files scanned) | `com/google/firebase`, `crashlytics`, `gms/measurement`, `gms/ads`, `gms/analytics`, `io/sentry`, `com/bugsnag`, `com/appsflyer`, `com/adjust/sdk`, `io/branch`, `com/amplitude`, `com/mixpanel`, `com/segment`, `com/facebook`, `AdvertisingIdClient` — **0 hits each.** This is a *debug* APK; see §7 #5. |
| Android | manifest-merger blame report | The only library-injected permissions are `USE_BIOMETRIC`/`USE_FINGERPRINT` (androidx.biometric 1.1.0) and `WAKE_LOCK`/`ACCESS_NETWORK_STATE`/`RECEIVE_BOOT_COMPLETED`/`FOREGROUND_SERVICE` (androidx.work 2.10.5). Neither library reports to a server. |
| Android | `app-android/.../core/diagnostics/VelaLog.kt:37,52-55,67` | The local diagnostic log is a rotated 512 KiB file and is a **no-op in release builds**. Nothing is uploaded. |
| iOS | `app-ios/VelaWallet/VelaWallet/Core/TrackerNotifier.swift:39,46,68` | Notifications are **local only** (`UNUserNotificationCenter.add`). No `registerForRemoteNotifications`, no APNs, no FCM, no `GoogleService-Info.plist`. |
| Desktop (out of store scope) | `app-desktop/vela-wallet/Cargo.lock` (1,108 crates) | `sentry`, `opentelemetry`, `telemetry`, `analytics`, `posthog`, `amplitude`, `mixpanel`, `datadog`, `bugsnag`, `crashpad`, `breakpad`, `minidump` — 0 hits each. Zed's telemetry crates are not in the gpui graph. |
| Web wallet (out of store scope) | `app-web/vela-wallet/package.json:55-62`, `pnpm-lock.yaml` | Runtime deps are `@noble/*`, `zbar-wasm`, `jsqr`, `qrcode`, `xlsx`. `@opentelemetry/api` appears only as an *optional peer declaration* of SvelteKit/Vitest and is never resolved. Source grep for `sendBeacon`/`gtag`/GTM/plausible/posthog/Sentry/amplitude/mixpanel/segment/datadog/fathom/umami/matomo → 0 hits in `src/`. |
| Rust core | `rust/Cargo.toml:18-72`, `rust/Cargo.lock` (399 crates) | No telemetry crate; `sentry`/`opentelemetry`/`telemetry`/`reqwest`/`hyper` = 0 hits. The core has no clock and no network; shells own all transport. |

**In-app "metrics" are counters in RAM.** `app-web/vela-wallet/src/lib/services/metrics.ts:10-23`
is a `Map<string, number>` with no transport; the native equivalents feed only the feedback
preview lines.

**The marketing site is different, and a reviewer may notice.** `getvela.app` loads **Rybbit**
analytics from `https://tj.appsdata.org/api/script.js`
(`app-web/getvela.app/src/routes/+layout.svelte:36`) on every page except `/chain-setup`, with
custom event tracking on CTAs, language switches and download clicks, plus **Google Fonts**
(`+layout.svelte:29-34`). Both are disclosed in the published policy
(`privacy/+page.svelte:194-203`). **The web wallet at `wallet.getvela.app` is clean** — zero hits
for any analytics package or beacon across `app-web/vela-wallet/src`. Neither is part of the
store binaries, and neither belongs in either store form. Say so plainly in the App Review notes
so the reviewer does not read the site's tracker as the app's.

---

## 5. Permissions and platform declarations — corrections needed

These are not privacy-label answers but they are attested alongside them, and the current sheet
is wrong about all three.

1. **Bluetooth is back, on both platforms.** Android requests `BLUETOOTH_SCAN`
   (`neverForLocation`), `BLUETOOTH_CONNECT`, and legacy `BLUETOOTH`/`BLUETOOTH_ADMIN` +
   **`ACCESS_FINE_LOCATION` at `maxSdkVersion="30"`**
   (`app-android/vela-wallet/app/src/main/AndroidManifest.xml:59-71`). iOS declares
   `NSBluetoothAlwaysUsageDescription` (`app-ios/VelaWallet/VelaWallet/Info.plist:93-94`). Both
   are for the caBLE "sign in with your phone" passkey flow (spec 019). The current review notes
   say "No Bluetooth" in both stores — **that statement is false and must be removed.**
2. **`ITSAppUsesNonExemptEncryption` is not set anywhere.** It is absent from
   `app-ios/VelaWallet/VelaWallet/Info.plist` and from `project.pbxproj`. The sheet's "✅ Done"
   referred to the Expo `app.json`, deleted in spec 039. Add it to the hand-maintained
   `Info.plist` or expect the export-compliance question on every upload.
3. **Other iOS declarations to keep in step:** `NSCameraUsageDescription` (QR scanning),
   `NSPhotoLibraryAddUsageDescription` (saving the receive QR),
   `UIBackgroundModes = [fetch]` and `BGTaskSchedulerPermittedIdentifiers =
   [app.getvela.VelaWallet.tracker]` (`Info.plist:84-101`). Android additionally requests
   `POST_NOTIFICATIONS` for local confirmation notifications — **no push service, no FCM**.
4. **Pre-empt one reviewer misreading.** The iOS background-task identifier is
   `app.getvela.VelaWallet.**tracker**` — that is the *transaction-receipt* tracker, not an
   advertising or analytics tracker. Say so in one line in the App Review notes; nothing else in
   the binary supports a tracking reading (§4).
5. **A stale comment to fix (accuracy, not policy).** `AndroidManifest.xml:19-24` says
   *"ACCESS_NETWORK_STATE is deliberately NOT requested"* — but it **is** in the shipped merged
   manifest, injected by `androidx.work:work-runtime:2.10.5`
   (`app-android/.../build/outputs/logs/manifest-merger-debug-report.txt:669`). Harmless for the
   form; misleading to anyone auditing the source manifest alone.

---

## 6. Recommended form answers, with the evidence in one line each

### 6.1 Apple — App Privacy

| Apple data type | Collected | Linked | Tracking | One-line evidence |
|---|---|---|---|---|
| **Identifiers → User ID** | **Yes** | **Yes** | No | The wallet address and the user-typed wallet name are stored by the index for 7–30 days (`p256-index-cf/src/submitter.rs:399-411`, `:57-58`) and published permanently on Gnosis (`p256-registrar/src/protocol.rs:27,88`); the relay logs the address on every submit (`vela-relay-cf/src/admission.rs:203-205`). |
| **Financial Info → Other Financial Info** | **Yes** | **Yes** | No | The relay receives and stores the full signed UserOperation — sender, calldata (recipients and amounts), signature — for up to 14 days (`vela-relay-cf/src/lane_do.rs:1400-1412`, `:1784-1787`) and the on-chain receipt with ERC-20 transfer logs (`:2543-2560`). |
| **Identifiers → Device ID** | No | — | — | No advertising ID, no vendor ID, no device fingerprint; no `AD_ID` permission, no ATT string (§4). |
| **Contact Info** (name, email, phone, address, other) | No | — | — | No field in any onboarding or settings screen asks for them. |
| **Health & Fitness, Sensitive Info, Contacts, Location** | No | — | — | No permission requested (iOS `Info.plist:84-101`); Android's `ACCESS_FINE_LOCATION ≤ SDK 30` is a pre-API-31 BLE-scan prerequisite and no location value is read or transmitted. |
| **Browsing History** | No | — | — | The in-app browser's history is one entry per origin, stored on device (`rust/crates/vela-core/src/app/browser_history.rs:11-16`); the explore document likewise (`explore_sites.rs:21-26`). |
| **Search History** | No | — | — | No search query is sent to any first-party host. |
| **Purchases**, **Payment Info** | No | — | — | No IAP, no card, no fiat on-ramp. (Crypto balances are declared under Other Financial Info, not here.) |
| **Usage Data**, **Product Interaction**, **Advertising Data** | No | — | — | No analytics SDK in any manifest (§4); in-app counters never leave RAM. |
| **Diagnostics → Crash Data / Performance Data / Other Diagnostic Data** | **No** (see §6.5 #4) | — | — | iOS has no send path at all (`SettingsSheet.swift:307-330`); Android hands a prefilled URL to the system browser (`VelaNavHost.kt:1693-1699`). No shipped binary POSTs diagnostics anywhere. |
| **Contacts** (the address book feature) | No | — | — | Contacts are the user's own saved wallet addresses, stored on device; they are never uploaded. Note the index may be asked for a saved address's registered wallet name (`registry_lookup.rs:19-24`) — that is a lookup *about* an address, covered by the Identifiers declaration. |

**Used for Tracking: No, for every single item.** Nothing is joined with data from other
companies' apps or websites, nothing is sold, nothing is used for advertising.

**Purpose for every declared item: App Functionality.** Without sending the operation there is no
transaction; without registering the key set there is no cross-device recovery.

### 6.2 Google Play — Data Safety

**Does your app collect or share any required user data types?** → **Yes.**

| Play data type | Collected | Shared | Ephemeral | Req/Opt | Purpose | Evidence |
|---|---|---|---|---|---|---|
| **Financial info → Other financial info** (wallet address, transactions) | Yes | **Yes** | No | Required | App functionality, Account management | Relay stores/logs the address and operation (`admission.rs:203-205`, `lane_do.rs:1400-1412`); index publishes the address on a public chain (`protocol.rs:88`). |
| **Personal info → User IDs** (wallet address as account id, public key, credential ID) | Yes | **Yes** | No | Required | Account management, App functionality | `submitter.rs:399-411` (stored), `protocol.rs:27,88` (published on-chain forever). |
| **Personal info → Other info** (user-chosen wallet and key names) | Yes | **Yes** | No | Required | Account management | `registry_metadata.rs:49` `key_names`; published on-chain. |
| **App activity / App info & performance** | **No** | — | — | — | — | No diagnostics leave the app (§3). |
| **Device or other IDs** | No | — | — | — | — | No advertising ID, no device ID (§4). |
| Location, Contacts, Calendar, Photos/Videos, Audio, SMS/Call logs, Health, Web browsing history, Installed apps | No | — | — | — | — | §4, §5, `browser_history.rs`. |

**Security practices.**
- Encrypted in transit → **Yes.** Every endpoint is `https://`; the passkey ceremony is
  OS-provided.
- Data deletion request path → **Yes**, `hello@mondaylabs.ltd`, with the honest caveat that the
  on-chain registry record is append-only and cannot be deleted by anyone
  (`p256-registrar/src/protocol.rs:5-6`). Settings → Erase this device clears on-device data on
  Android; it is not yet complete on iPhone (`privacy/+page.svelte:172-177`) — **do not claim
  otherwise on the iOS form.**
- Independent security review → **No.** No third-party audit exists and none is scheduled.
- Play Families → **No.**

**Other Play "App content" declarations.** Financial features: *non-custodial crypto wallet
(stores/holds crypto)* only — do **not** check exchange/buy/sell/trade. No ads. Adults 18+.
Permissions: Camera, Bluetooth (scan + connect, `neverForLocation`), legacy Bluetooth + fine
location capped at SDK 30, notifications, vibrate — see §5.

### 6.3 `PrivacyInfo.xcprivacy` reconciliation — **one blocker**

The manifest lives at `app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy`.

1. ~~**It is not on `main`.**~~ **Corrected 2026-09-23: it is.** `505dc3e2` landed on `main`
   with PR #309, and `git merge-base --is-ancestor 505dc3e2 origin/main` says so. The earlier
   reading was taken against a *local* `main` that was thirty commits behind its remote — the
   check is only as good as the ref it is run against, so run it against `origin/main`.
2. **Its content disagrees with the label in two places**, and a mismatch between the manifest
   and the nutrition label is itself a rejection reason:
   - `NSPrivacyCollectedDataTypeLinked` is `<false/>` for Other Financial Info. The manifest's
     own comment flags this as an open question. On the evidence in §2 — the relay logs the
     address, the index stores it and the chain publishes it — **it should be `<true/>`**.
   - It declares **only** `NSPrivacyCollectedDataTypeOtherFinancialInfo`. If the label declares
     Identifiers → User ID (§6.1), the manifest needs a second entry:
     `NSPrivacyCollectedDataTypeUserID`, Linked `true`, Tracking `false`, purpose
     `AppFunctionality`.
   - `NSPrivacyTracking = false` and an empty `NSPrivacyTrackingDomains` are correct and
     evidenced (§4). The `NSPrivacyAccessedAPITypes` entries (UserDefaults CA92.1, FileTimestamp
     C617.1) were measured from the archived binary and are out of scope here; leave them.

### 6.4 Where the published privacy policy already commits you

`app-web/getvela.app/src/routes/privacy/+page.svelte` is linked from both listings, so a reviewer
can and will read it. It already states that the index *"receives your registration record and
your IP address"* and keeps it 7/30 days (`:102-107`), that the relay *"receives your wallet
address and each transaction you send… logs your address and each operation's hash, keeps
operations for between one hour and 14 days"* (`:110-117`), and that the on-chain record contains
the wallet address, the wallet name and each key's label, permanently (`:55-89`). **Every one of
those statements checks out against the service code.** A nutrition label that says "no financial
data collected" while the linked policy says this is the specific kind of contradiction App
Review escalates.

### 6.5 The judgement calls

These are the decisions only the founder can make, because he signs the attestation. Both
readings are given with their consequence.

1. **Is a wallet address a "User ID" under Identifiers?**
   *Reading A (recommended — declare):* it is the account-level identifier for this app; Apple's
   own wording is "account ID … or other user- or account-level ID that can be used to identify a
   particular user or account". It is stable, it is stored server-side, and it is published
   on-chain with a user-chosen name next to it.
   *Reading B (do not declare):* it identifies a key set, not a person; no real-world identity is
   ever attached.
   *Consequence of B being wrong:* the label contradicts the linked privacy policy and the
   on-chain registry, both of which a reviewer can check in minutes. Under-declaration is a
   Guideline 5.1.1 rejection and, on a repeat, an account-level action. **Recommend A.**

2. **Is on-chain balance/transaction data "Other Financial Info"?**
   *Reading A (recommended — declare):* the relay receives, stores and logs the transaction and
   the address that sent it. This is not "the blockchain is public"; it is our server holding it
   for up to 14 days.
   *Reading B (do not declare):* the data is intrinsically public and the wallet is only a
   viewer.
   *Consequence of B being wrong:* same as #1, and worse, because the relay's retention is
   documented in our own policy. **Recommend A.**

3. **Are the default RPC hosts "third-party partners" in Apple's sense?**
   The form asks about data collected "by you or your third-party partners". The default RPCs
   (§1.2) are public endpoints operated by others; each one receives the wallet address and the
   IP on every balance read. They are replaceable in Settings
   (`network_admin.rs:2784-2795`).
   *Reading A (they are incidental infrastructure):* they are not integrated SDKs, there is no
   contract, no payment and no data-sharing agreement; **20 of the 24 defaults are the chain's
   own foundation/operator endpoint** (§1.3), which is the network itself rather than a vendor;
   and the user can point the app at their own node. The same status as a DNS resolver or an ISP.
   *Reading B (they are partners):* they are chosen by us, shipped as defaults, and the
   overwhelming majority of users will never change them — and 4 of the 24, plus the whole
   fallback tier, are commercial aggregators (publicnode, dRPC, 1RPC).
   **My reading: A** — replaceability plus the absence of any commercial or contractual
   relationship makes them user-directed infrastructure, and Apple's "partner" concept targets
   integrated third-party code and services, of which there is none. **But this changes nothing
   about the outcome**, because Vela's *own* relay and index already force a "Yes" on both
   Identifiers and Financial Info. The RPC question only decides how the answer is explained in
   the review notes, not what is ticked. Note that replaceability is a *mitigation*, not an
   exemption: the default is what ships, and the default sends the address to a third party.
   Note also that Alchemy, Ankr and dRPC-with-a-key are contacted **only** if the user enters
   their own API key (`network_admin.rs:544-546, 665-683`) — that is unambiguously
   user-directed, and unambiguously not a partner relationship.

4. **Diagnostics: declare or not?**
   Today no store binary transmits diagnostics (§3), so the honest answer is **Not collected**.
   *Reading B (declare anyway, Not Linked, App Functionality):* belt-and-braces, and it stays
   true if the in-app reporter is ever wired to `/api/bug-report`.
   *Consequence:* over-declaring is not a rejection but it is inaccurate today, and it invites a
   reviewer question ("where does this go?") you would have to answer with "nowhere yet".
   **Recommend: Not collected now; update the label in the same release that wires the
   reporter** — and re-implement the scrubbing server-side first, because the client that
   guaranteed it is gone.

5. **`aaguid-explorer.awesometools.dev` — first party or third party?**
   The code and the policy both call it the founder's own service that stores nothing
   (`passkey.rs:243-247`; `privacy/+page.svelte:128-131`), but **that claim could not be verified
   from source on this machine**, and the domain is not `getvela.app`. It receives a product
   model identifier, not a user identifier, so it does not change any label answer either way.
   *Action:* either produce the service's source and confirm "stores nothing", or move it onto
   `getvela.app`, or treat it as a third-party recipient in the policy. The current
   disclosure is already accurate; the gap is verification, not honesty.

6. **Play "Shared" for the on-chain record.**
   Google excludes transfers to service providers and user-initiated transfers. Publishing the
   wallet address, the wallet name and every public key to a permanent public ledger is neither.
   **Mark the Personal info and Financial info items as Shared.** Address-to-RPC is the borderline
   part and, on the reading in #3, is user-directed — but since the on-chain publication already
   forces "Shared", the distinction has no effect on the form.

7. **IP address under Play.**
   Vela's code stores the raw IP nowhere. The index keeps a salted 64-bit hash for ≤60 s
   (`edge.rs:758-766`; `submitter.rs:630-649`); the bug-report proxy keeps an in-memory map that
   no shipped app calls; the relay reads no IP at all. Cloudflare, as processor, sees the IP on
   every request regardless, and `vela-currency` has Workers Logs on.
   **Recommend: do not declare IP as collected** (Play permits treating purely-ephemeral
   connection data as not collected), and keep the existing privacy-policy disclosure, which
   already names Cloudflare (`privacy/+page.svelte:97-99`).

---

## 7. What could not be determined, and why

1. **Cloudflare platform-level logging retention** for `p256-index` and `vela-relay`. Neither
   `wrangler.toml`/`wrangler.jsonc` sets `observability`, `logpush` or `tail_consumers`, so
   whether Workers Logs is on — and for how long it keeps the line at
   `vela-relay-cf/src/admission.rs:204`, **which contains the user's wallet address** — is an
   account-level dashboard setting, not a repository fact. `vela-currency` explicitly enables it
   (`wrangler.jsonc:25`). **This must be checked in the Cloudflare dashboard before signing**,
   because it decides the retention period you state for the address log.
2. **Custom-domain bindings.** None of the three worker configs contains `routes` or
   `custom_domain`; the mapping of `p256-index-v2.getvela.app`, `vela-relay-cf.getvela.app` and
   `ethereum-data.getvela.app` to these workers is dashboard-configured. The client-side evidence
   is strong (health-identity match: `p256-index-cf/src/edge.rs:178` returns
   `"webauthn-p256-publickey-registry"`, which is exactly what the wallet accepts at
   `app-web/vela-wallet/src/lib/onboarding/core/registry.ts:23-24`), but a one-line `curl` of each
   `/api/health` would close it for a legal filing. No network calls were made for this audit.
3. **Whether `ALCHEMY_API_KEY` is set in the production relay and index.** If it is, Alchemy
   receives the user's address and calldata (relay) and the registration payload (index). This is
   a secret-store fact, not visible in any repo. The published policy already names Alchemy
   (`privacy/+page.svelte:113-114`), so the disclosure is safe either way — but the App Review
   answer "which third parties receive user data" depends on it.
4. **`aaguid-explorer.awesometools.dev`'s storage** — no source checkout on this machine (§6.5 #5).
5. **Only debug artifacts exist in the tree.** The Android permission set and the dex scan in §4
   come from a *debug* merged manifest and a *debug* APK; no release AAB/APK, `.ipa` or
   `.xcarchive` is present. The dependency sets are identical (there is no `releaseImplementation`
   in `app/build.gradle.kts:231-279`), but re-verify the release artifact before submission:
   `bundleRelease` then `bundletool dump manifest`, and confirm `AD_ID` is still absent.
6. **`ethereum-data.awesometools.dev`**, the older live deployment, was not audited separately; it
   is documented only in a comment (`ethereum-data/wrangler.toml:34-38`).

---

## 8. Changes this evidence forces in `privacy-and-review.md`

| § | Current text | Status |
|---|---|---|
| 0 | "Vela's own servers do **not** store the wallet address" | **False.** Index stores it 7–30 days and publishes it on-chain; relay stores it up to 14 days and logs it on every submit. |
| 0 | "Balances / tx history / RPC prefs — No — on-device `AsyncStorage`" | Stale vocabulary (Expo deleted in spec 039) and incomplete — the relay holds the operation and its receipt. |
| 0 | "`NSPrivacyTracking=false` already set in `ios/VelaWallet/PrivacyInfo.xcprivacy`" | Path no longer exists; the manifest is at `app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy` **and is not on `main`**. |
| 1.B | Judgement framed as "safe to declare even though we store nothing" | The premise is wrong; declaring is now the *accurate* answer, not the cautious one. |
| 1.C | Diagnostics "Collected: Yes" | No store binary transmits diagnostics. Should be **Not collected** today (§6.5 #4). |
| 1 | `ITSAppUsesNonExemptEncryption` "✅ Done" in `app.json` | `app.json` is deleted; the key is in neither `Info.plist` nor `project.pbxproj`. |
| 4, 5 | Review notes: "No Bluetooth", "dApp Connect pairs … WalletConnect-style relay" | Bluetooth **is** used for caBLE on both platforms (§5). WalletPair/dApp-session pairing was dropped by founder ruling 2026-09-08 — describing a feature the binary does not have is its own review risk. |
| 6 | "Removed all `BLUETOOTH*` … `ACCESS_FINE_LOCATION`" | Reversed by spec 019; both are back in `AndroidManifest.xml:59-71`. |
| 2 | Play: no deletion caveat for iOS | Erase-this-device is not complete on iPhone (`privacy/+page.svelte:172-177`); do not claim a full in-app deletion path on the iOS side. |
