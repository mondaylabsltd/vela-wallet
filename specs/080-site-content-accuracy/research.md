# Research — 080

**Baseline**: `main` at `dd482131` (2026-09-21) plus the service repositories at
their `origin/main` (vela-relay `030f2ff`, p256-index, vela-currency,
ethereum-data). Read-only verification by five parallel reports on 2026-09-22:
protocol/keys/fees, self-hosting (with sub-reports for services, desktop,
iOS/Android, web), shipped state per platform, privacy/terms data flows,
external citations. Every fact below was re-read at its citation by at least
one report; the load-bearing ones were re-read again while writing this.

Paths: `core/` = `rust/crates/vela-core/src/`.

## 1. Protocol and keys

| Fact | Value | Evidence |
|---|---|---|
| Built-in networks | **24 mainnets, no testnets**: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume, XRPL EVM | `core/app/network_admin.rs:206-439` (`BUILTIN_CHAINS: [_; 24]`); web `chains.ts:45-372`; iOS `ChainCatalog.swift:51-149`. 12 → 24 on 2026-09-17 (specs 060/061) |
| Gas with no native coin | Tempo only (pathUSD). Arc and Stable pay gas in their native coin (USDC / USDT0) | `core/app/fee_policy.rs:165,179`; `chains.ts:178,207-213` |
| Account | Safe **v1.4.1** (SafeL2 `0x29fc…C762`, factory `0x4e1D…ec67`), ERC-4337 **EntryPoint v0.7**, Safe4337Module `0x75cf…c226` (v0.3.0 per canonical deployments), SafeModuleSetup `0x2dd6…5b47`, MultiSend `0x3886…B526` | `core/safe.rs:23-37`; relay accepts only EP v0.7 (`vela-relay-core/src/admission.rs:26`) |
| **Fallback handler** | **The Safe4337Module**, not CompatibilityFallbackHandler (which is only checked at network admission) | `core/safe.rs:259` |
| Signers | Key 1 → **SafeWebAuthnSharedSigner** v0.2.1 `0x94a4…55c2` (key stored in the Safe's own storage). Keys 2–7 → a per-key **SafeWebAuthnSignerProxy** created by **SafeWebAuthnSignerFactory** v0.2.1 `0x1d31F259eE307358a26dFb23EB365939E8641195` (singleton `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`), deployed inside the setup MultiSend on first deployment | `core/safe.rs:32-43,173-282` |
| Keys per wallet | **1–7, threshold 1 (1-of-n)** | `core/safe.rs:43,252,314-318` |
| Adding / removing / replacing keys | **Not possible in any shell.** No `addOwner`/`swapOwner`/`removeOwner` anywhere; replace-signer dropped in spec 062 | `core/app/create_wallet.rs:15-17`; grep across shells |
| Address derivation | CREATE2 over SafeProxyFactory; `saltNonce = keccak(x₀‖y₀‖…)` over **all founding keys** (key 0 pinned, the rest sorted); one key = the legacy single-key address | `core/safe.rs:285-374` |
| Why a later key cannot be added | The counterfactual address on every chain not yet deployed is derived from the founding set; adding an owner on one deployed chain would leave the owner sets diverging across chains. Vela therefore exposes no owner changes | derived from the above |
| Self-calls to the Safe | **No guard found** that blocks a dApp-built call from the Safe to its own owner/module/fallback functions. EIP-1193 requests cannot carry `DELEGATECALL`; the wallet builds only CALLs (and MultiSend via delegatecall for its own batches) | report "not found" at `sign_request.rs`, `approval_guard.rs` |
| P-256 verification | **RIP-7212 precompile at `0x100`, required; no fallback verifier.** The address `0x100` is part of the signer config and therefore of every wallet address | `core/app/network_admin.rs:2034-2036,136-146`; `core/safe.rs:45-54,205,224` |
| Network admission | 11 contracts + the precompile probed with a real signature | `core/app/network_admin.rs:97-146`; mirrored by `getvela.app/src/lib/chain-setup/required-contracts.ts` |
| Deployment | Counterfactual; the Safe (and the extra signer proxies, ~119k gas each) deploys on its first UserOp per chain, **paid by the user inside that operation's fee** | `core/user_op.rs:259-301`; `core/safe.rs:39-42` |
| Sign-in with one key | One assertion → two candidate public keys → index `p256-index-v2.getvela.app` → if silent, registry contract `0x94fD…EA9` on Gnosis then Ethereum → group rebuilt only if members recompute to the recorded address. Two-signature on-device recovery works **for single-key wallets only** | `core/app/login.rs:402-470,584-594,671-760`; `core/registry_chain.rs` |
| rpId | Native shells: `getvela.app`, hard-coded. Web: `getvela.app` on `getvela.app`/`*.getvela.app`, the extension, and SSR; **any other host uses its own hostname** → different passkeys → a different wallet. The Chrome extension asserts `getvela.app` through `host_permissions`. `app-web/clearsigning`: same rule | web `passkey.ts:61-99`; `extension/manifest.json:30`; `clearsigning/app.js:29-38` |
| Self-built native apps | "This device" passkeys need the app to be signed by Vela's team (Associated Domains / Digital Asset Links). **Hybrid (phone QR) and USB security keys run through vela-core's own CTAP and work in any build.** iOS security keys: USB-C/Lightning via smart-card, YubiKey firmware ≥ 5.8. No native NFC path | README "A phone app you built yourself"; `ctap_bridge.rs:26-32`; spec 063 §4 |

## 2. Fees (the in-band model)

| Fact | Value | Evidence |
|---|---|---|
| How the relay is paid | **In band.** Every UserOp declares zero EntryPoint fees; a transfer to the relay's settlement address sits inside the MultiSend the passkey signs. The relay rejects anything else | `core/user_op.rs:239,1345`; `fee_policy.rs:2449`; relay `admission.rs:608-613`; relay `docs/fees.md` |
| What the wallet pays (standard chains) | `max( 3 × padded gas limits × max(C, R), floor )` — C = the wallet's own gas-price reading `max(eth_gasPrice, base+tip)`; R = the relay's price for the chosen **speed** (default **fast**: 1.8×base + 2×tip; standard 1.2×base + 1.25×tip; slow 0.9×base + tip). Padded limits = verification ×1.5 (floor 300k deployed / 2M undeployed) + call ×1.5 (floor 100k) + pre-verification + 10k | `core/app/fee_policy.rs:85,747-845,945-966,2109-2118,2340-2352`; relay `gas_math.rs:273-318`; default tier `core/app/fee_tier_pref.rs:57` |
| Consequence | The quoted fee is a multiple of the gas the transaction is expected to use at the current price, typically well above 3×; the relay keeps the difference (no refund) | derived; relay has no refund path |
| Quote sanity cap | A relay quote above 3× the wallet's own reading is refused | `fee_policy.rs:92,2109-2113` |
| Tempo | `2 × realistic static gas × pathUSD gas price`, floor $0.01 | `fee_policy.rs:179-184,1172-1188` |
| Minimums | $0.01 worth of the native coin (never below 0.00001 coin; flat 0.001 coin if nobody can price it); $0.01 in stablecoins | `fee_policy.rs:747-820` |
| Relay's own requirement | `max(1.4 × allocated gas × cap, floor)` | relay `src/utils/config.rs:13` |
| Fixed at signing | Yes — amount and recipient are inside the signed callData | above |
| Fee tokens | Native coin always; a USD stablecoin from the chain's `stables` list when the relay can price the native coin; zero balances hidden; Tempo: pathUSD only | relay `quote.rs:143-155,280-292`; `fee_policy.rs:2666-2671` |
| Speed | **A speed choice exists** (Settings → Transaction speed; default fast) — the docs' "there is no speed picker" is stale | `fee_tier_pref.rs`; desktop settings menu; specs 068/069 |
| Paymaster / sponsorship | **None.** No sponsor code in the relay; the shells answer the sponsorship step "denied" | `bundler-service.ts:394-414` dead; `SignExecutor.*` |
| **Per-wallet "gas account" / activation deposit** | **Does not exist any more.** The relay has no per-user account | relay `vela-relay-cf/src/http.rs:124-218`; `admission.rs` |
| What replaced it | When a relay's **shared treasury** on a chain is empty, Send shows a sheet: on built-in chains "Vela's operator runs this relayer and it is out of gas — report it", with an optional, **non-refundable** "start it yourself" contribution (floor 0.0001 native); on custom networks funding is up to the user. Desktop still shows the old "your fee reserve" strings (product bug) | `core/app/send.rs:2041,3816-3862,4562-4612`; relay `treasury.rs:13-16`; desktop `flows/live.rs:1770-1806` |

## 3. Services — what the wallet contacts, and what can be replaced

| Service | Default | Replaceable | Notes |
|---|---|---|---|
| Relay (ERC-4337 bundler with in-band settlement) | `https://vela-relay-cf.getvela.app/{chainId}` | Settings → Service Endpoints → Vela Relay (web, desktop, Android; **iOS page not wired**) | Must be **vela-relay** (Vela-specific quote method). Rust; Docker (Redis + Iggy) or Cloudflare Workers Paid (3 DOs, 2 queues, KV). One `OPERATOR_SECRET` derives one treasury + up to 100 relayer EOAs, same addresses on every chain; **fund the treasury on each chain**. Health `/api/health` → `service:"vela-relay"`. **Hard-codes `ethereum-data.getvela.app`** for chain metadata (`src/utils/rpc.rs:15`). Source Dockerfile looks broken by reading (misses workspace members); CI image uses `Dockerfile.release`. MIT |
| Public-key index (registration + lookup) | `https://p256-index-v2.getvela.app` | Settings (web, desktop after restart, Android for onboarding; **Android name lookups always use the default; iOS only via the sign-in endpoint sheet**) | `mondaylabsltd/p256-index`. Rust; Docker (Redis + Iggy) or Cloudflare Worker (1 DO + cron). Writes to the **canonical registry `0x94fD…EA9` on Gnosis** (permissionless); needs a funded Gnosis `PRIVATE_KEY`, `P256_INDEX_CONTRACT_ADDRESS`, and **`P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (undocumented in its README; without it every write fails)**. Health `service:"webauthn-p256-publickey-registry"`, `status` must be `ok`. **No LICENSE file** |
| Registry contract | `WebAuthnP256PublicKeyRegistry` V13 at `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; same address on Ethereum and Base via CREATE2) | Not replaced — reused. Read directly by the wallet when the index is silent | No owner, no proxy, append-only; stores the whole signed registration call. Legacy index `0xdd93…E9c3` is read-only history |
| Chain data | `https://ethereum-data.getvela.app` | Settings (all four fields honoured where the page works) | `atshelchin/ethereum-data`, MIT. Static files + tiny wasm Worker, or Docker, or Bun. Complete README. Vela-specific fields (`stables`, `dex`, …) the relay depends on are not in its README schema |
| Exchange rates | `https://vela-currency.getvela.app/v2/rates?base=USD` | Settings → Fiat rates | `mondaylabsltd/vela-currency`, MIT, no secrets, complete README; any Frankfurter-compatible USD-based source works. Web/iOS/Android ask Chainlink FX feeds first |
| RPC | Built-in per chain + public fallbacks + optional Alchemy/dRPC/Ankr keys | Per-network RPC override (web, desktop, Android; iOS read-only) | Android uses one RPC per chain (no failover; provider keys unused) |
| AAGUID directory | `aaguid-explorer.awesometools.dev` | Not configurable | Names authenticator models; bundled catalog answers first; optional |
| Selector lookups | sourcify / openchain / 4byte | Not configurable | Third-party; last-resort decoding |
| Phone sign-in tunnel | `cable.ua5v.com` (Google) / `cable.auth.com` (Apple) | Not configurable | Standard caBLE; not Vela's |
| getvela.app itself | Passkey rpId; `.well-known` association files for the store apps; privacy/terms links | **Not replaceable** | See §3a |

