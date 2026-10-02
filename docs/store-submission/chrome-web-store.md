# Chrome Web Store — first submission sheet (spec 094)

Paste-ready answers for the Chrome Web Store Developer Dashboard, for the extension's **first upload
(a test listing)**. Each field is a heading, then exactly what to paste (fenced block), then one
"Why / evidence" line. Every fact cites the code (`file:line`, paths from the repo root) or the claim
ledger (`specs/080-site-content-accuracy/claim-ledger.md`, ids `C-…`). Line numbers are as of the
`094-chrome-web-store` working tree on 2026-10-02. Path shorthand: `extension/…` and `src/…` are under
`app-web/vela-wallet/`; bare `background.js`, `content.js` and `lib/…` are in
`app-web/vela-wallet/extension/`; `inpage.js` is `rust/crates/vela-core/provider/inpage.js`.

Operator / data controller: **MONDAY LABS LTD** (UK). Facts and tone follow the mobile sheets
[`privacy-and-review.md`](privacy-and-review.md) and [`privacy-evidence.md`](privacy-evidence.md).

The owner's decisions (2026-10-02) are recorded where they apply, marked **Owner decision
(2026-10-02)**.

---

## 1. Pre-flight — what must be true before uploading

Build, from `app-web/vela-wallet`:

```bash
pnpm package:extension        # = node extension/build.mjs --zip
```

The upload is `app-web/vela-wallet/vela-wallet-extension-0.9.6-chrome-web-store.zip` (the store
package `extension/dist-store`, zipped by contents). The other zip, without `-chrome-web-store`, is the
GitHub release's "Load unpacked" package (`extension/dist-release`: it keeps `key` so a tester's id
stays the same) and must **not** be uploaded. Neither carries the parallel space.

