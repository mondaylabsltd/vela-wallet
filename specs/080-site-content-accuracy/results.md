# Results — 080 (draft, completed at the end of implementation)

## What changed

- **Every English and Chinese page** of getvela.app re-verified against the code
  and rewritten where it was wrong: 17 docs (one new — the self-hosting guide),
  landing, get-started, about, roadmap, privacy, terms, blog (dated corrections).
- **Findings**: 10 Fatal, 11 High, and the Medium/Low set in
  [audit-report.md](audit-report.md), with the product gaps that the docs now
  disclose instead of hiding.
- **Drift guards**: translated docs carry `source:` fingerprints and
  `i18n:status --gate` fails on stale ones; tests pin the landing network list to
  vela-core, every locale's network heading, locale-prefixed links in translated
  docs, and the anchors other pages link to.
- **Findability**: docs home opens with a twelve-question "Find an answer" table;
  sidebar regrouped (Keys & recovery; Run it yourself); every doc ends with a next
  step; the docs chrome is translated.
- **Security fix**: `/api/transactions` could leak the server's Alchemy key.
- **README and authority docs** (fact bank, F01, store listing draft) corrected so
  the next content pass can't reinstate the retired facts.

## SC-004 — questions answered within two clicks of /docs

Every question is one click from the docs home ("Find an answer"), and its
target page answers it in a named section:

| Question | Click 1 (from /docs) | Section |
| --- | --- | --- |
| Lost phone / deleted passkey | Recovery & sign-in | "Signing in on a new device", "The honest limits" |
| Add or change keys | Signers & security keys | "Why they are chosen at creation" |
| Why this fee | Networks & fees | "What the fee is" |
| Per-network deposit | Networks & fees | "When a relay runs out of gas" (no deposit exists) |
| Is my chain supported / add one | Networks & fees · Chain setup | "Built-in networks", "Adding another network" |
| getvela.app offline | Self-hosting guide → `#if-getvela-app-disappears` | "The one thing you cannot replace" |
| Run my own services | Self-hosting guide | "Run your own relay/index/…" |
| Connect to a dApp | Install → `#dapps` | "Using Vela with dApps" |
| Which platform, what it costs | Install · Get Vela | table at top |
| Is it audited | Audits & known issues | whole page |
| What data is public | Create your wallet → `#what-is-public` · Privacy | "What is public" |
| Check what I'm signing | Clear signing · The Bybit attack | whole pages |

## Verification

(filled in at the end)

## Self-hosting guide walk

(filled in at the end)

## Open items for the founder

See audit-report.md "For the founder" and "Not fixed here".