### 3a. What cannot be self-hosted, and the ways around it

A passkey is bound to its relying-party ID, `getvela.app`. Consequences, all
verified above:

- A copy of the web wallet served from **your own domain** creates passkeys for
  *that* domain — a different wallet. It cannot sign for a wallet created at
  wallet.getvela.app.
- Ways to keep signing for an existing `getvela.app` wallet **without
  getvela.app online** or without Vela's builds:
  1. **The Vela Chrome extension**, loaded from a release zip or built from
     source — asserts `getvela.app` through host permissions; any key type the
     browser can reach (this device's passkey, USB/NFC key, phone by QR).
  2. **The signing page as a Chrome extension** (`app-web/clearsigning`,
     loaded unpacked from source) — same rpId rule; **not published**.
  3. **A self-built desktop or phone app** — phone-by-QR (hybrid) and USB
     security keys work in any build; "this device" passkeys do not.
  4. The store/notarized apps' "this device" passkey depends on Apple/Google
     fetching `getvela.app/.well-known/*`; the other two methods do not.
- If `getvela.app` changed hands, its new owner could serve pages that request
  assertions for `getvela.app` passkeys. The browser/OS prompt does not show
  the transaction. This is an inherent property of WebAuthn, not a bug to fix,
  and it belongs in the threat model.

### 3b. Honest per-platform state of the endpoint settings (product gaps)

| Shell | Service endpoints | Per-network RPC |
|---|---|---|
| Web | Chain data, relay, fiat honoured; **the passkey index is ignored for create and sign-in** (`setRegistryUrl` never called), used for name lookups | Yes |
| Desktop | All four; the passkey-index change applies after restart/sign-out; one field saves on each keystroke without a health check | Yes (explorer override ignored) |
| Android | All four; **name lookups ignore the index override**; provider keys stored but unused | Yes |
| iOS | **Page shows demo data and saves nothing** (event field-name mismatch); passkey index settable only in the sign-in endpoint sheet | Read-only |

## 4. Privacy — data flows vs the policy (summary)

Undisclosed or misdescribed today: the passkey index service (sees the
registration record and the requester's IP, keeps task rows 7–30 days, answers
lookups about counterparties); the relay logs sender + operation hash, keeps
operations 1 h–14 days, alerts with hashes to Telegram, forwards to node
providers; the `X-Rpc-Url` header sends the user's top RPC URL (possibly with
their provider key) to the relay; chain-data requests reveal displayed token and
signed contract addresses; selector lookups to third parties; AAGUID directory;
phone-sign-in tunnel servers; Cloudflare as host; Google Fonts on the site;
Android bug reports open *public* GitHub issues; the extension's permissions;
in-app browser cookies. The on-chain record holds the whole signed registration
call (including `clientDataJSON` with origin) and a label per key. "On-device
data is deleted when you log out" is false (sign-out keeps history, contacts
and settings). p256-index has no license. Company: MONDAY LABS LTD, no.
16988118, England and Wales (Companies House, verified 2026-09-22); privacy
contact per store submission docs: hello@mondaylabs.ltd. Full table: the
privacy report, reproduced in audit-report.md §Legal.

