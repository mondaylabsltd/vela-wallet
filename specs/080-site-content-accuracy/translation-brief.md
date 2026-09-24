# Translation brief — re-aligning one locale to the finalized English + Chinese

You are the native-quality localizer and reviewer for ONE locale of getvela.app.
Work only on that locale's files. Worktree: `/Volumes/data/production/vela-wallet-080`;
site: `app-web/getvela.app` (below: `G/`).

## The reference

English and Simplified Chinese have been finalized **together** and are the
reference (founder, 2026-09-22). Read both for every page. They agree in meaning;
where their wording differs, take the meaning they share. Neither is a template
to copy word for word. The existing text in your locale was translated from an
**older** English that has since changed a lot — facts were corrected, sections
added, pages rewritten. Do not use it to "correct" the reference; reuse its good
terminology and phrasing only where the meaning still matches.

Facts are fixed by `specs/080-site-content-accuracy/claim-ledger.md` (read it;
the glossary is at the bottom). Qualifiers are facts: "up to", "by default",
"usually", "not yet", "where available", "any one of" must survive.

## The method (single-string i18n locale review, applied to whole pages)

For every string and paragraph, check and fix:
1. **Accuracy** — the reference meaning, nothing missing, added, softened or inverted.
2. **Naturalness** — reads as written by a native product team in this market;
   no literal translation, no English/Chinese sentence shapes; UI terms as local
   apps and OS settings actually say them.
3. **Cultural and tone risk** — no condescension, no hype, no overly imperative
   tone; the site's voice is plain, specific and calm.
4. **Consistency** — the same term for the same concept everywhere in this
   locale (key, passkey, security key, relay, registry, self-hosting, signing
   page); follow the locale's established term for "passkey" (zh-TW 密碼金鑰,
   zh-HK 通行密鑰, ja パスキー, ko 패스키, tr geçiş anahtarı; others "passkey").
   Formality: keep the register this locale already uses (see
   `specs/059-getvela-site-i18n/reviews/single-string.md`).
5. **Technical integrity** — see Invariants.

Regional variants are their own markets: zh-TW and zh-HK are not copies of zh or
of each other (zh-HK is written Standard Chinese for Hong Kong readers, with HK
vocabulary; not spoken Cantonese unless the file already uses it consistently);
es-MX and pt-BR use Mexican / Brazilian usage.

## What to change

1. **Catalog** `G/src/lib/i18n/messages/<tag>.json` — re-translate every key whose
   English changed in spec 080 (compare with `git diff dd482131 -- app-web/getvela.app/src/lib/i18n/messages/en.ts`
   or read `en.ts` and `zh.json` side by side):
   - `home`: `meta.description`, `meta.ogDescription`, `meta.organization`,
     `hero.facts[2].link`, `hero.facts[3].term` and `.link`, `why.p1`,
     all of `tradeoffs.items`, all of `compare.rows` (now **13** rows: a new
     "Adding a key later" row after "Losing one key", and "Open source" became
     "Source code"), `pricing.cards`, `networks.heading`, `networks.body`,
     `seal.label`, `seal.verify`, `faq.items[0].a`, `faq.items[1].a`,
     `faq.items[4].a`, `faq.items[5].a`, `faq.items[6].a`;
   - `about`: `meta.description`, `lede`, `team.bio`, `values[0..2].body`;
   - `roadmap`: `upcoming` and `shipped` are **new arrays** (5 and 10 items) —
     replace them entirely;
   - `getStarted`: `meta.description`, `lede`, `platforms.web.blurb`,
     `platforms.desktop.stores`, `fundingNote`;
   - `chrome.docs.groups.keys`, `chrome.docs.groups.selfHost`,
     `chrome.docs.titles["self-hosting"]`, `chrome.footer.links.selfHosting`, and all of `chrome.docs.ui` —
     already filled with a draft; review and improve them.
   Keep every other key as it is unless you find a real defect in it (then fix it
   and record it). Arrays must have exactly the English length.
2. **Docs** `G/src/content/docs/<tag>/<slug>.md` — all 17 slugs:
   introduction, install, create-wallet, why-vela, send-and-receive,
   networks-and-fees, passkeys, signers, recovery, clear-signing, bybit-attack,
   account-contract, security-audits, self-hosting (NEW — create it),
   clear-signing-self-host, whitepaper, faq. Rewrite each from the current
   `G/src/content/docs/<slug>.md` and `G/src/content/docs/zh/<slug>.md`.
3. **Review record** `specs/080-site-content-accuracy/reviews/<tag>.md` — see format below.

## Invariants (tests enforce most of these)

- Front matter: `title` and `description`; **quote the description with double
  quotes** (colons break YAML otherwise). Do not add a `source:` line (it is
  stamped centrally afterwards).
- Keep `<script>` import blocks, `<Callout type=… title=…>` components (translate
  the `title` attribute and body), code blocks (never translate commands,
  variable names, addresses, file paths), tables (same number of columns).
- Keep every `<span id="…"></span>` anchor exactly as in the English, in the same
  section (self-hosting: if-getvela-app-disappears, relay, index, chain-data,
  exchange-rates, web-app; install: dapps; create-wallet: what-is-public).
- **Links**: every internal link to a localized page must carry your prefix —
  `/<tag>/docs/…`, `/<tag>/chain-setup`, `/<tag>/get-started`, `/<tag>/roadmap`,
  `/<tag>/about`, and `/<tag>` for home. Links to `/blog…`, `/privacy`, `/terms`,
  `/registry` stay **unprefixed** (English-only pages). External links unchanged.
  Anchors after `#` unchanged.
- Catalog values: keep every `<a href>`, `<strong>`, `<em>` exactly as in the
  English value (same hrefs, same tag count); translate the link text only.
  Internal hrefs in the catalog stay English-relative (`/docs/…`, `/chain-setup`)
  — the site localizes them. Keep paragraph breaks (`\n\n`) the same count.
- Numbers and names that are facts: 24 networks, seven keys, contract names and
  addresses, EIP/ERC numbers, product names (Vela, Safe, MetaMask, Base Account,
  Coinbase Wallet, Chrome, YubiKey, GitHub…).
- Never write that an audit is planned or coming.

## Check your work

From `G/`:

```sh
bunx vitest run src/lib/content/docs.test.ts src/lib/i18n src/lib/networks.test.ts 2>&1 | grep -E "<tag>|Tests "
```

Every test naming your locale must pass. Do not edit other locales, English,
Chinese, tests or scripts; do not run `i18n:stamp`; do not commit.

## Review record format (`reviews/<tag>.md`)

```markdown
# <tag> — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: <agent> · Method: single-string locale review, whole pages

## Terminology and register
(the terms chosen for key, passkey, security key, relay, registry, self-hosting,
signing page; the register; anything departing from the 059 choices, and why)

## Findings
| # | File / key | Before (old translation) | Type | Severity | Why | After |
(type: mistranslation / unnatural / cultural risk / terminology / UI fit / technical;
severity High / Medium / Low. List every High and Medium you found in the OLD
text and fixed, and the notable Low ones.)

## Open items
(anything you could not decide reliably without more product context — leave
the text conservative and say so here; "none" if none)

## Result
reviewed — no open High or Medium findings
```
