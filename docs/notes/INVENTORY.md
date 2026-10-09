# getvela.app/notes — story inventory (draft)

The backlog for a new section of getvela.app, modelled on bun.sh/guides: hundreds of short,
separate pages, each telling **one** true story about why Vela is the way it is: a decision, a
principle, an incident that forced a design, or what a small piece of UI is for.

- **Harvested 2026-10-09 at `8e70878dd`** by reading the core, the four shells, specs 001–100,
  the relay and p256-index repos, issues, and the published site; a second pass added the
  settings, cost, speed and security notes (Part 0). ~400 candidates after merging duplicates;
  the target is the best 220 or more.
- **Every row is a lead, not a fact.** Re-read the cited source when you write the page. The
  harvest is a day old the moment it is written.
- **[`specs/080-site-content-accuracy/claim-ledger.md`](../../specs/080-site-content-accuracy/claim-ledger.md) wins** over this file, over
  `docs/CONTENT-SOURCE-100-CLUES.md` (stale in many places), and over `docs/requirements/*`.

Paths: `core/` = `rust/crates/vela-core/src/`, `W/` = `app-web/vela-wallet/src/lib/`,
`S/` = `specs/`, `TS/` = `app-web/trusted-signer/`, `relay:` = the vela-relay repo,
`p256:` = the p256-index repo.

**Status:** ✅ ready to write · 🗣 needs the founder (a reason only you know, or a call on whether
to publish) · ⏸ hold (code about to change) · 📜 a "why we changed it" story · 🔒 maybe internal

---

## Part 0 — Using Vela well

Harvested 2026-10-09, second pass (settings catalog, cost and speed, security ladder). These are
"use" notes: what a setting does and when to change it, what changes the fee, what each layer of
security adds. Same rules as every other note: one point, a source, the cost stated.

> ⏸ Most numbers in **paying-less** and **faster** move when `fix/eth-mainnet-fee` merges: the
> default speed becomes Standard (owner, 2026-10-09), the wallet stops adding its own ×1.5 and
> the 300k verification floor, and Ethereum/Gnosis/Polygon/BNB get a new formula. Tips that rest
> only on the $0.01 minimum (Gnosis, Stable, Kaia, XRPL EVM, Tempo) are not affected.

### settings — Settings (36)

| # | Title | What it does, default, when to change | Sources | Status |
|---|---|---|---|---|
| 1 | Theme: Light, Dark or System | Default System; web applies it before the first paint so nothing flashes | core/prefs.rs 46, 69, 94; W/services/preferences.svelte.ts | ✅ |
| 2 | Text size: one slider, six steps | 0.82–1.35×, default 1.0×, same steps on every app since spec 072; multiplies the phone's own setting | core/prefs.rs 58–70, 115–130 | ✅ |
| 3 | Language: follow the system, or pin one of fifteen | On web the language is in the URL; "Suggest a fix on GitHub" | core/prefs.rs 22, 72, 104–110 | ✅ |
| 4 | Display currency: what it changes and what it doesn't | Every fiat figure, never what you send or sign; a currency with no rate is never shown at 1 | core/app/display_currency.rs 1–21 | ✅ |
| 5 | Why your currency isn't in the list | Web/desktop list what the rate sources price; iOS/Android a fixed eight | W/settings/core/currency-catalog.ts; Android CurrencyCatalog.kt 31–39 | 🗣 phones gap |
| 6 | Number format: why the OS never formats your money | Presets incl. Indian grouping; Automatic reads the platform once, then it's a fixed preset | core/l10n/mod.rs 5–14; core/prefs.rs 47 | ✅ |
| 7 | Date and time format | ISO avoids ambiguity in screenshots you share | core/prefs.rs 48–56 | ✅ |
| 8 | Hiding your balance | Tap the balance; an "Unlimited" approval is never hidden because it's a warning; caches don't un-hide it | core/app/balance_dashboard.rs 802–806, 1053–1070 | ✅ |
| 9 | Your default transaction speed | One default per device, kept on sign-out; an unreadable value counts as "never chose" | core/app/fee_tier_pref.rs | ⏸ default changes |
| 10 | Changing the speed for one send | One-shot; the next send starts at your default again | fee_tier_pref.rs 29–33; S/068 | ⏸ |
| 11 | Built-in networks vs ones you add | Built-ins can't be removed; a custom one can, and your funds stay at the same address | core/app/network_admin.rs 272–510 | ✅ |
| 12 | Using your own RPC node for a network | Top priority; refused only on proof it's another chain; the relay is sent the URL, key included | network_admin.rs 41–53; S/098 §0.1 | ✅ |
| 13 | The explorer link for each network | Cosmetic; point it at the explorer you trust | W/settings/ui/NetworkDetailPanel.svelte 39–50 | ✅ |
| 14 | RPC provider keys (Alchemy, dRPC, Ankr) | One key covers every network the provider has; stored in plaintext on the device; sent to the relay inside the URL | network_admin.rs 74–75, 538–542 | ✅ say the plaintext part |
| 15 | How Vela picks a node, and why yours may be skipped | A score, not an order: priority minus latency, cooldowns, bans | core/app/rpc_pool.rs | ✅ |
| 16 | When a network can't be reached: Fix RPC | Lists the chains it couldn't read; a 429 never triggers it | W/settings/live.ts 1236–1320 | ✅ (desktop "Report it" dead) |
| 17 | Adding a network Vela doesn't ship | 12 contracts + `0x100`; "couldn't verify" gets a Retry; the relay's directory must list the chain to send | network_admin.rs 95–174 | ✅ |
| 18 | Service endpoints: the four fields | Chain data, passkey index, relay, fiat rates; HTTPS only; the badge is advice, it saves either way | network_admin.rs 189–193 | ✅ link self-hosting |
| 19 | Switching relays: what follows the switch | Fees, the gas tank per network, your RPC URLs | ledger C-fee-1, C-fee-2, C-relay-1 | ⏸ for any figure |
| 20 | Signing in when the passkey index is down | The index is a cache; desktop/phones offer another index at sign-in, web only warns | core/app/login.rs 1–24 | 🗣 |
| 21 | The Trusted Signer page address | Only on a device that chose it; HTTPS or loopback; a copy on another domain can't sign | core/app/sign_pref.rs; ledger C-signpage-1 | ✅ |
| 22 | Device storage: what each row holds | Your data vs cache vs connections; "custom tokens and networks" also wipes RPC overrides | core/storage_catalog.rs 15–118 | ✅ |
| 23 | Clear all caches is safe, and what it leaves alone | First thing to try when a balance looks wrong | storage_catalog.rs 157–160 | ✅ |
| 24 | Custom tokens, and why you can't remove just one | Auto-adds only from authenticated receipts; no app removes a single token yet | core/app/manage_tokens.rs; core/app/token_trust.rs 24–30 | 🗣 gap |
| 25 | Connected sites never expire | No expiry; disconnect what you're done with | core/app/dapp_permissions.rs 66–69 | ✅ |
| 26 | Sign out, remove a wallet, or erase? | Three different things; none touches passkeys or funds | core/app/session.rs 38–90 | ✅ |
| 27 | Erase this device: what goes, and what can't | Scans every `vela.` key; keeps one unconfirmed public key; servers keep relay records ≤14 days, index ≤30 days, chain forever | storage_catalog.rs 119–166; privacy page | ✅ |
| 28 | Several wallets on one device | One wallet is one row; lists aren't synced between devices | session.rs 50–57; ledger C-sync-1 | ✅ |
| 29 | Your keys, as the wallet sees them | Read-only; which key signs on this device | W/settings/live.ts 1525–1600 | ✅ |
| 30 | Backing up your public keys to Ethereum | One transaction, one signature; status read from the chain | core/registry_backup.rs | ⏸ Ethereum fee |
| 31 | Usage statistics on web and the extension | On by default, cookieless, never addresses or amounts; native apps send none | W/analytics/consent.svelte.ts; privacy page | 🗣 requirements A03 still says "no analytics" |
| 32 | Send feedback: exactly what is sent | Never addresses, balances or RPC URLs; always goes to getvela.app, even for self-hosters | W/services/bug-report.ts 1–63 | 🗣 not in self-hosting's list |
| 33 | Debug mode: http dApps on your own network | Developer builds only; 7 taps on the version; never public http | core/prefs.rs 187–330; S/091 | ✅ |
| 34 | Contacts: import and export | JSON or CSV; existing contacts win; lists don't sync, so this is how you move them | core/app/contacts_io.rs 1–40 | ✅ |
| 35 | Explore: favourites and groups | A group is only a view; deleting it keeps the sites | core/app/explore_sites.rs | ✅ |
| 36 | Which settings follow you to another device | None: everything is per device | core/prefs.rs 1–15 | ✅ |

### paying-less — Paying less (18)