## 5. Shipped state (as of 2026-09-22)

| Surface | State |
|---|---|
| Web wallet (wallet.getvela.app) | Live. Create/sign in (1–7 keys), send incl. split / sweep / batch import, receive + payment requests, contacts, settings incl. endpoints and speed, clear-signing sheet. **No dApp connection** (by ruling) — use the extension |
| Chrome extension | MV3, injects EIP-1193 + EIP-6963; side-panel signing; Chromium only (Chrome, Edge, Brave named); **GitHub release zip, load unpacked; not on the Chrome Web Store** |
| Desktop (gpui) | Windows (x64/arm64, unsigned installer), Linux (.deb/.rpm/Flatpak, x64/arm64), macOS (.dmg notarized by hand — v0.9.3 only; v0.9.4 has none, so the site shows "coming shortly"). Built-in dApp browser on macOS/Windows, **none on Linux**. No store listings |
| iOS / Android | Native apps wired and device-tested (mostly with the fixed test keyset); in-app dApp browser; **not in the App Store or Google Play; no TestFlight/Play testing**. Price decided: $39.99 one-time (mobile only). Self-build possible with the limits in §1 |
| Signing page (`app-web/clearsigning`) | Built and tested locally; **not deployed or published** |
| Releases | v0.9.3, v0.9.4 (pre-releases). CalVer (066) not yet in effect |
| Roadmap drift | Address book shipped; WalletPair dropped (dApp connect = injection); 24 networks shipped; "path for chains without P-256" contradicts the chain-setup rule; an audit is not scheduled; "accounts follow via platform backup" is false for the native apps; "branded pull-to-refresh" and web "one-tap feedback" no longer exist |