| # | Check | How | Why / evidence |
|---|---|---|---|
| P1 | `manifest.json` at the zip root | `unzip -l <zip> \| grep -c ' manifest.json$'` → 1 | `build.mjs` zips each package's contents (`extension/build.mjs` §6, `zip -qrX … .` with `cwd: dir`) |
| P2 | No `key` in the manifest | `unzip -p <zip> manifest.json \| grep -c '"key"'` → 0 | The store assigns the id; `deriveStorePackage` deletes `key` (`extension/build.mjs` §6). Nothing depends on the id: the rpId is `getvela.app` by host permission (`src/lib/onboarding/core/passkey.ts:74-86`) |
| P3 | `version` 0.9.6, `minimum_chrome_version` 122, `incognito` `not_allowed` | `unzip -p <zip> manifest.json` | `app-web/vela-wallet/extension/manifest.json:4,7,8` |
| P4 | No developer pages | `unzip -l <zip> \| grep -cE '/(parallel\|gallery)[./]'` → 0 | `STORE_PRUNE = ['parallel']` (the page, its `parallel.boot<n>.js` and its data directory), `PRUNE = ['gallery', 'gallery.html']` (`extension/build.mjs`); asserted by `src/lib/extension/package.test.ts` |
| P5 | No top-level name starting with `_` | `unzip -l <zip> \| awk '{print $4}' \| grep -c '^_'` → 0 | Chrome refuses the whole package (`extension/README.md:51-57`) |
| P6 | Load the unzipped store package once with "Load unpacked" and create/sign | real Chrome ≥ 122 | The id differs from the development one; the passkey must still be a `getvela.app` one (P2) |
| P7 | The privacy policy with the S10 fixes is **deployed** | `curl -s https://getvela.app/privacy \| grep -c 'Limited Use requirements'` → 1 | Section 8. Deploying getvela.app is the owner's |
| P8 | Publisher account: 2-Step Verification on, registration done | Dashboard | Required to publish ([2-Step Verification](https://developer.chrome.com/docs/webstore/program-policies/two-step-verification), [Register](https://developer.chrome.com/docs/webstore/register)) |
| P9 | Relay, passkey index and chain data are up while the review runs | status checks | The reviewer creates a wallet: the index must accept the registration (`src/lib/onboarding/core/registry.ts:156`) |

| P10 | Steps 3-5 of section 10 done once by hand on the store package, in a real Chrome with a real passkey | — | The automated runs sign with the parallel space's fixture key and load the development package for that; the store package's own e2e (`e2e/extension-store-package.e2e.ts`) proves the install, the welcome, the wasm, the provider and the EIP-5792 answers, not a real authenticator |

The provider no longer logs anything in a page's console (spec 094; 089 F20 —
`rust/crates/vela-core/provider/inpage.js`, pinned by `dapp_rpc` test `the_provider_logs_nothing_into_a_page`).

---

## 2. Store listing → Item details

### Name

```
Vela Wallet
```

Why / evidence: taken from the manifest (`extension/manifest.json:3`).

### Summary (short description)

```
The passkey wallet, in your browser. One address across 24 networks, no seed phrase.
```

Why / evidence: the dashboard takes this from the manifest `description`
(`extension/manifest.json:5`); 84 characters (limit 132). Claims checked:
- "passkey wallet", "no seed phrase" — C-keys-2, C-sign-1; the wallet has no seed phrase anywhere.
- "One address" — C-addr-1: the address comes from all the wallet's keys and is the same on every network.
- "24 networks" — C-net-1; 24 built-in chains in `app-web/vela-wallet/src/lib/services/chains.ts:45-371`
  and `rust/crates/vela-core/src/app/network_admin.rs:270` (`[NetBuiltinChain; 24]`).
- Caveat that does not make it false: on networks that lack Safe's passkey signer factory, a wallet made
  from two to seven keys cannot be deployed yet (`app-web/getvela.app/src/content/docs/networks-and-fees.md:53-57`);
  the address is still the same there.

### Description (detailed)

```
Vela is a self-custody wallet for Ethereum and other EVM networks that you sign with passkeys instead of a seed phrase. This extension is the same Vela wallet as the web wallet at wallet.getvela.app and the desktop and phone apps: the same keys open the same address everywhere. It is open source and every service it uses can be run by you.

HOW IT CONNECTS TO DAPPS
To let dApps find it, Vela adds a standard wallet provider (EIP-1193 and EIP-6963) to every website you visit, in the page's main frame. A dApp can be on any website, which is why it runs on all of them. It does not read the page's content: it only receives the requests a site sends to the wallet. A site sees your address only after you approve the connection. Connection and signing requests open in Chrome's side panel next to the page (or in a small Vela window), and nothing is signed until you approve it with one of your keys.

YOUR KEYS
When you create a wallet you choose up to seven keys, and any one of them can sign on its own:
• a passkey on this device, synced by Google Password Manager, iCloud Keychain or your password manager if you allow it;
• another phone, reached by scanning a QR code;
• a hardware security key.
Every signature needs that key's own check: fingerprint, face, device PIN, or a touch and PIN on a security key. Signing is done on your device, and your passkey's private key never goes to Vela. Keys are chosen when you create the wallet; they can't be added, removed or replaced later.

WHAT YOU SEE IS WHAT YOU SIGN
Vela decodes each transaction and message before you sign: built-in ERC-7730 descriptors for common contracts, descriptors from Vela's chain-data service, token standards, then a public function-selector database (marked best effort). Anything it can't decode carries a clear blind-signing warning. Unlimited token approvals are shown in red, and calls that would hand over your account are refused.

FEES ARE SHOWN BEFORE YOU SIGN
Each transaction carries a relay fee, paid from your wallet in the network's coin or a supported USD stablecoin. The exact amount is shown before you sign and is part of what you sign. It goes to the relay the wallet is set to: Vela's by default, or another vela-relay you choose or run yourself. There is no deposit or gas account to fund first.

ONE ADDRESS, 24 NETWORKS
Your address comes from all the keys you create the wallet with, and it is the same on every network: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume and XRPL EVM, all mainnets. You can add other EVM networks that meet its requirements, and your own RPC nodes.

A SAFE YOU CONTROL
Your account is an unmodified Safe v1.4.1 on ERC-4337 (EntryPoint v0.7), with Safe's 4337 and passkey modules. No contract in the funds path was written by Vela. Vela holds no key and no role on your Safe, so it can't move or freeze your funds by itself.

RUN IT YOURSELF
Everything is MIT-licensed: the wallet, the relay, the public-key index, the exchange-rate service and the chain-data directory. Each service can be run by someone else, and the wallet can be pointed at your copies. Source: https://github.com/mondaylabsltd/vela-wallet

GOOD TO KNOW
• Creating a wallet writes each key's public key, the wallet's name, the key labels and the address to a public registry on Gnosis Chain. That record is permanent and readable by anyone, so choose a name you're happy to make public.
• Your list of accounts stays in this browser. On another device, sign in with any of your keys and the wallet is rebuilt from the public registry.
• The interface is available in 15 languages.
• Needs Chrome 122 or later. It does not run in Incognito windows.

Privacy policy: https://getvela.app/privacy
```

Why / evidence, paragraph by paragraph:
- Provider on every site, top frame, no page content: `extension/manifest.json:35-48` (`*://*/*`,
  `all_frames: false`, `world: MAIN`); `content.js:42` reads only `window.location.origin`; the provider
  reads only the origin (`rust/crates/vela-core/provider/inpage.js:209`). Address only after approval:
  `extension/background.js:879-911` (`grantedAccounts`). Side panel or window: `background.js:1221-1236`,
  `:515-523`. Same wallet: `src/lib/onboarding/core/passkey.ts:74-86`; C-rp-1, C-dapp-1.
- Keys: C-keys-1, C-keys-2, C-auth-1, C-sign-1. Clear signing: C-clear-1, C-clear-2, C-approve-1, C-selfcall-1.
- Fees: C-fee-1 (shown before you sign, part of what you sign, relay replaceable), C-fee-2 (no deposit
  or gas account). Never a multiple of the on-chain cost, never "free" (founder rule 2026-09-22).
- Networks: C-net-1, C-addr-1. Safe: C-acct-1, C-custody-1. Licences/self-hosting: C-lic-1, C-selfhost-2.
- Registry: C-reg-1. Account list: C-sync-1. 15 languages: `extension/build.mjs` header ("all 15 locales").
- Deliberately absent, as in the mobile listing (`store-listing-copy.md` red lines): no "free", no
  "audited" or audit plans (C-audit-1: no third-party audit of Vela's own code, none scheduled), no
  "alpha/beta", no WalletConnect (C-dapp-1). Keyword stuffing (Yellow Argon) is "unnatural
  repetition of the same keyword more than 5 times": the text is prose (3,855 characters; "wallet"
  and "key" occur 17 times each in sentences), and the one list, the networks, names each network once.

### Category — **Owner decision (2026-10-02)**

```
Productivity › Workflow & Planning
```

Why / evidence: the current store groups categories under Productivity, Lifestyle and Make Chrome
Yours ([announcement](https://groups.google.com/a/chromium.org/g/chromium-extensions/c/YS-HD7Ta3EQ)).
MetaMask and Rabby Wallet are both listed under **Workflow & Planning**
([MetaMask](https://chromewebstore.google.com/detail/metamask/nkbihfbeogaeaoehlefnkodbefgpgknn),
[Rabby](https://chromewebstore.google.com/detail/rabby-wallet/acmacodkjbdgmoleebolmdjonilkdbch)),
so people browsing for a wallet find Vela beside them. Alternative: **Productivity › Tools**
([exists](https://chromewebstore.google.com/category/extensions/productivity/tools)). "Privacy &
Security" (Make Chrome Yours) would invite a reading of the extension as a security product, which it
does not claim to be.

### Language

```
English
```

Why / evidence: the listing text above is English; the manifest has no `default_locale`
(`extension/manifest.json`), so the extension's own name and summary are English. The listing can be
localized later in the dashboard (089 research R4).

### Homepage URL / Support URL

```
https://getvela.app
https://getvela.app/support
```

Why / evidence: `/support` exists for the store listings (`app-web/getvela.app/src/lib/i18n/urls.ts:94-96`).

### Mature content

```
No
```

---

## 3. Privacy practices → Single purpose

```
Vela Wallet is a self-custody crypto wallet. It holds the person's Ethereum accounts, signs with their passkeys, and lets the websites they use (dApps) ask, through the standard wallet provider (EIP-1193 / EIP-6963), to connect, sign a message or send a transaction, each of which the person approves or rejects in Chrome's side panel. Everything in the extension serves that purpose: the wallet pages, the provider that dApps talk to, and the service worker that carries requests between them.
```

Why / evidence: `extension/README.md:1-18, 78-98`; `extension/background.js:1-46`.

---

## 4. Privacy practices → Permission justification

Context for all of them: `"incognito": "not_allowed"` (`extension/manifest.json:8`) — the extension
cannot be enabled in Incognito. `"minimum_chrome_version": "122"` (`manifest.json:7`) — from Chrome 122
an extension page may use a WebAuthn relying party ID only for a domain it holds host permission for
([Chromium announcement, Dec 2023](https://lists.w3.org/Archives/Public/public-webauthn/2023Dec/0078.html)),
which is what the `getvela.app` entry below relies on. No `default_popup`, no `scripting`,
`webRequest`, `cookies`, `history` or `externally_connectable` (`manifest.json:15-49`).

### `storage`

```
Vela keeps its own state in chrome.storage: the sites the person connected and the address each may see, the network each site switched to, the network list (RPC node and relay addresses) that the wallet publishes for the service worker, and a snapshot of the signed-in address so an already-connected site gets eth_accounts and eth_chainId without a window opening. chrome.storage.session holds the requests still waiting for the person's answer, so a restarted service worker still knows what it owes, plus a short diagnostic log that is cleared when the browser closes. storage.local is closed to content scripts (setAccessLevel TRUSTED_CONTEXTS). It also keeps, for 24 hours, the operation hashes it gave to sites, so their receipt lookups can be answered. Nothing in chrome.storage is sent to us.
```

Why / evidence: grants `vela.perm.<origin>` = `{origin, address, chainId, grantedAt}`
(`src/lib/dapp/keys.ts:12`, `src/lib/dapp/connections.ts:57-66`); chain picks `vela.chain.<origin>`
(`background.js:948`); catalog `vela.ext.chains` (`src/lib/dapp/core/ext-chains.ts:20-31, 85`); snapshot
`vela.ext.cache` (`src/lib/dapp/core/ext-cache.ts:125`, read at `background.js:890-911`); ledger
`vela.req.*` in `storage.session` (`background.js:107-109, 273-279`); log `vela.sw.log` / `vela.sw.counts`
in `storage.session` (`extension/lib/swlog.js:11-25`); operation hashes `vela.ext.op.*` for 24 h
(`extension/lib/op-receipt.js:22-24`, written at `background.js:1046-1054`); access level `background.js:121-127`. 089 R3.

### `tabs`

```
The toolbar button opens the wallet in a browser tab and reuses the wallet tab when one is already open. Finding that tab means querying tabs by the extension's own URL (chrome-extension://<id>/*), which Chrome only answers with the tabs permission; without it the query returns nothing and every click would open another wallet tab. On a fresh install the extension opens its welcome in a tab and counts the web pages already open, only to say whether they need a reload before their sites can see the wallet. Tab URLs are compared in memory and never stored or sent.
```

Why / evidence: `background.js:220-236` (`tabs.query({url: chrome.runtime.getURL('') + '*', windowType: 'normal'})`);
measured in 089 (research R2 D9, R3: "own extension URLs are invisible without it"). The install warning
this adds is already covered by the all-sites host permission (089 D9).

### `sidePanel`

```
Connection and signing requests open in Chrome's side panel next to the page that asked. A toolbar popup cannot be used: it closes as soon as the passkey prompt takes focus, in the middle of a signature. The panel is opened with chrome.sidePanel.open inside the page's click; a request a page sends without a click opens in a small Vela window instead.
```

Why / evidence: `extension/README.md:37-50` (constraint 4); `background.js:457-471` (`openPanelNow`),
`:1221-1236` (panel first, synchronously), `:511-523` (420 × 760 popup window fallback);
`manifest.json:27-29`.

### Host permission `https://getvela.app/*`

```
Vela's passkeys belong to the relying party getvela.app, the same as the web wallet at wallet.getvela.app and the desktop and phone apps, so the same keys open the same wallet address. Since Chrome 122, an extension page may use a WebAuthn relying party ID only for a domain it has host permission for; this entry is what lets the extension's own pages create and use getvela.app passkeys. Without it the extension could not use the person's existing keys. It is also used when the person chooses to send a bug report from Settings (a POST to https://getvela.app/api/bug-report).
```

Why / evidence: `src/lib/onboarding/core/passkey.ts:74-86` (`isPackagedApp()` → `getvela.app`;
"Chrome permits an extension to claim a relying party it holds host permission for"); README
constraint 3 (`extension/README.md:33-36`); bug report `src/lib/services/bug-report.ts:24-33, 60`.

### Host permission `*://*/*` (and the content scripts on every site)

```
Three uses, all for the wallet:
1. Content scripts on every http and https page, main frame only, at document_start. inpage.js (page world) adds the standard wallet provider (EIP-1193 and EIP-6963) so a dApp on any domain can find the wallet before its own scripts run; content.js (isolated world) passes that provider's requests to the service worker. Neither reads the page's DOM, text, forms or cookies; content.js reads only the page's origin, to tag its messages.
2. The service worker sends a dApp's requests that need no signature (an allowlist: eth_call, eth_getBalance, eth_blockNumber and similar node reads, plus ERC-4337 bundler reads) to the RPC node and relay the person's wallet is set to for that site's network. Those endpoints can be any host (built-in public nodes, a provider the person added, or their own node) and many send no CORS headers, so the worker needs host access to reach them.
3. When a site's connection or network changes, the worker tells that site's open tabs (accountsChanged, chainChanged, disconnect), matching each tab's URL to the site's origin in memory.
A narrower list is not possible: dApps live on any domain and the RPC endpoints are the person's choice. The extension has no webRequest, cookies or history permission and keeps no list of visited pages.
```

Why / evidence: (1) `manifest.json:35-48`; `content.js:42`; provider `inpage.js:428-469` (sets
`window.ethereum` only if absent, announces EIP-6963). (2) `background.js:1199-1201` → `forwardRead`
`:1070-1086` → `readChain` `:993-1043` (`fetch` at `:1006-1011`); allowlist `extension/lib/protocol.js:323-375`,
routed by `classifyMethod` `:395-414` (anything else is refused). (3) `background.js:1093-1106`. 089 R3.

---

## 5. Privacy practices → Remote code

```
No, I am not using remote code.
```

Justification (if the form asks):

```
Every script is in the package: the service worker, the two content scripts and the wallet pages, which are built and bundled before upload (inline scripts are moved into packaged files). The wallet's core logic is a WebAssembly module that is also in the package. The extension pages' content security policy is "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'": no remote script source and no 'unsafe-eval'. Network requests fetch data only (JSON-RPC answers from nodes and the relay, network and token metadata, ERC-7730 descriptor JSON, exchange rates); none of it is run as code.
```

Why / evidence: `manifest.json:32-34`; `extension/build.mjs` (externalises inline scripts, bundles
`inpage/content/background/panel/open` with esbuild, unminified); README constraints 1-2
(`extension/README.md:25-32`); the core wasm is copied into the package at build (`build.mjs`, spec 094
B2 step `scripts/sync-wasm.mjs`); the optional Tevm engine is disabled and not a dependency
(`src/lib/services/sim/sim-engine-tevm.ts:5-24`).

---

## 6. Privacy practices → Data usage

Chrome requires disclosure "even when data is processed or stored locally on a user's device and is not
transmitted" ([User Data FAQ](https://developer.chrome.com/docs/webstore/program-policies/user-data-faq)).
The extension is the web wallet's build (`extension/README.md:3-6`), so the wallet's own traffic counts.

Where things live: `chrome.storage.local` / `.session` (section 4, `storage`); the wallet pages'
IndexedDB `vela`/`kv` and `localStorage` (`src/lib/services/storage.ts:1-27`). Default services
(`src/lib/services/endpoints.ts:14-23`): passkey index `p256-index-v2.getvela.app`, relay
`vela-relay-cf.getvela.app`, chain data `ethereum-data.getvela.app`, rates `vela-currency.getvela.app`,
authenticator names `aaguid-explorer.awesometools.dev`; plus RPC nodes and public selector databases
(`src/lib/services/selector-registry.ts:52, 69`). Server-side retention: `privacy-evidence.md` §2.

| Data type | Tick | Reason (what, where, to whom) |
|---|---|---|
| Personally identifiable information | **Yes** | The wallet address (an account identifier) and the wallet name and key labels the person types. Stored in the browser; sent to Vela's passkey index when a wallet is created (kept 7 days after it lands, 30 if it fails) and published permanently on a public registry on Gnosis Chain (`src/lib/onboarding/core/registry.ts:156`; C-reg-1; privacy-evidence §2.1). Contacts (names and addresses the person saves) stay in the browser. No name, email, phone or ID is asked for. |
| Health information | No | None handled. |
| Financial and payment information | **Yes** | Balances, transaction history and each transaction: the operation (sender, calldata with recipients and amounts, signature) goes to the relay, which logs the address and keeps operations 1 hour to 14 days (privacy-evidence §2.2), and reads go to RPC nodes. History is kept in the browser (`vela.transactionHistory`). No card numbers, no fiat payments. |
| Authentication information | **Yes** | Passkey public keys and credential IDs (sent to the index and published on-chain; public by design, they cannot sign); the WebAuthn signature in each operation sent to the relay; RPC provider API keys the person adds, kept in the browser and sent only to that provider as part of its URL. The extension never has the passkey's private key (it stays with the authenticator, C-sign-1); no passwords; no seed phrase. |
| Personal communications | No | No messaging. (Bug report: see judgement J3.) |
| Location | No | No geolocation API, no location value is read or sent. (IP addresses: see judgement J1.) |
| Web history | **Yes** | Kept in the browser only, never sent: the sites connected (origin, address, network, time granted — `src/lib/dapp/connections.ts:57-66`); the network each site switched to, including sites that asked before connecting (`background.js:923-954`); in Activity, each approved request's site origin and dApp name with its time and status (`src/lib/services/transactions-model.ts:34-46`); while the browser is open, the hosts of sites that asked to connect or sign, in the worker's log (`background.js:575-579`, `lib/swlog.js:11-19`). No list of visited pages is kept. A user-facing feature (Settings → Connections, Activity), which Limited Use allows. |
| User activity | No | No click, scroll, mouse, keystroke or network monitoring; no `webRequest` permission (`manifest.json:30`). The extension only receives requests a site sends to it. |
| Website content | **Yes** | Conservative: the provider receives what a site sends to the wallet — the transaction, message or typed data to sign, and node reads. Approved requests are kept in Activity (message up to 8,000 characters, `transactions-model.ts:65`; the request up to 24,000, `src/lib/services/dapp-history.ts:26`); reads are forwarded to the person's RPC node or relay (`background.js:1070-1086`). The signing sheet loads the site's own icon from the site (`src/lib/signing/live.ts:126-131`). The page's DOM, text, images and forms are never read (`content.js:42`). |

**Owner decision (2026-10-02) — judgement calls:**
- **J1. IP addresses as "Location": not ticked**, matching the Play answer (§6.5 #7). Every server the
  wallet talks to sees the IP (Cloudflare for Vela's services; RPC nodes); Vela's code stores none —
  the index keeps a salted 64-bit hash for at most 60 s for rate limits, the bug-report endpoint keeps
  it in memory for its rate limiter, the relay reads none (privacy-evidence §2.1, §2.2, §3a, §6.5 #7);
  the privacy policy names Cloudflare and RPC nodes as seeing it.
- **J2. Website content: Yes** (kept). A narrower reading (only page DOM counts) would allow No; Yes
  avoids a Purple Nickel / Purple Lithium mismatch with the privacy policy, which describes these
  requests.
- **J3. Bug report.** Sent only when the person writes one and taps Send: typed text, device lines
  (version, browser and OS name, language, names of unreachable networks, failure counters including
  the worker's — never site names), up to five re-encoded screenshots; it becomes a **public** GitHub
  issue via `getvela.app/api/bug-report`, screenshots kept in R2 until deleted
  (`src/lib/services/bug-report.ts:111-140, 167-206, 489-499`; privacy-evidence §3a). The form has
  no "user-generated content" type; it is covered by the privacy policy's Bug reports section.
  **No extra tick.**
- **J4. Authentication information and the on-chain registry.** Chrome's policy says authentication
  information must not be publicly disclosed. What is published is public keys and credential IDs,
  which cannot authenticate anyone; be ready to say so if asked. (Kept as a note.)

Evidence that the 089 draft (research R4 "Data use") was wrong: bug reports send typed text,
device lines and screenshots, not "counters only"; Web history is handled locally, so it is "Yes".

---

## 7. Privacy practices → Certifications (tick all three)

| Certification | True? | Why / evidence |
|---|---|---|
| I do not sell or transfer user data to third parties, outside of the approved use cases | Yes | Data goes only where the wallet needs it to work: the relay that submits the transaction, the RPC nodes that answer reads, the index that registers keys, and a public GitHub issue the person chooses to file. Nothing is sold; the policy says so (`app-web/getvela.app/src/routes/privacy/+page.svelte`, "We do not sell or share this data, build profiles from it, or use it for advertising"). |
| I do not use or transfer user data for purposes that are unrelated to my item's single purpose | Yes | No analytics, ads or tracking SDK in the wallet (privacy-evidence §4: zero hits across `app-web/vela-wallet/src`); in-app counters stay in memory (`src/lib/services/metrics.ts:10-23`). |
| I do not use or transfer user data to determine creditworthiness or for lending purposes | Yes | No such feature or data flow exists. |

---

## 8. Privacy practices → Privacy policy URL

```
https://getvela.app/privacy
```

Must be **deployed with the S10 fixes first** (pre-flight P7). The fixes on this branch
(`app-web/getvela.app/src/routes/privacy/+page.svelte`, section "The browser extension"): requests that
need no signature are forwarded for **any** site, connected or not (`background.js:1199-1201`, no grant
check); what is kept locally, including chain picks for unconnected sites and the session log; the
provider reads no page content; feedback; no Incognito; how to delete; and the Chrome Web Store
Limited Use statement in its official wording
([Limited Use](https://developer.chrome.com/docs/webstore/program-policies/limited-use)):
"The use of information received from Google APIs will adhere to the Chrome Web Store User Data
Policy, including the Limited Use requirements."

---

## 9. Store listing → Graphic assets

Requirements: icon 128 × 128 PNG with 96 × 96 art and 16 px transparent padding; 1-5 screenshots at
1280 × 800 or 640 × 400, square corners, full bleed; small promo tile 440 × 280 (required); marquee
1400 × 560 optional ([Images](https://developer.chrome.com/docs/webstore/images)).

| File (`docs/store-submission/chrome-web-store/`) | Size | Caption (one line) |
|---|---|---|
| `store-icon-128.png` | 128 × 128, transparent (96 × 96 art + 16 px padding) | — |
| `screenshot-1-wallet.png` | 1280 × 800 | Your wallet in a tab: one address across 24 networks, signed with a passkey. |
| `screenshot-2-connect.png` | 1280 × 800 | A dApp asks to connect; you decide in Chrome's side panel, next to the page. |
| `screenshot-3-signing.png` | 1280 × 800 | Every request decoded before you sign: what it allows, to whom, and until when. |
| `promo-small-440x280.png` | 440 × 280 | — |

How they were made (spec 094 B4), all from the real build and reproducible with
`app-web/vela-wallet/scripts/store-art/` (`art.mjs`, `wallet.mjs`, `panel.mjs`; run from
`app-web/vela-wallet` after `pnpm build:extension`):
- Screenshots: the extension's own pages in Playwright's Chrome for Testing with a throwaway profile.
  The wallet is the parallel space's fixture multi-key Safe (renamed "Everyday") with its real
  balances on the real networks, read-only; the development package is used because the parallel
  space is how a script signs in, and the parallel-space badge (a developer marker the store package
  does not have) is hidden. Screenshots 2 and 3 are the real side panel, captured through Chrome's
  CDP endpoint, beside a neutral demo page (`scripts/store-art/demo-dapp.html`, "Example Swap",
  served as `https://swap.example`, a reserved documentation domain); screenshot 3 is a Permit2
  permit for 250 USDC, clear-signed by the built-in descriptor, with the "can't be capped here" line
  every shell draws for a permit.
- Icon and promo tile: the canonical mark (`docs/design/icon/app-icon.svg`, `app-mark.svg`) and the
  app's font, rendered by Chromium.

Check each file's pixel size before upload (`sips -g pixelWidth -g pixelHeight <file>`).


---

## 10. Test instructions tab (paste-ready)

Paste into the dashboard's **Test instructions** tab
([docs](https://developer.chrome.com/docs/webstore/cws-dashboard-test-instructions)). No account or
credentials are needed.

```
Vela Wallet is a crypto wallet that signs with passkeys. No account, password or funds are needed to test connecting and signing a message.

1. Install the extension. Its welcome opens in a tab. Pin it: Extensions menu (puzzle icon), then the pin next to Vela Wallet. Tabs that were open before the install need a reload before a website can see the wallet (the welcome says so when there are any).
2. The Vela toolbar button opens the wallet in a tab at any time.
3. Create a wallet: choose Create Wallet, choose "This device" as the key, give the wallet any name when asked, and approve Chrome's passkey prompt. You can save the passkey in your Chrome profile (Google Password Manager), iCloud Keychain on a Mac, or Windows Hello; Chrome's prompt also offers a phone (QR code) or a USB security key. Chrome asks for the passkey more than once: Vela confirms each key with a signature. If the key is not synced (for example Windows Hello or a security key), Vela asks for a second key before it creates the wallet.
   Note: creating a wallet writes the wallet's name, key labels, public keys and address to a public registry on Gnosis Chain. Use a test name.
4. Open https://metamask.github.io/test-dapp/ (a public test page). In the "EIP 6963" card, click "Vela Wallet", then click "Connect". Vela opens in Chrome's side panel next to the page; approve the connection. The page shows your account.
5. In the "Personal Sign" card click "Sign". Review the message in the side panel and approve it with your passkey. The signature appears on the page. "Sign Typed Data V4" works the same way. These are signatures only: nothing is sent on-chain and no funds are needed. (The returned signature is a smart-account signature, EIP-1271, so the page's ecrecover-based "Verify" buttons will not match it. That is expected.)
6. Do not send transactions: every network in the wallet is a mainnet.

Notes:
- A request opens in the side panel when the page sent it from a click. If a page sends it without a click, Chrome does not allow the side panel to open, so Vela opens a small window instead.
- On the legacy window.ethereum object the provider sets isMetaMask: true, because many older dApps only offer a wallet when that flag is set. Through EIP-6963 it announces its real identity (name "Vela Wallet", rdns "app.getvela"), and it never replaces a window.ethereum another wallet already set.
- Keep the extension's Site access on "On all sites" for the test. If you restrict it, dApps on those sites cannot see the wallet and Chrome does not let it use its getvela.app passkeys; the wallet then says so at the top of its pages and offers to allow access again in one click.
- The extension cannot be enabled in Incognito windows.
- Source code (MIT): https://github.com/mondaylabsltd/vela-wallet — the extension is in app-web/vela-wallet/extension, the page provider in rust/crates/vela-core/provider/inpage.js.
```

Why / evidence:
- Reload of open tabs: content scripts are declared in the manifest only (`manifest.json:35-48`); there
  is no `scripting` permission to inject into tabs that were already open.
- Toolbar → tab: `background.js:210-240`. Labels "Create Wallet", "This device":
  `rust/crates/vela-core/i18n/locales/en/onboarding.json` (`create.createWalletBtn`, `create.methodPlatformTitle`).
- More than one prompt (member proof): `rust/crates/vela-core/src/app/create_wallet.rs:886-900`; second key
  when the only key is not synced: `create_wallet.rs:878-884`. Registry record: C-reg-1.
- Test dApp: `https://metamask.github.io/test-dapp/` has an "EIP 6963" provider card, a "Connect" button
  (`id="connectButton"`), "Personal Sign" and "Sign Typed Data V4" cards with "Sign" buttons (read from
  its `index.html` and `main.js` on 2026-10-02). EIP-1271 signature: `rust/crates/vela-core/src/app/sign_request.rs:617-618`.
  All 24 built-in networks are mainnets: C-net-1.
- Side panel or window: `background.js:457-471, 1221-1236`; README constraint 4.
- `isMetaMask` shim and true EIP-6963 identity: `inpage.js:387-398, 428-449, 458-469`; `lib/protocol.js:31-32`.
- getvela.app access: [Chromium announcement](https://lists.w3.org/Archives/Public/public-webauthn/2023Dec/0078.html);
  `passkey.ts:74-86`.
- Repository public: GitHub API `"visibility": "public"` (checked 2026-10-02); C-lic-1.

- Site access: `src/lib/extension/site-access.ts` (`chrome.permissions.contains` / `.request`),
  `src/lib/extension/ExtensionNotices.svelte`; a refused ceremony reads `onboarding.common.siteAccessBody`
  (`src/lib/onboarding/core/copy.ts`). Install tab: `background.js` `runtime.onInstalled`.
- Step 5 on a brand-new wallet: a message signature needs no deployment (the Safe's EIP-1271 signature
  is made by the passkey either way; only an on-chain verification waits for the first transaction).
  Pre-flight P10 runs it once by hand.

---

## 11. Account → Trader / non-trader declaration (EU Digital Services Act)

```
Non-trader
```

**Owner decision (2026-10-02): Non-trader.** For reference, the
[Trader FAQ](https://developer.chrome.com/docs/webstore/program-policies/trader-verification-faq)
defines a trader as "any natural person or any legal person, who is acting for purposes relating to
his trade, business, craft or profession"; the declaration can be changed in the dashboard later.

---

## 12. Review-time expectations and rejections

What is documented ([Review process](https://developer.chrome.com/docs/webstore/review-process)):
most items finish review within a few days, but it can take a few weeks; contact developer support if a
submission has waited more than three weeks without significant changes. Reviews take longer for new
developers and new items, broad host permissions such as `*://*/*`, sensitive permissions such as `tabs`,
and large or hard-to-review code. This item has all of these.

**Estimate (not an official number):** several days to about three weeks for this first submission.
After approval the item can be staged and published by hand within 30 days, then it reverts to a draft
([Publish](https://developer.chrome.com/docs/webstore/publish)).

**Owner decision (2026-10-02) — visibility for the test listing: Unlisted** — it installs from its
URL without appearing in search ([Distribution](https://developer.chrome.com/docs/webstore/cws-dashboard-distribution)).
Going public later also means updating the site's install page and claim ledger C-plat-1 ("not yet on
the Chrome Web Store").

If rejected: the publisher email receives the violation and a reference ID; fix and resubmit, or appeal
from the dashboard or the One Stop Support form (reply typically within three days). The likely
reasons for a wallet like this one, with their IDs
([Troubleshooting](https://developer.chrome.com/docs/webstore/troubleshooting)):

| ID | Reason | Where this sheet answers it |
|---|---|---|
| Purple Potassium | Permissions not needed or not justified | Section 4, one paragraph per permission, with the narrower alternative ruled out |
| Purple Lithium / Purple Nickel | Privacy policy or disclosure missing or inconsistent | Sections 6 and 8; the policy and the form must say the same thing |
| Purple Magnesium | Web browsing activity collected without a user-facing need | Section 6, Web history: local only, shown in Connections and Activity |
| Yellow Argon | Keyword stuffing | Section 2: prose, no keyword lists; each network named once |
| Red Nickel / Red Potassium | Misleading claims, impersonation | No "free", no audit claims; the `isMetaMask` shim explained up front (section 10) |
| Blue Argon / Red Titanium | Remote or obfuscated code | Section 5: packaged, unminified page-side scripts |
| Yellow Magnesium | Reviewer could not make it work | Section 10, and pre-flight P6 and P9 |