| # | Title | The tip, and its number | Sources | Status |
|---|---|---|---|---|
| 1 | When the speed setting changes your fee, and when it doesn't | Under the $0.01 minimum all three speeds cost the same (Gnosis, Stable, Kaia, XRPL EVM, Tempo, the 0.001-gwei L2s) | fee_policy.rs 106, 1016–1018 | ⏸ borderline chains move |
| 2 | Paying the fee in a stablecoin doesn't save money | Same dollar value at the relay's prices; the transfer even uses slightly more gas | fee_policy.rs 18–21, 1022–1035; relay quote.rs | ✅ (size unverified) |
| 3 | Which coin pays when you don't choose | Not one you're sending → stablecoin → larger USD balance | fee_policy.rs 2827–2884 | ✅ |
| 4 | On networks you add, the minimum can be 0.001 of the coin | When nobody can price the coin; #682 (0.001 OKB ≈ $0.12) | fee_policy.rs 983–1014 | ✅ |
| 5 | Your first send on a network costs more | It deploys the wallet; 2,000,000 verification gas reserved instead of 300,000 | fee_policy.rs 109–110, 3126–3130 | ⏸ numbers |
| 6 | Receiving is free, so don't spread small amounts across networks | Each network you later send from charges its deployment once | ledger C-deploy-1 | ⏸ size |
| 7 | Money sent on "the wrong" network isn't lost | Same address everywhere; moving it costs that network's first send | ledger C-deploy-1 | ✅ |
| 8 | Pay up to 60 people for one fee | Split, sweep and payroll import build one operation, one signature | core/app/send.rs 83–86; batch_import.rs 88 | ✅ |
| 9 | Why a batch costs less than separate sends | Each operation carries a fixed reserve and the $0.01 minimum; a batch pays them once | fee_policy.rs 3122–3138 | ⏸ (shrinks, stays) |
| 10 | Let dApps batch: approve and swap in one go | EIP-5792 `atomic: supported`; one fee instead of two, and no wait in between | core/app/dapp_rpc.rs 510–538 | ✅ |
| 11 | Sweeping everything? Pay the fee in the native coin | A stablecoin fee coin holds back 2× the fee, leaving one fee behind | send.rs 4650–4663 | ✅ |
| 12 | Max on the network's coin can send all of it | If a stablecoin covers the fee, Max sends the whole native balance | fee_policy.rs 2837–2842; send.rs 3862–3900 | ✅ |
| 13 | Max on a coin that also pays the fee leaves half a fee | Holds back 1.5× the quote in case the price moves | send.rs 3870–3883 | ✅ |
| 14 | Your own relay: you pay about the real gas, but not less up front | Same formula; the fee goes to your own treasury | site self-hosting 135–139, 181–182; relay fees.md §1 | ✅ |
| 15 | A flaky RPC can raise your fee but never lower it | The higher of two readings; #212 | fee_policy.rs 1141–1161 | ✅ |
| 16 | A refused or held send costs nothing | The fee travels inside the operation | fee_policy.rs 2420–2430; relay fees.md §2 | ✅ (on-chain revert unverified) |
| 17 | Ethereum: timing matters more than any setting | The fee follows the base fee; 0.10–13.1 gwei in one day | memory measurement 2026-10-08 | ⏸ |
| 18 | Back up to Ethereum when gas is low | ~4.31M gas, one time; pick a quiet hour | core/registry_backup.rs 47–60 | ⏸ |

### faster — Landing faster (10)

| # | Title | The tip | Sources | Status |
|---|---|---|---|---|
| 1 | How long a send usually takes on each network | Arc 2 s … Ethereum 24 s; the same at every speed; "longer than usual" after twice that | network_admin.rs 236–241, 272–505 | ✅ |
| 2 | What Fast actually buys | A bigger tip, and headroom for a base-fee spike | relay fees.md §2a, §3 | ⏸ ceilings change |
| 3 | Where Fast doesn't get you in sooner | Where the tip is ~0, twice zero is zero | relay fees.md §2b; live reads | ⏸ |
| 4 | When Slow is a bad idea | Over +43% base-fee rise → held ~35 min, then refused at no charge | relay fees.md §3; hold.rs | ⏸ |
| 5 | Fast for free on cheap networks | Same cost → the send goes Fast, and the screen says so | core/app/fee_speed.rs 357, 385 | ⏸ applies to everyone after the branch |
| 6 | Don't sit on the confirm screen | The quote goes stale after 30 s and isn't repriced before submit | fee_policy.rs 160–166 | ⏸ branch reprices per block |
| 7 | One transaction at a time per network | A second one while the first is pending is refused, free | core/user_op.rs 1690–1692 | ✅ |
| 8 | Your own RPC makes reads steadier, not sends faster | The relay never broadcasts through it | relay docs/rpc.md | ✅ |
| 9 | If the relay's gas tank is empty | Stops at Continue; checks again every 10 s | ledger C-fee-2; send.rs 1537 | ✅ |
| 10 | On BNB Chain, speed is the whole price | Base fee 0: the tier is the tip | relay fees.md §2b | ⏸ |

### security-layers — Security, layer by layer (32)