## 6. External citations

Report received 2026-09-22. Corrections applied (audit-report.md F-4, F-5, H-10):
Safe4337Module v0.3.0 has three reviews — Ackee (2024, one acknowledged warning),
**Certora (Aug 2026, Medium M-01 acknowledged, not fixed)**, Nethermind (Aug 2026,
no findings); the passkey module v0.2.1 additionally had a Nethermind review (Aug
2026, no findings), and its factory and singleton were in scope of every v0.2.x
review; the Safe bounty's top tier is "High, up to $1,000,000" (no Critical tier);
the EntryPoint griefing disclosure (5 Feb 2026) — the $50k is from secondary
reports and "never exploited" is unsourced, impact includes prolonged
unavailability, and pending `handleOps` are visible in the public mempool; the
pathUSD "DefiLlama assessment" has unclear provenance — cite Tempo's SECURITY.md;
Bybit facts verified; MetaMask: limited gas sponsorship exists, the licence is
source-available (non-commercial), EIP-7702 smart accounts are default for new
users; Base App was renamed Coinbase Wallet on 10 Sep 2026 and account.base.app
redirects; blog: the Ledger price (was $59, not $44) and the Family/Dharma/Loopring
framing were wrong at their date.

## 6a. Adversarial review (2026-09-22)

