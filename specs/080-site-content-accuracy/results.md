# Results — 080

Branch `080-site-content-accuracy`, worktree `vela-wallet-080`, from `main` at
`dd482131`. Delivered 2026-09-22.

## What changed

- **Every English and Chinese page** of getvela.app re-verified against the code,
  the service repositories and primary external sources, and rewritten where it
  was wrong: 17 docs (the **self-hosting guide** is new), landing, get-started,
  about, roadmap, privacy policy, terms, and dated corrections on 5 blog posts.
- **13 more locales** re-aligned to the finalized en + zh and reviewed with the
  single-string method (records in `reviews/`). ≈370 High and ≈245 Medium
  defects in the old translations fixed.
- **Findings**: 12 Fatal and 21 High in [audit-report.md](audit-report.md) —
  10 + 11 from the source-verification pass, 2 + 10 from an independent
  adversarial review of the rewritten English; plus the product gaps the docs now
  disclose.
- **Security fixes on the site** (product, small):
  - `Permissions-Policy: publickey-credentials-get=(), publickey-credentials-create=()`
    on every getvela.app response (`_headers` for prerendered pages, the hook for
    rendered ones) — passkeys are bound to this domain, so no page or script it
    serves may request a signature.
  - The analytics tag is no longer loaded on `/chain-setup`, which keeps a funded
    key in localStorage.
  - `/api/transactions` no longer puts an unchecked query parameter into the
    Alchemy hostname (API-key leak).
- **Drift guards**: translated docs record the English they came from
  (`source:`), `i18n:status --gate` fails on stale ones; tests pin the landing
  network list to vela-core `BUILTIN_CHAINS` and every locale's heading, keep
  translated-doc links in their locale, and require the anchors other pages link to.
- **Findability**: docs home opens with a twelve-question "Find an answer" table;
  sidebar regrouped (Keys & recovery; Run it yourself); every doc ends with a
  next step; the docs chrome (browse, on this page, edit, pager) is translated.
- **README and authority docs** (`CONTENT-SOURCE-100-CLUES`, F01, the store
  listing draft, `docs/ROADMAP.md`) corrected so the next content pass can't
  reinstate retired facts; the claim ledger is the canonical wording.

## Verification (final state)

| Gate | Result |
| --- | --- |
| `bun run check` | 0 errors, 0 warnings (baseline 0/0) |
| `bunx vitest run` | 829 passed (baseline 514) |
| `bun run i18n:status --gate` | exit 0 — 0 stale, 0 unstamped; 21/22 pages per locale (chain-setup stays English, as before) |
| `bun run build` | exit 0; `self-hosting.html` prerendered in all 15 locales; `_headers` carries the Permissions-Policy |
| Link crawl (quickstart §3) | en + 14 locales: 353 pages, every internal link and `#anchor` resolves |
| Screenshots | landing (1440, 500 px), `/docs`, `/zh/docs` (phone), `/docs/self-hosting`, `/get-started`, `/ja/` (phone) — read; no overflow, no fallback notice on translated pages |
| `npm --prefix scripts run check:expo-residue` | ok |
| ESLint on changed Svelte | 5 `no-navigation-without-resolve` errors in the landing component — identical before and after this work |

## SC-004 — questions answered within two clicks of /docs

Every question is one click from the docs home ("Find an answer"):

| Question | Click 1 | Section |
| --- | --- | --- |
| Lost phone / deleted passkey | Recovery & sign-in | "Signing in on a new device", "The honest limits" |
| Add or change keys; a key compromised | Signers & security keys | "Why they are chosen at creation", "If a key may be compromised" |
| Why this fee | Networks & fees | "What the fee is" |
| Per-network deposit | Networks & fees | "When a relay runs out of gas" (none exists) |
| Is my chain supported / add one | Networks & fees · Chain setup | "Built-in networks", "Adding another network" |
| getvela.app offline | Self-hosting guide `#if-getvela-app-disappears` | "The one thing you cannot replace" |
| Run my own services | Self-hosting guide | relay / index / chain data / rates / apps |
| Connect to a dApp | Install `#dapps` | "Using Vela with dApps" |
| Which platform, what it costs | Install · Get Vela | table at top |
| Is it audited | Audits & known issues | whole page, incl. "Gaps in Vela's own defences" |
| What data is public | Create your wallet `#what-is-public` · Privacy | "What is public" |
| Check what I'm signing | Clear signing · The Bybit attack | whole pages |