The ladder (each level: what you add → what it newly covers → what it still doesn't):

1. **One synced passkey** → lost phone, phishing → a lost or taken-over cloud account, a watched passcode. Fine for small amounts in alpha.
2. **A second way in** (security key with a PIN, kept elsewhere) → lost cloud account, deleted passkey → theft of any one key: it's another door, not a second lock.
3. **Spending and savings in separate wallets** → a drained hot wallet doesn't reach savings → wallets that share an account or a security key.
4. **Daily checks** (identicon, green check, first-time tag, risk colours, caps, disconnecting) → poisoning, spoofed names, standing approvals → an app that draws a lie.
5. **The Trusted Signer** → a tampered wallet app → a hacked computer, a tampered page, a correctly shown bad request.
6. **Verify what you run** (attestation, installed code over the web wallet) → a tampered re-upload → bad code in the source.
7. **Remove the dependencies you can** (Ethereum backup, extension zip, your own services) → Vela's services or Vela going away → the getvela.app domain, lost keys.

| # | Title | The point | Sources | Status |
|---|---|---|---|---|
| 1 | Decide your keys before you create the wallet | Fixed at creation; write down kinds, places, accounts | ledger C-keys-1; create_wallet.rs 15–19 | ✅ pilot |
| 2 | The smallest setup: one synced passkey | Its strength is your passcode plus that account | ledger C-keys-2, C-alpha-1 | ✅ |
| 3 | Why the app asks for a second key when your only key doesn't sync | Checked once, at creation; turning sync off later isn't caught | create_wallet.rs 894–900 | ✅ |
| 4 | No "Synced" badge? Treat the key as unsynced | The gate fails open, the badge doesn't | create_wallet.rs 475–483 | ✅ |
| 5 | A synced passkey plus a security key in a drawer | Survives a lost phone and a lost cloud account | site signers | ✅ |
| 6 | Two security keys and no cloud account | No Apple or Google account involved; a PIN and a touch every time | site signers; core/ctap/ceremony.rs | ✅ |
| 7 | Set the security key's PIN, and keep it away from the key | A lockout's only exit erases the key | ceremony.rs 184–224 | ✅ |
| 8 | Name your wallet and keys as if strangers will read them | The registry publishes names and labels forever | ledger C-reg-1 | ✅ |
| 9 | Test your backup key before you fund the wallet | Sign in somewhere with key 2 | core/app/login.rs | 🗣 endorse? |
| 10 | Every extra key is another way in, for you and for a thief | "Extra signers are a way back in, not a second lock." | ledger C-keys-1; site why-vela | ✅ |
| 11 | When more keys make you less safe | Someone else's phone, a shared vault, a PC others use | ledger C-keys-2 | 🗣 |
| 12 | Your passcode and your Apple or Google account are part of your wallet | Face ID falls back to the passcode | ledger C-auth-1; site passkeys | ✅ |
| 13 | Each device signs with the key you signed in with | No per-signature choice since 26 September 2026 | core/app/sign_pref.rs 17–22 | ✅ |
| 14 | Keep spending money and savings in separate wallets | Keys never change; abandoning a small wallet is cheap | ledger C-keys-1, C-compromise-1 | 🗣 endorse? |
| 15 | Don't put both wallets' keys on one security key | Losing it hits both | ceremony.rs 184–186 | ✅ |
| 16 | Know your wallet's identicon | A different face is a different address | core/identicon.rs | ✅ |
| 17 | The green check means you starred it | And imported contact files carry their stars | core/app/contacts.rs; contacts_io.rs | ✅ |
| 18 | Which names next to an address Vela actually checked | Forward-verified reverse names; "Vela User" labels are not verified | core/app/name_verify.rs | ✅ |
| 19 | Read the colour before you slide | Four levels; uncertainty floors at caution | core/app/clear_signing.rs 36–48 | ✅ |
| 20 | A sign-in message for another site is a phishing tell | Classed as phishing; Activity records a plain message | clear_signing.rs 5436–5451 | ✅ |
| 21 | Cap an unlimited approval unless the swap needs it | Ledger wording | ledger C-approve-1 | ✅ |
| 22 | Signed permits are approvals you can't see on-chain | dApp Activity records them; Vela can't cancel one | core/app/dapp_activity.rs | ✅ |
| 23 | Revoking an approval takes a transaction, and Vela has no list | | approval_guard.rs | 🗣 gap |
| 24 | "Refused" means a site tried to take over your wallet | Leave the site | ledger C-selfcall-1 | ✅ |
| 25 | A custom service endpoint is a trust decision | Green means it answered with the right name, not that it is honest | site self-hosting; ledger C-relay-1 | ✅ |
| 26 | Before you sell or give away a device | Erase in Vela, then the OS; a Windows Hello key goes with the PC | storage_catalog.rs | 🗣 OS steps |
| 27 | On a computer that isn't yours | Sign in by phone QR or security key; erase when you finish; there is no app lock | ledger C-sync-1 | ✅ |
| 28 | Check the attestation, not just the checksum | | README; docs/RELEASING.md | ✅ (fix install.md wording first) |
| 29 | Web, extension or app: which code are you trusting today? | | ledger C-access-1, C-plat-1 | 🗣 recommendation |
| 30 | If a key may be in someone else's hands | Ledger wording; move funds network by network | ledger C-compromise-1 | ✅ |
| 31 | If you lose a device | Locked and unknown passcode vs could be unlocked | site signers; whitepaper | 🗣 |
| 32 | Getting ready for Vela disappearing, while it still exists | Extension zip, Ethereum backup, which keys work without getvela.app | ledger C-access-1 | ✅ |

---

## Part 1 — Why it's like this

### principles — What Vela is, and isn't (19)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why Vela has no token | No token, no airdrop, no points; written down as a red line | ledger C-token-1; requirements A03 FR-5 | ✅ |
| 2 | A wallet that does less, on purpose | No NFT gallery, swaps, DeFi dashboard; the one exception taken (the dApp browser) still goes through the single signing path | requirements A01 | 🗣 pricing-analysis.md still models swaps |
| 3 | Why we say "alpha" and never "beta, expect bugs" | It works, holds real funds, start small; no fear banners | ledger C-alpha-1; blog vela-is-in-alpha | ✅ |
| 4 | Why we won't say an audit is "planned" | "A goal for when the project can fund one, not a commitment with a date" | ledger C-audit-1 | ✅ |
| 5 | Why the home page gives three reasons not to use Vela | Trimmed 2026-09-16 because defending them made them stop reading as trade-offs; Maturity row "New, few users" | getvela.app `messages/en.ts` home.tradeoffs | ✅ |
| 6 | Why we stopped saying "no one can freeze your money, not even us" | Stablecoin issuers can blacklist any address in any wallet | en.ts hero comment | 📜 |
| 7 | Why "Vela can't move your funds" always has a second sentence | No key, no role on your Safe, but Vela writes the software that asks your keys to sign | ledger C-custody-1 | ✅ |
| 8 | Why we retired "Your keys. Your face." | Named the mechanism, not the promise; subtitle rewritten three times (never sees → never receives → private key never goes to Vela) | S/059 approved-copy; ledger C-sign-1 | 📜 |
| 9 | Why our posts carry dated corrections, not silent edits | "Corrections, 22 September 2026" | S/080 research D7 | ✅ |
| 10 | Where the code and the docs disagree, the code wins | Five parallel reports + a hostile review found 2 Fatal, 10 High, all fixed before translation | whitepaper callout; S/080 §6a | ✅ |
| 11 | Why the site keeps a claim ledger, and fixes gaps instead of rewording them | 14 product gaps disclosed by 080 were fixed by 081 so the disclosures could be deleted, not softened | S/080 claim-ledger; S/081 FR-020 | ✅ |
| 12 | The wallet counter would rather show nothing than a low number | Live chain read; counts getvela.app wallets only (273 vs 269 before the filter) | site `+page.svelte` 141–222; commit 0ff9432b9 | ✅ |
| 13 | Why we publish known issues we can't fix | Certora M-01, EntryPoint < v0.9 griefing; the relay absorbs it | site docs/security-audits.md | ✅ |
| 14 | Who builds Vela, and why in public | One founder, MONDAY LABS LTD; "you earn it by showing your work" | about page; blog hello-world | ✅ |
| 15 | Why the phone apps cost money and everything else is free | "You pay for convenience, not for access"; the free web wallet is the full trial | en.ts home.pricing; S/063; S/095 | 🗣 price; Mac App Store copy vs ledger C-plat-1 |
| 16 | Why there's no subscription | A wallet that downgrades when you stop paying doesn't fit self-custody | docs/marketing/pricing-analysis.md §四 | 🗣 |
| 17 | Why Vela doesn't buy ads or pay influencers | No tracking means no way to measure; paid promotion of a trust product spends the trust | docs/marketing/distribution-plan.md §四 | 🗣🔒 |
| 18 | What happens if Vela can't pay for itself | 90-day gates committed 2026-07-02, before launch, so the goalposts can't move | docs/marketing/90-day-gates.md | 🗣🔒 |
| 19 | Why old app versions must keep working, even in alpha | Old builds may be suboptimal against a new relay, never refused or stuck | memory note (owner 2026-10-09); relay fees.md §2a | 🗣 |

### privacy — What Vela knows about you (10)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | No email, no KYC, and exactly what goes on-chain instead | Each key's public key, credential ID, model, flags, wallet name, labels, address; "choose a name you're happy to make public" | ledger C-reg-1; privacy page | ✅ |
| 2 | What the web wallet counts, and what it never sends | Cookieless events, masked paths, never addresses/amounts; native apps send none | privacy page (7 Oct 2026) | 🗣 requirements A03 says "no analytics" |
| 3 | Why the apps have no crash reporter | One third-party package on iOS (lottie-ios); `NSPrivacyTracking=false` | docs/store-submission/privacy-and-review.md | 🗣 confirm what apps keep today |
| 4 | Why a bug report becomes a public GitHub issue | Public by design; IP held in memory for rate limiting only | privacy page "Bug reports" | ✅ |
| 5 | What a bug report can never contain | Built from an allowlist; RPC URLs excluded because they often carry an API key | W/services/bug-report.ts header | ✅ |
| 6 | The feedback button that used to send nothing | Five made-up preview lines over a button wired to nothing; now the preview is the payload | bug-report.ts; S/081 FR-016; W/settings/ui/FeedbackBody.svelte | 📜 |
| 7 | Why the relay is told your RPC address, API key included | Spec 081 said never; spec 098 reversed it on 2026-10-03: "send, and say so" | S/098 §0; S/081 US3; relay docs/rpc.md | 📜 |
| 8 | Why the wallet icon dApps see is built into the page | A remote icon URL "would leak every dApp visit to our host" | core/provider/inpage.js 102–108 | ✅ |
| 9 | Why getvela.app forbids its own pages from using passkeys | Any page on the domain could ask your keys to sign; Permissions-Policy, analytics off the key page | site whitepaper threat model | ✅ |
| 10 | Why Vela never asks a website which password manager holds your key | 55 providers compiled in; the 2026-08-22 build asked AAGUID Explorer, which told a third party | core/passkey.rs 1–22; commit 261ec436c | 📜 |

---

## Part 2 — Your wallet

### keys — Passkeys and keys (16)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why a wallet can have seven keys, and not more | Was 21 until 2026-08-14; ~119k gas per key; a 13-key wallet was really deployed and signed by key 12 | core/safe.rs 38–42; scripts/onchain/multikey-safe-gnosis/README.md | 🗣 why 7 exactly |
| 2 | Why we dropped "replace a key" and built "back up to Ethereum" instead | 「更换 signer 确实很复杂而且有风险」; a network you never used still deploys with the original keys | S/062 §0, §4 | 📜 |
| 3 | Issue #159: the request that became multi-key wallets | A user asked for hardware keys; seven keys of any kind followed | issue #159; S/019 | 🗣 |
| 4 | Why your new passkey signs once before the wallet exists | Issue #1: create() succeeded, nothing was stored, and a fundable wallet was already saved | core/app/create_wallet.rs 15–17; commit 62b20fe5d | ✅ |
| 5 | Why Vela insists your passkey is "discoverable" | Otherwise it never appears in Google Password Manager and dies with the device | commit 6abd3370c; S/019 results | ✅ |
| 6 | Why Vela only accepts P-256 passkeys | RSA dropped 2026-07-03; an RSA-only key passed create() and left an orphan | commit 5a4077d21 | ✅ |
| 7 | Why some password managers can't make a Vela wallet | Safe's verifier needs clientDataJSON to start exactly `{"type":"webauthn.get","challenge":"` | core/webauthn.rs 16–17, 203–240 | 🗣 is Xiaomi still the case? |
| 8 | Why the app no longer picks your first key for you | A Samsung owner with a YubiKey only saw "Setup was cancelled" (2026-08-26) | create_wallet.rs 1120–1130; S/019 | ✅ |
| 9 | Why every key in a wallet must come from the same website | One domain hash per registry record; options that break it are dimmed with a reason, not hidden | create_wallet.rs 122–142 | ✅ |
| 10 | Why iPhones need iOS 17.4 for a multi-key wallet | Without `excludedCredentials` a second passkey can silently replace the first | S/019 results T125 | ✅ |
| 11 | Why a wallet name fits nine Chinese characters but not ten | User handle is 64 bytes; UUID + separator take 37 | core/app/mod.rs 98–103 | ✅ |
| 12 | Why the create checklist has three boxes, none ticked | Was four, then two; the two-box version never said the name goes on-chain. "A checklist people tick without reading records nothing." | create_wallet.rs 54–77; W/ui/onboarding/v2/NameScreen.svelte | ✅ |
| 13 | Why a phone you scanned is never drawn as a USB key | #207: icon, caption and badge came from three unrelated signals | core/passkey.rs 95–121 | ✅ |
| 14 | Why a missing "Synced" badge never blocks you | "A badge is not a gate" | core/passkey.rs 385–436 | ✅ |
| 15 | Why you say where your key is once, not at every signature | A per-signature picker lived one week (09-19 → 09-26) | core/app/sign_pref.rs; commit 1457cb4d7 | 📜 |
| 0 | The four ways to create a wallet or sign in, and how they differ | This device, another phone, a security key, the Trusted Signer: what each key is, where it lives, which apps offer it | founder request 2026-10-09; core/app/mod.rs KeyMethod | ✅ pilot draft |
| 16 | "Touch ID or Windows Hello", said to an iPhone | An iPhone 11 was told both; neither applied | core/app/method_words.rs; S/087 F01 | ✅ |

### address — Your address and the Safe underneath (9)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why your address is the same on every network | No chain id in the derivation; `0x100` is | ledger C-addr-1, C-p256-1; core/safe.rs | ✅ |
| 2 | Why you can receive before your wallet exists | It deploys itself in your first transaction there | ledger C-deploy-1 | ✅ |
| 3 | Why no contract in the funds path was written by Vela | Unmodified Safe v1.4.1 + Safe's modules | ledger C-acct-1; site docs/account-contract | ✅ link the doc |
| 4 | Why any one of your keys can deploy your wallet on a new network | Keys 2–7 are deployed inside the Safe's own setup; GS024 | core/safe.rs 134–142 | ✅ |
| 5 | Why the order of your keys doesn't matter, and the same key twice is refused | A duplicate makes a wallet you could fund and nobody could ever deploy | core/safe.rs 304–311 | ✅ |
| 6 | Why adding multi-key support moved nobody's address | `0x762EdA60…329F` is a release blocker | core/safe.rs 8–16 | ✅ |
| 7 | Your address was the one number that could never change | The Rust port ran beside the old code; zero mismatches before the TS was deleted | S/001 FR-002/006/007 | ✅ |
| 8 | Why we took Safe's signer bytecode from the chain, not from npm | npm 0.2.0 (solc 0.8.24) ≠ deployed v0.2.1 (0.8.26, viaIR) | core/safe.rs 55–59 | ✅ |
| 9 | Why multi-key wallets can't deploy on some networks yet | 11 of 24 lacked Safe's signer factory (2026-09-23); "the real friction is holding eleven native coins" | commit 83dbab4e6 | 🗣 current count |

### sign-in — Signing in, recovery and the registry (16)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why your account list doesn't sync between devices | Nothing to sync, so nothing for Vela to hold; any key rebuilds it | ledger C-sync-1 | ✅ |
| 2 | Why sign-in still works if Vela's index is gone | Reads the registry contract directly (Gnosis, then Ethereum) | core/registry_chain.rs | ✅ |
| 3 | How one signature can find your wallet | One P-256 signature gives two candidate keys; the wrong one "has no holder" | core/webauthn.rs 253–258; core/app/login.rs | ✅ |
| 4 | Why Vela checks its own index server against the chain | "Our own service lied or broke" | core/registry_resolve.rs; S/067 | ✅ |
| 5 | Why the public-key registry belongs to nobody | No owner, no proxy, append-only; "Vela is the registry's first client, not its owner" | p256: README | ✅ |
| 6 | Why the registry can never change a wallet's keys | "References are claims, not authority" | p256: README | ✅ |
| 7 | Why creating a wallet makes a key nobody uses again | A throwaway software key closes each record | core/registry_proof.rs | ✅ |
| 8 | Why the index pays your registration gas | 1.1M gas for one key, 3.6M for seven; 5/min per IP + a budget | p256: README | ✅ |
| 9 | Why the index sends one wallet per transaction | Failures attribute to one task; a daily heartbeat because "a silent channel becomes a signal" | p256: README | ✅ |
| 10 | How your wallet's record reaches Ethereum without signing again | The registry keeps the exact bytes; anyone may replay them. 87 s on Ethereum | core/registry_backup.rs; S/062 results | ✅ |
| 11 | Why a one-key wallet opens before the chain confirms it, and a multi-key wallet waits | 6.9 s of 7.75 s was waiting on the registry (#409) | create_wallet.rs 29–37; commit 7e3e61c11 | ✅ (fix site, see below) |
| 12 | Why a wallet's name can be found from its address | Reads the first key out of the Safe's own storage | core/registry_lookup.rs | ✅ |
| 13 | Why the same wallet never appears twice on one device | A wallet is its address, not its passkey | core/app/login.rs 632–650 | ✅ |
| 14 | Signing out is not erasing | Only the account list goes; "all six or none" became remove one | core/app/session.rs 58–88 | ✅ |
| 15 | Why "Erase this device" keeps exactly one record | An unconfirmed public key; the old hand-kept list of 11 keys missed contacts | core/storage_catalog.rs; commit 968429fbd | ✅ |
| 16 | Why "check your network" and "change the server" are different messages | If no attempt left your machine, another server can't help | core/app/login.rs 42–45, 128–131 | ✅ |

### security-keys — Security keys and scanning with a phone (12)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the desktop app talks to your security key itself | "A wallet whose only key path runs through an OS passkey service is a wallet that a lapsed domain association can padlock" | core/ctap/mod.rs 8–19 | ✅ |
| 2 | Why Windows reaches your key through the smart-card port | Build 1903 reserves the USB interface for Windows' own dialog | app-desktop/…/ctap/ccid.rs | ✅ |
| 3 | Why Vela never says "locked" when you mistype a PIN | 0x33 vs 0x34; a locked key's only exit destroys the wallet's key | core/ctap/ceremony.rs 444–457 | ✅ |
| 4 | Why Vela draws its own PIN keypad | A key that can type one-time codes is a keyboard, so the OS hides the on-screen one | S/019 deviations §14 | ✅ |
| 5 | Why the PIN sheet opens all the way | OnePlus 5T at 1.3× text: the last keypad row was below the fold | app-android/…/UsbCeremonyPrompts.kt 75–81 | ✅ |
| 6 | Why a security key on iPhone needs YubiKey firmware 5.8 | Smart-card path, no Apple service; 5.7 doesn't work | S/019 deviations §16 | ✅ |
| 7 | Why the scan-with-your-phone QR has exactly six fields | A seventh, allowed by the standard, broke Google Password Manager's older reader | core/cable/qr.rs 28–42 | ✅ |
| 8 | The upper-case letters that let iPhones sign | Apple's relay said "Policy violation" to lower-case connection IDs | commit 245784794 | ✅ |
| 9 | Why a failed scan says "scan again", not "set up Face ID" | #446, #450 | core/cable/conn.rs 55–76 | ✅ |
| 10 | Why Vela tells Android where your passkey lives | No hints → "Connect your security key" for a phone; a smart-card hint emptied the list | core/app/shell.rs 127–139 | ✅ |
| 11 | Why Android's passkey sheet gets ten seconds | TransactionTooLargeException at 556,892 bytes: too many saved passkeys | W/onboarding/core/passkey.ts 243–290 | ✅ |
| 12 | Why a phone app you build yourself can't use "this device" | Platform passkeys only reach apps signed with our key; an empty entitlements file once broke passkeys while every test passed | docs/ARCHITECTURE.md; S/019 T138 | ✅ |

---

## Part 3 — Moving money

### fees — What you pay, and why (20)

> ⏸ Branch `fix/eth-mainnet-fee` changes the default speed and the fee formula. Write nothing that
> states the formula or the default until it lands, then update ledger C-fee-1 / C-fee-3 first.

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the fee is one exact number, not a breakdown | The fee is a transfer leg inside your Safe operation; displayed = signed | core/app/fee_policy.rs 1–6; send.rs 22–27 | ✅ |
| 2 | Why the fee can't change after you sign | Zero EntryPoint fees; a changed payment invalidates your signature | relay: docs/fees.md §1 | ✅ |
| 3 | Why there's no gas account or activation deposit any more | The old gate: nonce ≤ 3 + registration + treasury | ledger C-fee-2; S/080 research | 📜 |
| 4 | Why nobody sponsors your gas | No paymaster, so nobody can gate your transactions with a policy | ledger C-fee-3 | ✅ |
| 5 | Why we describe the fee by its formula, not as "2× gas" | "~60% markup", "≈2×", "≈1×": every multiple in circulation was wrong | S/080 research D1 | 📜 |
| 6 | Why the markup is spent, not spare | The signed amount is never repriced, so the 3× absorbs drift | relay: docs/fees.md §3 | ⏸ |
| 7 | Why Vela refuses a relay quote above 3× its own reading | The check can't be fooled by the quote it audits | fee_policy.rs 94–100 | ✅ |
| 8 | Why a failed gas-price read shows no fee instead of a guess | A 5 gwei fallback quoted an Ethereum transfer at $23, ~80× (2026-09-26) | fee_policy.rs 2140–2152 | ✅ |
| 9 | Why the smallest fee is one cent | And why speeds cost the same on cheap chains; measured 2026-09-21 | fee_policy.rs 106, 941–960 | ✅ |
| 10 | Why a network you add yourself no longer pays 12× the minimum | 0.001 OKB ≈ $0.12; a thin pool once priced OKB at $5 | fee_policy.rs 969–1010 | ✅ |
| 11 | Why money in Vela can't lose its unit | Five bugs, one cause: an absent factor became 1; a CNY figure labelled "USDC" | core/app/money.rs 1–32 | ✅ |
| 12 | Why Ethereum mainnet fees were up to 23.5× the chain cost | 3 × 3.98 × 1.97, measured to the wei | memory note; branch fix/eth-mainnet-fee | ⏸🗣 ledger says don't lead with a multiple |
| 13 | Why a new wallet's first contract call gets its gas measured separately | Base: 265,786 estimated for a 4.3M-gas call | core/user_op.rs 575–596 | ✅ |
| 14 | Why the same BNB transfer once quoted two different fees | #212: exactly 2×, one flapping endpoint | fee_policy.rs 199–216 | ✅ |
| 15 | Why the fee retries on its own | iOS once showed "Estimating…" for 4 min 35 s | fee_policy.rs 777–812 | ✅ |
| 16 | Why Vela won't pay a swap's fee in the coin being swapped | 096 F2: PancakeSwap approved all the USDC to Permit2 | commit 6fba8ef5b | ✅ |
| 17 | Why the fee prefers a stablecoin | "A fee in dollars reads as what it costs" | fee_policy.rs 2833–2848 | ✅ |
| 18 | Why a greyed-out fee coin tells you why | "Need X, have Y"; #408, #438 | fee_policy.rs 818–827 | ✅ |
| 19 | Why Max sometimes fills 0, and says why | #210 | send.rs 924–931 | ✅ |
| 20 | Paying gas in dollars on Tempo | pathUSD at 2×; the 89,700 < 90,025 deploy | fee_policy.rs 170–196 | ✅ |

### speed — Choosing a speed (10)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the speed defaults to Fast | The feature must not make an existing send slower | core/app/fee_tier_pref.rs 18–23 | ⏸ default may become Standard |
| 2 | Why your speed lives in Settings, and the send screen overrides once | "A setting that silently drifts is a setting nobody can trust" | S/068; fee_tier_pref.rs 30–37 | ✅ |
| 3 | Why "Economy" was renamed "Slow" | 经济便宜 mixed a value judgement into a speed scale | S/068 tier table | ✅ |
| 4 | Why paying for Fast once bought no priority | The relay scaled the cap, not the tip; "never validate a pricing change on BSC alone" | relay: fees.md §2a | ⏸ |
| 5 | Why "Slow" never tips below the market | An under-tip is rejected, not mined late | relay: fees.md §2a | ✅ |
| 6 | Why the speed you're quoted is the speed the relay signs | Polygon: quoted 107.7 gwei, signed 34.72 | relay: fees.md §2b | ✅ |
| 7 | Why your send sometimes goes Fast when you chose Slow | Same cost → Fast, and the screen says so | core/app/fee_speed.rs 350–400 | ✅ |
| 8 | Why some networks have only one speed | Decided from the numbers, never from a list of chains | fee_speed.rs 330–345 | ✅ |
| 9 | Why each speed shows a gas-price range | The chain never charges the cap; #684, #685 | fee_policy.rs 2292–2330 | ✅ |
| 10 | Why the price you tapped is the price you get | #681 | fee_speed.rs 35–39 | ✅ |

### relay — The relay (10)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | What a relay can and can't do to your transaction | Can delay, refuse, choose timing (front-run within slippage); can't change recipient, amount or fee | ledger C-relay-1; whitepaper | ✅ |
| 2 | Why you can swap the relay, but it has to be vela-relay | A Vela-specific quote method; one secret → treasury + 100 relayers at the same addresses on every chain | ledger C-relay-1; site self-hosting | ✅ |
| 3 | Why the wallet asks the relay "can you pay?" before you sign | 404 vs 503; anvil's 31337 belongs to a dead GoChain testnet | relay treasury.rs; S/098 | ✅ |
| 4 | Who fixes an out-of-gas relayer depends on who added the network | "A shrug or a request for money that should never have been asked for" | core/app/network_admin.rs 524–535 | ✅ |
| 5 | Why the relay never broadcasts through your RPC | A caller's node could lie about a nonce; 198 of 2,602 chains lack a public endpoint | relay docs/rpc.md; S/098 §3 | ✅ |
| 6 | Why the relay reprices a short payment but never signs at a loss | `FloorUnfundable`: "a clean rejection, never a loss" | relay fees.md §1–2a | ✅ |
| 7 | Why the relay runs on Docker and Cloudflare from one core | An I/O-free decision core, byte-identical JSON-RPC | relay README | ✅ |
| 8 | Why the relay asks Binance for one price | To value stablecoin fees and cap top-ups at $20; the wallet itself uses on-chain prices | relay README; fees.md §4 | ✅ |
| 9 | The Arbitrum top-up that couldn't land | 21,000 gas vs ~21,397; re-sent forever (2026-10-03) | relay PRs #16/#18/#19 | 🗣 publish? |
| 10 | The Avalanche upgrade that stalled every send for two weeks | Since 2026-09-22 C-Chain charges ≥ half the limit; #440 | relay PR #20 | 🗣 publish? |

### signing — What you see is what you sign (15)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the confirm screen is decoded from the bytes you sign | | ledger C-clear-2 | ✅ |
| 2 | Why Vela refuses a request to change who controls your wallet | "As completely as the payload that drained Bybit" | core/app/self_call_guard.rs 1–24 | ✅ |
| 3 | Why a typed-data request carrying two documents is refused | `[benign, malicious]` could show one and sign the other | core/typed_data_request.rs 1–23; S/085 audit | ✅ |
| 4 | Why "Verified" only appears for descriptions built into the app | Fetched over plain HTTP from an editable address used to say verified | core/app/clear_signing.rs 43–48 | ✅ |
| 5 | Why "this node can't simulate" is a caution, not red | Arbitrum's public node answers -32603 "method handler crashed" | core/app/sim_outcome.rs 17–26 | ✅ |
| 6 | Why a contract's error message is cleaned before you see it | A reason containing `$t(` would make the wallet print a sentence of the contract's choosing | sim_outcome.rs 28–39 | ✅ |
| 7 | Why a simulated "you receive" amount is shown only for tokens you trust | A hostile contract can emit a fake `Transfer(_, you, big)` | core/app/token_trust.rs 16–50 | ✅ |
| 8 | Why the slide stays shut, and says why | One gate, first blocker first; iOS had two copies of it | core/app/sign_confirm.rs 1–19 | ✅ |
| 9 | Why there's no Reject button next to slide-to-confirm | Closing is the rejection; 88% of the track, "far more than a mis-tap and far less than a fight" | W/signing/ui/SlideToConfirm.svelte | ✅ |
| 10 | Why a swipe no longer rejects a signature | A stray touch at 08:29:26 rejected a request (2026-09-28) | S/079 ruling 1; W/wallet/ui/BottomSheet.svelte 35–42 | 📜 |
| 11 | Why the signing sheet became a button | Decided 8 October 2026 (#461): every app's signing sheet confirms with a tap; folded into the no-reject-button note | issue #461 | ✅ done |
| 12 | Why Vela stopped blocking unlimited approvals | Permit2 batches spend the approval in the same transaction; ruling 2026-09-26 | core/app/approval_guard.rs 12–22 | 📜 |
| 13 | Why "unlimited" starts at 2^200 | Three thresholds used to disagree | approval_guard.rs 118–124 | ✅ |
| 14 | Why "increase allowance" has no Revoke, and shows the total | "Increase by 100 must never read as cap at 100" | approval_guard.rs 550–567 | ✅ |
| 15 | Why a huge amount on the signing sheet wraps at its commas | 10^30 USDC ran off a 360 px panel; "≈ $1e+24" | W/signing/ui/AmountHero.svelte; commit e5c4cb8bd | ✅ |

### trusted-signer — The Trusted Signer page (8)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 0 | The Trusted Signer vs signing in the app | Who decodes, who computes the digest, who asks the key; what each protects against | founder request 2026-10-09; ledger C-signpage-1 | ✅ pilot draft |
| 1 | Why the signing page can't reach the internet | The network block is inside the hashed bytes; it cost the favicon and, for a while, wallet creation | S/076 FR-003 | ✅ |
| 2 | Why the wallet app doesn't serve the signing page itself | "Not a trade, a hole" | S/075; S/076 | ✅ |
| 3 | Why every version of the page has its own address | Bun and Node build identical bytes; 12 accepted versions | core/trusted_signer/integrity.rs | ✅ |
| 4 | Why the app checks the page but doesn't block it yet | "A self-inflicted outage in the shape of a security feature" | integrity.rs 35–58 | ✅ (ledger C-signpage-1, not the code comment) |
| 5 | Why we built Bluetooth and cross-device signing, then cut them | 「我确定砍掉蓝牙」, the next day | S/075; TS/HANDOVER.md | 📜 |
| 6 | Why the signature comes back through a link any app could claim | 071 rejected it, 076 measured and adopted it | core/trusted_signer.rs 48–79 | ✅ |
| 7 | Why the page shows your account's name, never the recipient's | The recipient's name comes from the requester | TS/HANDOVER.md ruling #4 | ✅ |
| 8 | Why hosting your own signing page makes your domain the lock | "A trap for somebody who does not know they made it" | S/075 | ✅ |

### send-receive — Sending and receiving (17)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why a name shown for an address must resolve back to it | "Set the reverse record … to vitalik.eth, and four wallets called it that" | core/app/name_verify.rs | ✅ |
| 2 | Why the green check needs a saved and starred contact | A poisoned look-alike is never starred | core/app/contacts.rs 14–18 | ✅ |
| 3 | Why the identicon sits inside the recipient field | Poisoning copies the first and last characters, not the picture | W/flows/ui/RecipientField.svelte | ✅ |
| 4 | Why Vela warns when you send tokens to their own contract | | S/096 F12 | ✅ verify |
| 5 | Why the receive QR is just your address by default | #208 closed as unsupported, reversed after #312 | core/app/payment_request.rs; S/090 | 📜 |
| 6 | Why there's a face in the middle of the QR | Network, token or identicon: something that must match | W/flows/ui/QRCard.svelte | ✅ |
| 7 | Why the QR card stays light in dark mode | Dark mode punched a dark hole in a code a camera must read | S/021 results #9 | ✅ |
| 8 | Why the QR doesn't grow with your text size | A code that shrinks for its caption stops scanning | W/flows/ui/QRCard.svelte | ✅ |
| 9 | Why a screenshot of a Vela QR now scans | Android's reader couldn't read Vela's own code from a screenshot | core/qr_scan.rs; S/090 | ✅ |
| 10 | The share card redrawn after a WeChat Pay card | 480×700, ignores the theme, says what it is on its own | W/flows/ui/ShareCard.svelte; commit 4b10edca4 | ✅ |
| 11 | Why the receive address is never shortened | Two lines, monospace, an error colours the border | W/flows/ui/AddressCard.svelte | ✅ |
| 12 | Why the Receive screen ignores a shorter token list | "A merchant at a counter" judges payment by this screen | core/app/receive_watch.rs | ✅ |
| 13 | Why a pay link refuses `amount=1e18` | It became ≈7.5×10⁴ tokens | core/app/payment_request.rs 18–23 | ✅ |
| 14 | Why a payroll import refuses to guess an exchange rate | 5000 CNY previewed as 5000 USDT behind a green Apply | core/app/batch_import.rs 19–61 | ✅ |
| 15 | Why a recipient named "Alice123" never becomes the amount | #137, regressed twice | batch_import.rs 13–18 | ✅ |
| 16 | Why duplicate batch rows are flagged, not removed | #203 | core/app/send.rs 240–252 | ✅ |
| 17 | How 阿豪 ends up under "A" | The web filed every Chinese name under # for two specs | core/app/contacts_initials.rs | ✅ |

### transactions — After you press send (8)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why a timeout never marks your transaction failed | Calling it failed invites a re-send; "an honest unknown" | core/app/tx_tracker.rs 18–35 | ✅ |
| 2 | Why Vela writes the record before your transaction leaves | EX13: a landed op read "not on chain yet" for 5 min 49 s | tx_tracker.rs 37–81 | ✅ |
| 3 | A payment that landed was reported "failed, try again" | Following the advice pays twice (082 G21, P0) | S/082 rulings 1, 8 | 📜 |
| 4 | Why the progress ring never fills by itself | 70% at the usual time, never past 92%; #199 | W/flows/ui/ring.ts | ✅ |
| 5 | "About 24 seconds": why networks you add get no countdown | A ring for an unknown chain "would be an invented promise" | network_admin.rs 510–522 | ✅ |
| 6 | Never "pending forever": why "unknown" is an honest answer | | S/087 F04/F05; PARITY D2.3 | ✅ |
| 7 | When EntryPoint says success and your Safe says no | | tx_tracker.rs 2192–2210 | ✅ |
| 8 | Why incoming coins once showed up only after a restart | Native coin into a Safe emits `SafeReceived`, not a Transfer (#443) | issue #443 | ✅ verify |

### balances — Balances, prices and numbers (16)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why prices come from on-chain pools, with Chainlink as referee | DEX price used only within 0.5×–2× of Chainlink | core/app/balance_dashboard.rs 526–600 | ✅ |
| 2 | Why some coins are pegged at $1 and others never are | The peg was a `symbol == "USD"` literal in four places that "agreed by luck" | balance_dashboard.rs 528–560 | ✅ |
| 3 | Why your total never drops because a network didn't answer | #188 | balance_dashboard.rs 16–22 | ✅ |
| 4 | "Can't reach 3 networks": why we list every one, even the empty ones | "While a network cannot be read, nobody knows what it holds"; ~5 of 24 unreachable from mainland China | S/092 | ✅ |
| 5 | Why a brand-new wallet that's offline doesn't say $0 | | W/wallet/live.ts 360–386 | ✅ |
| 6 | Why one token address can be two different tokens | `0x4200…0006`; USDC with 6 and 18 decimals | core/app/token_registry.rs 11–16 | ✅ |
| 7 | Why airdropped tokens don't show up on their own | One admitted scam token would poison two trust lists | core/app/token_trust.rs | ✅ |
| 8 | Why the cents are smaller than the dollars | #231: a big "4.00" left you to guess dollars or coins | W/wallet/ui/BalanceDisplay.svelte | 🗣 the 0.58 symbol rule is unused |
| 9 | Money in is green; money out isn't red | Red means something went wrong; a transfer you chose did not | W/flows/ui/AmountHero.svelte 3–7 | ✅ |
| 10 | Why typing "4,5" means 4.5 on every keypad | "1.5e-7" salvaged digit by digit became 1.57 | core/l10n/amount_text.rs; S/073 | ✅ |
| 11 | Why "08" turns into "8" while you type | #421: read as 8 by someone who meant 0.8 | amount_text.rs 54–66 | ✅ |
| 12 | Your number format is a setting, not a guess | "The user chose 'space', not 'French'"; old formatter wrong for 21 of 137 currencies | core/l10n/currency.rs, number.rs | ✅ |
| 13 | Why a tiny balance never shows 0, and a change never reads −0 | 1000 wei read "xDAI −0" | core/l10n/number.rs 250–254 | ✅ |
| 14 | The interest payment that showed up a trillion times too big | 1.373924e-12 read back as +1.373924 | commit 02c24bee3 | ✅ |
| 15 | Why your balance shows dollars when the rate is missing | "A defaulted 1 under a ¥ is a lie" | W/wallet/live.ts 168–172; core/app/display_currency.rs | ✅ |
| 16 | Why a token logo that failed comes back after a minute | One bad minute left every token a lettermark until the app was killed | core/app/remote_mark.rs | ✅ |

---

## Part 4 — Out in the world

### networks — Networks (17)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why a chain without the P-256 precompile can't run Vela | Chain setup shows no steps, "because offering them would imply they lead somewhere" | site lib/chain-setup/verdict.ts; ledger C-p256-1 | ✅ |
| 2 | The twelve contracts a network needs, and the one we stopped requiring | | core/app/network_admin.rs 95–172; S/081 FR-009 | ✅ |
| 3 | Why Vela checks the precompile with a real signature | A precompile has no bytecode | network_admin.rs 174–185 | ✅ |
| 4 | "Unable to verify" is not "incompatible" | Celo had everything and was called incompatible | commit 0b95b1aa0 | ✅ |
| 5 | How eleven networks were admitted in one afternoon | Every probe had a control (2026-09-17) | S/061 | ✅ |
| 6 | Arc: the coin that is also a token | Listing the ERC-20 view doubles every balance | S/060 R2 | ✅ |
| 7 | Why Arc transactions have a 20 gwei floor | Arc silently drops anything cheaper; "the worst failure this wallet can produce" | fee_policy.rs 130–160 | ✅ |
| 8 | Stable: the same trick as Arc, found by division | 2000000000001 wei ↔ 2 | S/061 research | ✅ |
| 9 | Tempo has no coin of its own | `eth_getBalance` returns the same constant for every address | balance_dashboard.rs 224–230 | ✅ |
| 10 | What the chain-setup tool is for | Some contracts anyone can deploy; one only Safe can sign | site routes/chain-setup | ✅ |
| 11 | The site said "12 networks" for a week after we shipped 24 | Now a test mirrors the core's list | site lib/networks.ts | ✅ |
| 12 | How Vela picks an RPC endpoint | Six tiers, latency, cooldowns; a black-holed node cost 8.5 s per read | core/app/rpc_pool.rs | ✅ |
| 13 | "Rate-limited" isn't "down" | Grey line, no button; a 429 banner would train people to ignore banners | rpc_pool.rs 39–46; W/settings/ui/BalanceDetailBody.svelte | ✅ |
| 14 | Settings checks that an endpoint is what it says it is | "An RPC endpoint that answers 302 is not an RPC endpoint" | network_admin.rs 42–54 | ✅ |
| 15 | Why Vela doesn't trust your phone's "connected" indicator | Ten offline/back flaps in two minutes | core/app/net_health.rs | ✅ |
| 16 | The desktop follows your system proxy and never goes around it | One timeout once flipped all traffic to direct for 60 s | app-desktop/…/executor/proxy.rs | ✅ |
| 17 | Why a fee bug can hide on BNB Chain | Base fee 0 makes speed tiers inert | relay fees.md §2b; core/app/fee_speed.rs | ✅ |

### dapps — How dApps talk to Vela (16)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why there's no WalletConnect, and what happened to WalletPair | 「wallet pair 不用接呀 直接用 dapp browser inject 就行呀」; 1,959 lines left unwired | S/027 D40/D43; ledger C-dapp-1 | 📜🗣 |
| 2 | Why Vela tells old dApps it's MetaMask, and new ones the truth | Reversed an earlier "never spoof" stance | core/provider/inpage.js 180–195 | ✅ |
| 3 | Why the provider answers from an allowlist | A denylist would make the wallet an open relay for `eth_signTransaction` | core/app/dapp_rpc.rs 52–108 | ✅ |
| 4 | A dApp can't ask Vela for `eth_sign` | One routing table replaced four | dapp_rpc.rs 4–12 | ✅ |
| 5 | Why "add this token" from a dApp always says no | "Tokens join through Vela's own trust rules, not because a page asked" | dapp_rpc.rs | ✅ |
| 6 | A page that goes away gets 4900, never 4001 | dApps retry a 4001, which can double-spend | core/app/dapp_browser.rs 28–37 | ✅ |
| 7 | Each site remembers its own network, and starts on Ethereum | Native browsers had started on Gnosis because the test fixture lives there | dapp_browser.rs 49–50 | ✅ |
| 8 | A transaction prepared for another chain is refused | "Calldata prepared for another chain means something else" | core/app/sign_request.rs 1058–1067 | ✅ |
| 9 | Why plain-http sites don't get the wallet | `10.0.0.1.evil.com` is a public name anyone can register | core/app/dapp_permissions.rs 860–915 | ✅ |
| 10 | When a site asks to add a network Vela doesn't have | Vela's own sheet runs the Settings check | dapp_rpc.rs; S/100 | ✅ |
| 11 | Why Vela switches you after adding a network, though the standard says not to assume it | wagmi throws otherwise | S/100 R4 | ✅ |
| 12 | A dApp is told only the account you're signed in to | #315 | dapp_permissions.rs 26–32 | ✅ |
| 13 | Activity says where it happened, never in the dApp's own words | | core/app/dapp_activity.rs; S/093 | ✅ |
| 14 | Why the Chrome extension opens a side panel, not a popup | A popup closes when the passkey prompt takes focus | W/../extension/README.md; S/027 D34 | ✅ |
| 15 | Same passkey, same address: how the extension signs as getvela.app | "One passkey, one address, two doorways" | S/027 D31–D32 | ✅ |
| 16 | We test dApps with real money | A golden Safe through PancakeSwap, Uniswap, Aave, CoW, Curve, Sky; 12 findings | S/096, S/097 findings | ✅ |

### browser — The built-in browser (11)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the web wallet has three tabs, not four | A page inside a browser can't host a browser | W/wallet/ui/TabBar.svelte; S/022 | ✅ |
| 2 | Why the desktop browser isn't Chromium in a box | "No 200MB of CEF"; it draws above everything, so signing is a third column | app-desktop/…/webview.rs 16–39 | ✅ |
| 3 | Switching tabs no longer reloads the page | One webview per tab, six live, a busy tab never suspended | webview.rs; core/app/browser_tabs.rs; S/099 | 📜 |
| 4 | One request log per tab, and what it never writes down | Owner: 「要清晰，好定位问题」 | core/app/dapp_record.rs | ✅ |
| 5 | The installed Windows app's browser had never opened | WebView2's data folder in Program Files; a crash report every 1.7 s | S/083 W1 | ✅ |
| 6 | No "continue anyway" on a bad certificate | "A typo … a warning: neither heals by asking again" | core/app/browser_load.rs 250–270 | ✅ |
| 7 | Why the address bar shows a lock and no "Secure" label | 「HTTPS 并不代表这个站点真的安全」 | S/079 ruling 5 | ✅ |
| 8 | Only `mailto:` and `tel:` may leave the desktop browser | `ms-msdt:` and `search-ms:` have been remote-code holes | webview.rs 220–260 | ✅ |
| 9 | Why Recents never shows "can't open page" | #329, #425 | browser_load.rs; browser_history.rs | ✅ |
| 10 | Typing a word searches DuckDuckGo; `javascript:` never runs | | dapp_rpc.rs 910–925 | ✅ |
| 11 | The old Safari extension trusted a Universal Link for 14 days | | core/app/ext_cache.rs | 📜 |

---

## Part 5 — The interface

### interface — Interface details (21)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Quiet, typographic, de-containered | The card pile was removed; the wallet home is the reference | docs/DESIGN-LANGUAGE.md | ✅ |
| 2 | Orange means money moves | The July review found orange on errors, sign-out and filter chips | docs/DESIGN-REVIEW-2026-07.md; W/ui/Button.svelte | ✅ |
| 3 | Why the white label on the orange button is a recorded exception | 3.6:1 against 4.5:1 | W/tokens/contrast.test.ts | 🗣 still open |
| 4 | A control that can't act is not drawn as one | "Dimming alone is not a reason"; the ⌘K box that ran nothing | W/signing/ui/FeeRow.svelte; S/081 | ✅ |
| 5 | A button that's working doesn't look disabled | | W/ui/Button.svelte 26–31 | ✅ |
| 6 | Why the "quote is old" note never pushes the button down | A control that moves under your thumb is how a wrong tap happens | W/flows/ui/FeeStaleNote.svelte | ✅ |
| 7 | One bottom sheet, everywhere | Four designs behaved four ways | W/wallet/ui/BottomSheet.svelte; sheet-gesture.ts | ✅ |
| 8 | No bottom sheets on a desktop | 440 px fits a 42-character address in mono | W/contacts/ui/SheetOrDialog.svelte | ✅ |
| 9 | Why the phone tab bar has no words | "Configur…" under a 97 px tab | W/wallet/ui/TabBar.svelte | ✅ |
| 10 | The Settings tab that signed you out | On all four clients, before spec 023 | S/023 spec | 📜 |
| 11 | Vibration only for what your eyes can't confirm | 「很多地方应该加，但不能泛滥」 | docs/design-system.md 190–209 | ✅ (doc vs VelaHaptic.kt differ on tab switch) |
| 12 | The launch animation plays once a week | | W/launch/constants.ts | ✅ |
| 13 | Why the logo holds for 400 ms, not 2 seconds | The canvas was 92.3% empty | S/012 research | ✅ |
| 14 | The intro you only see once | 「不然就很烦」 | W/intro/gate.ts | ✅ |
| 15 | Progress that only moves when something happened | The percentage meter was three statuses divided by three | W/ui/onboarding/v2/ProgressScreen.svelte; S/014 | ✅ |
| 16 | Your account picture is a security control | `0xd8da…6045` vs `0xd8db…6045` | S/003; core/identicon.rs | ✅ |
| 17 | Why every avatar is the identicon, with no initials option | Initials are something anyone can type | commits 47ad833ed, 111bf9e22 | 🗣 |
| 18 | Tap any identicon to see it big, beside its address | | W/wallet/ui/IdenticonViewer.svelte | ✅ |
| 19 | Why the identicon code cares how JavaScript prints a number | 2 in 9,068 doubles print differently | core/identicon.rs 9–36 | ✅ |
| 20 | Why identicons are circles, not hexagons | A shared SVG id made every avatar square | core/identicon.rs 548–593 | ✅ |
| 21 | Grey for a rate limit, red for a dead RPC | A green triangle reads as an alarm | W/settings/ui/Callout.svelte | ✅ merge with networks #13? |

### language — Languages and readability (12)

> Notes are English only, for native English readers (spec 101 D3), and carry no Chinese. #8 is
> about Chinese button wording and is cut unless it can be told without the characters;
> send-receive #17 (how a Chinese name is filed under A) works as a story about sorting names.

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why Russian plurals were wrong on every phone | "21 получателей"; tests ran on full-ICU Node | S/004; core/i18n/plural.rs | 📜 |
| 2 | Every word costs bytes | 990,499 bytes to read one language; a budget raised only by dated owner approval | rust/crates/vela-core/tests/i18n_residency.rs | ✅ |
| 3 | Why Russian was once set in Times New Roman | "−Без лимита" on the signing sheet | commit ea3f22d19 | ✅ |
| 4 | No web font for Chinese, Japanese or Korean | Japanese breaks at phrases, not inside イーサリアム | commit ccb04e1b9 | 🗣 the wallet does download Noto Sans SC |
| 5 | The mockups were drawn in Chinese, the short language | "Follow Syst…" | commit 0871a2e4a | ✅ |
| 6 | A label never ends in "…" | Transaktionsgeschwindigkeit → Transaktionstempo | W/settings/ui/SegmentedControl.svelte | ✅ |
| 7 | Why the welcome headline has three sizes | Chinese 6.9 em, Indonesian 15.0 em | W/tokens/tokens.css 115–117 | ✅ |
| 8 | 收款 and 转账, not 接收 and 发送 | File-transfer words vs payment words | docs/DESIGN-REVIEW-2026-07.md | ✅ |
| 9 | Traditional Chinese falls back to English, never to Simplified | | core/i18n/resolve.rs 5–17 | ✅ |
| 10 | "A translated brand is a wrong brand" | 1Password stays 1Password in 15 languages | core/passkey.rs | ✅ |
| 11 | Six text sizes, applied before the first pixel | A theme flip "reads as a glitch rather than a setting" | app-web/vela-wallet/src/app.html 21–50 | ✅ |
| 12 | The text-size slider waits for your finger to lift | | W/settings/ui/TextScaleSlider.svelte | ✅ |

---

## Part 6 — Behind it

### self-hosting — Running it yourself (7)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why getvela.app is the one thing you can't self-host | Passkeys belong to the domain | ledger C-rp-1, C-access-1 | ✅ |
| 2 | Why the wallet checks a service's name before trusting it | `/api/health` must name the right service | site docs/self-hosting | ✅ |
| 3 | The week "every service is replaceable" wasn't true | The relay hard-coded Vela's chain directory until v0.9.6 | S/080 §6a; ledger C-selfhost-2 | 📜 |
| 4 | Why the chain directory decides what the relay trusts | ~2,600 networks; its stables list decides who can pay fees | relay README | ✅ |
| 5 | Why the exchange-rate service is tiny | 84 upstream sources → the ~30 the wallet needs | vela-currency README | ✅ |
| 6 | Why there's no .env and no remote switch | A remote config service would give a server power over every wallet | docs/project-takeover/13 ADR-005 | 🗣 still true on all four? |
| 7 | A phone app you build yourself is the same wallet, with one door closed | | docs/ARCHITECTURE.md; S/063 §4 | ✅ |

### engineering — How it's built (15)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why one Rust core runs under four different apps | Keccak written twice; the Safe address in three places | docs/ARCHITECTURE.md; S/001 | ✅ |
| 2 | From "hand-write every primitive" to "hand-roll nothing" | ADR-001 reversed | ADR-001; S/001 FR-005 | 📜 |
| 3 | The core decides; the apps only do | "Register a passkey" is a sentence, not a ceremony | core/app/mod.rs, shell.rs | ✅ |
| 4 | Forty state cells and a reject that still sent the transaction | BUG-2 | S/016; docs/KNOWN-BUGS.md | 📜 |
| 5 | "22 of 22 integrated", and the audit that said otherwise | Count call sites, not files | commit 52dbddbfc | 📜🗣 |
| 6 | Three bridges from one crate | crate-type can't vary per target | S/001 research D2, D5 | ✅ |
| 7 | Why we never shipped a React Native binding | | S/001 Amendment | 📜 |
| 8 | How big is the wallet's brain? | 2,930,927 bytes base64 → a fingerprinted file; cap moves only by measured, approved steps | rust/scripts/build-web.mjs | ✅ |
| 9 | Why the desktop app is Rust all the way down | Web needed 311 generated wire types; desktop none | S/007; S/030 | ✅ |
| 10 | Porting to Rust created a bug TypeScript couldn't have | u128 clamp: a 126 DAI fee quoted as one cent | commit 4a2fc3e6f | 🗣 did it reach users? |
| 11 | Why we told the Rust linter "no" on money checks | `!(x > 0.0)` is the only spelling where NaN fails | `#[allow(neg_cmp_op_on_partial_ord)]` in core | ✅ |
| 12 | 961 files that could no longer run on a phone | Hermes has no WebAssembly | S/039 | 📜 |
| 13 | Delete first: retiring the old app before the new one was live | 「客户端缓存和数据可以丢失」 | S/039 | 📜 |
| 14 | One source, one generator, one CI gate | Four apps are only worth having if they can't drift | docs/ARCHITECTURE.md | ✅ |
| 15 | Getting a Rust app into the Mac App Store | gpui's private window calls, vendored and cleaned | S/095 | ✅ |

### practice — How one person ships four apps (13)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | 100 specs in ten weeks | Spec 001 on 2026-07-28, spec 100 on 2026-10-04 | specs/; git log | ✅ |
| 2 | AI writes the code; a person owns it | "Prove this code is unsafe until you can't" | docs/agent-rules/ | 🗣 how openly |
| 3 | "The owner should not be the diff" | Desktop showed 16 items where web showed 23 | S/078 | ✅ |
| 4 | The web is the spec | One report turned out to be a core bug in all four apps | docs/PARITY-2026-09-21.md | ✅ |
| 5 | A sentence the iPhone can't say | A key Android uses and iOS never mentions | S/058 | ✅ |
| 6 | 9,695 lines of screens nobody could open | A rebase dropped the wiring; CI never built Android or iOS | S/029 | 📜 |
| 7 | The setting that never saved while every check was green | iOS sent `id` where the core wanted `field` | S/081 results T003 | 📜 |
| 8 | The compiler crash that only appeared when archiving | Swift 6.2.4; fixed with one word | S/058 results | ✅ |
| 9 | The parallel space: a real wallet where only the key is fake | | docs/PARALLEL-SPACE.md | ✅ |
| 10 | Was this binary built from this source? | | rust/scripts/build-web.mjs; check-ios-core-fresh.sh | ✅ |
| 11 | Why native fixes have to be seen on a real phone | A share sheet called its completion handler twice (#449) | commit b21891cb1 | ✅ |
| 12 | iOS CI went from 35 to 8 minutes, and the speedups we threw away | 1,263 tests run in 13 s; a probe suite waited 427 s | PRs #454, #457 | ✅ |
| 13 | The tests that had quietly stopped testing | | memory notes | 🗣 anchors |

### releases — Releases and downloads (10)

| # | Title | The story, and its hook | Sources | Status |
|---|---|---|---|---|
| 1 | Why the phone apps are never on GitHub Releases | | S/063 | ✅ |
| 2 | In the first week, three of six downloads couldn't be installed | Gatekeeper called the Mac image "damaged" | S/063 §1–2 | 📜 |
| 3 | Why the Windows installer stays unsigned | A certificate removes a warning without making the file more genuine | README; docs/ARCHITECTURE.md | ✅ |
| 4 | A checksum says two files match, not who made them | `gh attestation verify` | docs/RELEASING.md | ✅ |
| 5 | Pushing a branch is the release | `cannot lock ref 'refs/heads/release'` | S/064 | ✅ |
| 6 | What the About screen used to say | `v1.0.0 (6ab8f)`, copied from a design mock | S/064 §1 | 📜 |
| 7 | A version number that says when | | S/066 | 🗣 the 0.9 line continued |
| 8 | Why 26.9.0 and never 26.09.0 | cargo rejects it; rpm treats them as equal | S/066 §3–4 | ✅ |
| 9 | The download button that doesn't send you to GitHub | 14 files named by CPU; R2 because GitHub is slow from mainland China | S/065 | ✅ |
| 10 | Why every release builds everything | 0.9.5's Windows installer broke on Windows-only code | docs/RELEASING.md | ✅ |

---

## Reserves (verified, not yet placed)

Page dots aren't buttons · one orange thing per intro drawing · Settings rows capped at 560 px ·
dark mode's "sunken" colour was lighter than the page · the scanner frame is four corners · the
address appears only once the wallet exists · right-to-left name protection built but off · the A–Z
contacts index · a dApp's `value` field read one way everywhere (`core/tx_request.rs`) · the dead
"rapid" tier · the estimation dummy call to `0x…04` · fee pre-warm after 5–6 s measured · the
extension keeps JS copies of Rust rules (S/089).

## Found while harvesting: status (2026-10-09, branch 101-site-notes)

Fixed on this branch (English and Chinese; the other 13 locales show stale in `i18n:status`):

- `networks-and-fees.md`: every app lets you set a per-network RPC; Android uses the pool.
- `recovery.md`, `self-hosting.md`: the custody sentence now has C-custody-1's second half.
- `create-wallet.md` step 5: a one-key wallet opens once the registry accepts its record (#409).
- `blog/vela-is-in-alpha.md`: the public-key index is MIT (p256-index#7).
- Privacy page: the relay never submits through your RPC (relay `docs/rpc.md`); date bumped.
- `install.md`: attestation is made by the release workflow; Mac images are attested afterwards
  by `macos-attest.yml`.
- `seoConfig.repoAppDir` → `app-web/getvela.app` (every docs edit link was a 404).

Left for the founder:

- The roadmap still lists "every app honouring your own services" as upcoming. Removing an item
  changes the array length, so all 14 translations would fall back to English until retranslated.
- `self-hosting.md` tells Android builders to run `installDebug`, a developer build with the
  debug-mode switch. Which release command to recommend instead?
- In-app copy (15-locale corpus): Sign Out's "your passkey stays in Face ID / fingerprint"; Erase's
  "Delete everything"; "RPC, Explorer & Bundler URLs"; the iOS/Android Fiat Rates hint; "Lowest
  fee, if you can wait"; Android's dead "Open Chain Setup Tool"; desktop's dead "Report it".
- iPhone security keys: `signers.md`, `install.md`, `self-hosting.md` and ARCHITECTURE promise
  them, but the smart-card entitlement they need is dropped from every signed iOS build
  (`VelaWallet.entitlements:40-46`, spec 088 audit I10). The docs also say "Lightning"; the code
  says USB-C only. `install.md` gives both "iOS 16+" and "17.4".
- `signers.md` and `create-wallet.md` describe three ways to create a wallet; the desktop and phone
  apps offer four (the Trusted Signer).
- Hold both Trusted Signer notes (`keys/four-ways-to-sign-in`, `trusted-signer/trusted-signer-vs-in-app`)
  until spec 102 (signing venue, per-account, another session 2026-10-09) settles, and until the old
  `/b/<hash>/` signer pages with the callback hole fixed in PR #484 are removed from hosting. Their
  framing already matches spec 102: a venue, not a fourth kind of key.
- The Trusted Signer's protection assumes a person refuses a passkey prompt that comes without the
  page (the app can still reach the same passkeys). Spec 075 says a compromised wallet "cannot
  sign by itself"; the trusted-signer note says the habit plainly. Confirm the wording.
- Found writing batch 2 (2026-10-09):
  - Android hides only the total when you hide balances; asset values and activity amounts stay
    visible (`WalletLive.kt` L222, L519). Every other app hides all of them.
  - "Back up public keys to Ethereum" says only public keys are published (`settingsModals.json`
    L99); it replays the whole registration, wallet name and key labels included.
  - Adding a network that lacks the P-256 precompile says "Some required contracts are not yet
    deployed… Use the Chain Setup tool" (`settings/live.ts` L331–345); that tool can't help there.
  - The public-key index has three names: "sync server" (sign-out warning), "PASSKEY INDEX"
    (Settings), "public-key index" (docs).
  - No app lets you label keys 2–7; they're published as "Key 2", "Key 3"… The core supports a
    rename event that nothing sends.
  - Currency decimals and symbol position follow CLDR only on desktop; web and phones always show two
    decimals with the symbol in front.
  - The "green check for a starred contact" exists only as a core field; no app draws it, and the
    `contacts.rs` header and Android `ContactsWire.kt` still describe it.
  - Android has 13 Chainlink fiat feeds, web and iOS 16. `self-hosting.md` L37 gives the fallback
    order backwards (Chainlink is first).
  - **Possible issue:** on iOS and Android a second send started while the first is pending reads
    the same nonce from the chain; vela-relay keys operations by hash and never sends the
    `[existingHash:…]` marker the wallet's `NonceHeld` path expects (relay `execution.rs`
    L2436–2458), so both are signed and accepted and one is dropped later. Web reuses a cached next
    nonce for 10 s; desktop waits for the previous op.
  - Ledger C-fee-1 / `networks-and-fees.md` "$0.01 minimum" is imprecise for fees in the network's
    coin: the floor is the larger of $0.01 and 0.00001 of the coin (ETH networks sit above $0.01).
  - Switching the fee coin recalculates locally from the existing gas estimate until the next
    requote, so a stablecoin fee is briefly priced on the lighter native-coin operation
    (`fee_policy.rs` ~L3794 vs L18–23). Relay `docs/fees.md` §4's "the asset's USD price" wording
    doesn't match the code either (stablecoin counted at $1).
  - **Possible bug (`login.rs`):** when the index and both chains fail, sign-in still offers the
    two-signature rebuild, which always makes a one-key wallet. For a key from a multi-key wallet it
    opens a different address and tries to publish it, with no warning in the prompt.
  - Ledger C-reg-1 omits that each registry entry also stores the key's attachment and transports,
    unsigned. The registry note says so; the ledger and privacy page may want it too.
  - Multi-key wallets on a network without Safe's passkey signer factory: comments, chain-setup and
    the app's `singleKeyOnly` message say the wallet "cannot be deployed". By Safe's MultiSend
    semantics key 1 can probably still deploy and sign; keys 2–7 can't until the factory exists.
    Untested on a chain.
  - `safe.rs` still holds constants for a Vela-written fee-splitter contract the relay no longer
    uses; harmless to C-acct-1 but an auditor will ask.
  - iPhone security keys: the smart-card entitlement is dropped from every signed build, Debug on
    a device included (`VelaWallet.entitlements` L30–46, spec 088 I10), so the route is probably
    inert. The phones note now says it isn't confirmed.
- Stale code comments: `core/trusted_signer/integrity.rs` header, `core/app/mod.rs` header,
  `vela-core-wasm/src/lib.rs`, `AllowanceEditor.svelte` header, `docs/design-system.md` haptics.