An independent hostile reading of the rewritten English found two Fatal issues
(third-party script on the passkey domain; "every service can be replaced" while
the relay hard-codes Vela's chain directory), ten High (fee understated at
"several times"; approval guard threshold; self-calls; key revocation; rpId rules;
custody absolutes; relay powers; privacy; the signer factory missing from the
network check; unverified recipient names) and many Medium/Low. All were fixed in
en + zh before translation (commit 63bd439a); the product side of F1 was fixed
with a Permissions-Policy header on getvela.app and removing analytics from
/chain-setup. Details in audit-report.md.

## 7. Decisions

**D1 — Describe the fee by its formula, not by a multiple.**
Decision: the fees page states the formula in plain words (three times the
padded gas the wallet reserves, priced at the higher of the wallet's gas reading
and the relay's price for the chosen speed; minimum $0.01; the relay keeps the
difference), and that the exact amount is on the confirm screen and signed.
The landing trade-off says the fee is a multiple of the on-chain cost and links
there.
Rationale: every multiple in circulation ("≈2×", "3×") is wrong for the default
speed, and a measured multiple varies by chain and transaction.
Alternatives: quote "~2×" from the pricing essay (false); compute a typical
multiple (not measured here — would be a new unverifiable number).
Flag for the founder: the pricing narrative (`docs/marketing/why-we-charge.md`
"about twice the raw on-chain cost") and the code disagree; one of them has to
change.

**D2 — Remove the gas-account activation text everywhere; describe the treasury
bootstrap instead.** Rationale: the mechanism no longer exists (§2).

**D3 — 24 networks, listed once.** The networks page carries the full table;
the landing grid shows all 24; other pages say "24 built-in networks" and link.

**D4 — The signing page is "built, not published; load it from source".**
Every page that mentions it says so. Rationale: §5.

**D5 — The self-hosting guide tells the per-platform truth** (§3b), including
iOS, and the gaps go to a follow-up list for the product. Rationale: R8; a
guide that promised a Settings page that does not save would be caught by the
first reader who tries.

**D6 — Legal pages: correct facts, dates, company details and contact.** Add
the standard statutory carve-out and governing law for a UK company; flag both
pages for legal review in results.md. Rationale: FR-011/012; the current pages
bind every user through the onboarding checkbox.

**D7 — Blog posts get dated correction notes**, not silent rewrites (R5).

**D8 — Fix the `/api/transactions` key-leak now** (allowlist the network slug);
recommend deleting the five dormant routes. Rationale: security; minimal change.

**D9 — Product gaps found during the audit are listed, not fixed here**:
iOS endpoints page, Android name lookups, desktop index setting and fee-reserve
strings, desktop "Self-hosting guide →" dead label, remote descriptors marked
"verified" without authentication, no guard on Safe self-calls, `X-Rpc-Url`
leak, relay's hard-coded chain-data host, p256-index missing license and
Dockerfile, relay Dockerfile, desktop/iOS erase, iOS privacy manifest, web
feedback button, analytics on the same origin as the chain-setup key.
Rationale: out of scope for a content spec and owned by concurrent sessions;
each is recorded with file references in results.md.

**D10 — Google Fonts: disclose, don't re-engineer.** The privacy policy stops
saying data never reaches Google; self-hosting the fonts is recommended.