## Self-hosting guide walk (SC-005)

| Step | How verified |
| --- | --- |
| Signing page served locally (`python3 -m http.server`) | **run** — pages and manifest served; manifest has `host_permissions: getvela.app` |
| Exchange rates without Docker (`cargo run --release -p vela-currency-server`) | **run** — `/v2/rates?base=USD` returned 30 rates; `/health` → `vela-currency`, ok |
| Extension build (`pnpm install && pnpm build:extension`) | **run** — `extension/dist` produced, as the guide says |
| Chain data (Docker image) | checked against its README (Docker daemon not running here) |
| Relay (Docker / Workers) | checked against its README, `docs/cloudflare.md`, `docs/fees.md` and code; needs Redis + Iggy or Workers Paid and funded treasuries — not run |
| Public-key index | checked against its README, `.env.example` and code (the undocumented `P256_INDEX_DOMAIN_REGISTRY`); needs a funded Gnosis key — not run |
| Endpoint settings per app | checked in code for all four shells (research §3b); not run on devices |
| Native builds | commands copied from the README; not run |

## Decisions made without the founder (all recorded, all reversible)

- Code is the authority; retracted: 12 networks, the gas-account deposit, "any
  Safe interface can drive it", "compromised server yields no signing ability",
  "removes the upgrade primitive", the signing page as a fallback, "all services
  MIT", "unlimited approvals are blocked", "access doesn't depend on Vela".
- The landing hero fact #4 (a 059-approved string) was reworded to what is true.
- The fee is described by its formula. *(Superseded 2026-09-22 by the founder:
  no multiple of the on-chain cost; lead with "shown before you sign, paid to the
  relay, relay replaceable" — see "Follow-up" below.)*
- zh-HK moved to written Hong Kong Chinese, matching the app's zh-HK.
- The 13 AI-reviewed locales stay `drafted` in `review.json` with a pointer to
  their review; `reviewed` is reserved for a native reading.
- Terms: governing law (England and Wales) and the statutory carve-out added;
  legal review recommended.

## Open items for the founder

See audit-report.md "For the founder" and "Not fixed here". The most important:

1. ~~**Fee narrative vs code**~~ — resolved 2026-09-22: the code wins; marketing
   copy corrected.
2. **Product gaps the docs now disclose**: iOS Service Endpoints page not wired;
   web onboarding ignores a custom passkey index; no guard on self-calls (`enableModule`, `addOwner…`); network check omits
   the signer factory; fetched descriptors labelled "verified"; `X-Rpc-Url` leaks
   provider keys to the relay.
3. **Legal review** of the rewritten terms and privacy policy.
4. **Native readings** of the 13 locales (es-MX, vi and others flagged it
   explicitly); app/site wording differences the translators recorded
   (e.g. es-MX "billetera", fr "clé d'accès", zh-TW 通行密鑰 vs 密碼金鑰).
5. **Landing nav**: spec 059 fixed it at two links; a "Docs" link would help
   expert readers. Not changed.

## Follow-up, 2026-09-22 (founder)

| Ask | Done |
| --- | --- |
| Marketing fee copy must follow the code | `why-we-charge.md` + leads #17/#18/#22 corrected (also: no "Apple Pay chip / key never leaves it", web wallet doesn't connect dApps, real licences); G05/requirements/manual-test/takeover notes carry the formula |
| Don't lead with "ten times or more" | every surface now says: exact fee shown before you sign, paid to the relay (Vela's by default), switch relays or run your own; formula stays on the fees page (`#fee`) |
| "Access doesn't depend on Vela staying online" — make it true | vela-relay PR #12: `VELA_RELAY_CHAIN_DIRECTORY_URL` (both shells, default unchanged); site restores the 059 fact #4 and drops every "relay needs a code change" line. **Merge vela-relay#12 before this PR is published** |
| Hero subtitle | "Signing is done on your device. Your passkey's private key never goes to Vela." / 签名在你的设备上完成。通行密钥的私钥绝不会交给 Vela。 (C-sign-1) |
| Fact #3 positive | "What you see is what you sign: Vela decodes the exact transaction before you approve it." — not "a signing channel": the independent signing page is unpublished (C-clear-2) |
| Fact #4 wordy | 059 string restored; link → `/docs/self-hosting#if-getvela-app-disappears` |
