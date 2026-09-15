# Results — 059

Running record. Each entry is what was actually run, not what was intended.

## Where 059 stands (2026-09-15)

**The machinery is complete and the content is partial, by design.** Every
mechanism the spec asks for is built, tested and shipping; what remains is
words, and the architecture is explicitly built so that missing words are a
legal state rather than a broken page.

| | state |
|---|---|
| Routing, `[[locale=locale]]`, 15 locales, English unmoved | done |
| `<html lang>`, alias 308s, 404s for `/en/` and unknown prefixes | done |
| `hreflang` / `x-default` / canonical / `og:locale` / JSON-LD `inLanguage` | done |
| Sitemap with per-URL alternates, generated from the same record | done |
| Page-granular fallback + notice in the reader's language | done |
| Docs keyed by `(locale, slug)`, edit-link follows the rendered file | done |
| Language switcher, `Accept-Language` offer banner (no redirect) | done |
| `chrome` + `notice` in **all 15 locales** | done |
| Landing page + about + roadmap + get-started in **10 of 15** | partial |
| Docs (15 pages) in any locale but English | **not started** |
| R7 review passes | **not started** — every locale is `drafted` |

`bun run i18n:status` is the authority on the numbers and will stay right after
this file goes stale.

## What is deliberately not done

- **Docs translations.** 15 docs × 14 locales is roughly 120,000 words. Nothing
  is blocked by their absence: `/ja/docs/faq` serves the English body under a
  Japanese notice, and no `hreflang` claims a Japanese version exists.
- **R7 reviews.** Every locale is `drafted` and `review.json` says so. Nothing
  rendered to a reader claims otherwise. A locale becomes `reviewed` only when
  `reviews/<tag>.md` exists with no open High or Medium finding.
- **Taglines for the 13 locales other than en/zh.** They are translated in the
  files but were never founder-approved per FR-006; `approved-copy.md` records
  which two are.

## Phase 1 — the locale table (T001–T006)

`src/lib/i18n/locales.ts` + `messages/{en.ts,zh.json}` + `review.json`.

- 27 unit tests, including the parity test that parses
  `app-web/vela-wallet/src/lib/i18n/locales.ts` and `src/lib/settings/fixtures.ts`
  off disk (FR-007).
- The site was byte-identical at this point — nothing imported any of it yet.

## Phase 2 — the route subtree (T007–T019)

### T019 — the build budget, measured before the words arrived

| | value |
|---|---|
| `bun run build` wall time | ~5.3 s |
| prerendered HTML files | 216 → **291** after `/get-started` and the docs grew |
| of which localized | 14 pages × 15 locales, plus the English-only blog and legal |

Shiki compiles each markdown file once, not once per page, so fifteen locales of
the same English docs cost almost nothing. **That is the number that will move**
when docs translations land: 15 docs × 14 locales is 210 new markdown files to
highlight. Re-measure then; do not assume this figure holds.

### T018 — English is untouched (FR-009, SC-005)

Against `wrangler dev`, every pre-existing path returns `200` with no redirect:

```
/  /docs  /docs/faq  /docs/whitepaper  /about  /roadmap
/blog  /blog/hello-world  /privacy  /terms  /registry
/sitemap.xml  /blog/rss.xml
```

`/docs/`, `/about/`, `/blog/` still 307 to their unslashed form — pre-existing
SvelteKit trailing-slash behaviour, verified unchanged, not something 059 added.

### T017 — the URL contract, as tests

`bun run test:e2e` — 56 passing. All four tables in contracts/routing.md:
English untouched, 14 locale roots rendering with the right `<html lang>`, the
404 set (`/en`, `/xx/`, `/ja/blog`, `/ja/privacy`…), and the 308 set including
query-string preservation.

Two pre-existing config defects fixed to get there: `playwright.config.ts` waited
on port 4173, which nothing in this project serves, so the e2e suite could never
have run; and it now uses its own port 8788, because 4173 is shared with
`app-web/vela-wallet`'s suite and two sessions on one port silently test each
other's build.

## Deviations from tasks.md

- **T016** creates `src/routes/+error.svelte`, not one inside the locale subtree.
  An unknown prefix matches *no* route, so the subtree's layout never runs and an
  error page inside it would never render.
- **T017** lives at `e2e/routing.e2e.ts`, not `tests/routing.spec.ts` — the
  project's Playwright `testMatch` is `**/*.e2e.ts`.
- **T034 skipped on purpose.** The landing page's hand-written `<svelte:head>`
  already emits canonical, alternates, `x-default` and `og:locale` correctly.
  Moving it into `Seo.svelte` is a refactor with no reader-visible change, on the
  one file the founder asked not to disturb. `/roadmap` (T035) *was* moved,
  because it was missing canonical and alternates entirely.
- **T062** is `bun run i18n:stamp` + the `stale` column, rather than hashes
  embedded in the page. Same guarantee, and it survives a copy edit.

## Incidents

### A moved file came back

`bun run build` began failing with `The "/" and "/[[locale=locale]]" routes
conflict`. `src/routes/+page.svelte` had been `git mv`d into the locale subtree
while open and dirty in the founder's editor; the next save recreated it at the
old path. The resurrected copy carried the founder's own in-progress hero edit,
which was merged into the moved file rather than discarded.

**Standing hazard**: an editor holding the old path breaks the build on its next
save. The landing page lives at `src/routes/[[locale=locale]]/+page.svelte`.

### The English moved under the translations — twice, then continuously

Mid-translation, two hero facts were reworded; later the whole trade-offs block
was rewritten three times, and a false gas claim ("roughly 1.5–3× a plain
transfer") was removed from the trade-offs while surviving one section down in
the FAQ, in English and four locales.

Two things came out of this:

1. **`bun run i18n:stamp` / the `stale` column exist because of it.** A
   translation records a fingerprint of the English it was made from; when the
   English changes, `i18n:status` says so. Before this, drift was invisible —
   the files were structurally perfect and said something the page no longer
   said.
2. **Two sessions were editing these files at once.** A parallel session was
   revising the English copy (and re-syncing zh/ja/zh-TW/de/ko) while this one
   was adding locales. That is why translation stopped at ten: pouring
   translations into a source that is being rewritten produces stale files
   faster than it produces finished ones. The remaining four (vi, id, tr, zh-HK)
   are deliberately left until the English settles.

### A phantom test failure

Rewriting the JSON files and running vitest in the same breath reports href
mismatches that are not there — `import.meta.glob` in `resolve.ts` serves the
previous module graph. A second run is clean; an independent walk over the same
files found nothing either time.

## Defects found and fixed along the way

| what | where |
|---|---|
| `app.html` hardcoded `og:title`/`twitter:title` on **every page** — a third, retired tagline duplicating the per-page tags, and the first one wins | fixed, T032, e2e asserts exactly one `og:title` |
| The FAQ repeated a gas claim the trade-offs had just removed for being untrue | fixed in English + de, zh, zh-TW, and the five locales this session owns |
| `/roadmap` emitted no canonical and no `hreflang` at all | fixed, T035 |
| Two Chinese strings landed in `ru.json` during the bulk chrome pass | fixed; `messages.test.ts` now catches that class |
| `playwright.config.ts` waited on a port nothing serves | fixed |

## Where to pick it up

1. `bun run i18n:status` — anything `stale` is a locale saying something the
   English no longer says. Fix those before adding new ones.
2. vi, id, tr, zh-HK need `home`/`about`/`roadmap`/`getStarted`.
3. Docs: `src/content/docs/<tag>/<slug>.md`. Start with `zh` — it is the one
   locale the founder can sign off directly.
4. R7 reviews, per `spec.md` §R7, into `reviews/<tag>.md`. A locale is
   `reviewed` only when its findings file has no open High or Medium.

## Session of 2026-09-15 (evening) — the last four locales, the Chinese docs, and a miss

### T059 — the build, re-measured against the T019 baseline

| | T019 (one locale of content) | now |
|---|---|---|
| `bun run build` wall time | ~5.3 s | **8.51 s** |
| prerendered HTML files | 216 | **306** |
| docs per locale | 11 | 16 |

306 = 15 locales × (16 docs + 4 pages) + 6 English-only (blog index + 5 posts).
The prediction in T019 held: Shiki compiles each markdown file once, so the
sixteen Chinese docs added ~3 s, not ~50. Nothing here is near the ~5 min budget
in plan.md, and adding the remaining thirteen locales of docs should cost on the
order of 10 s more, not minutes.

### Delivered

- **vi, id, tr, zh-HK** — the four locales that had only `chrome` and the notice
  now carry `home`, `about`, `roadmap`, `getStarted`. Fifteen of fifteen.
  `zh-HK` is written Cantonese, and uses 通行密鑰 where `zh-TW` uses 密碼金鑰.
- **T049** — all sixteen docs in `src/content/docs/zh/`. `zh` is the first locale
  at 20/20.
- **T058** — the headline of every locale recorded in approved-copy.md with its
  state; none claims approval it does not have.
- **T066–T070** — R7 reviews in `reviews/`, one file per locale plus the
  single-string pass. `zh` → `reviewed`; the other thirteen stay `drafted` on
  purpose (see below).

### Incident — a stale flag cleared without verifying it

`i18n:status` reported nine locales stale for `home`. This session cleared them
by re-stamping, reasoning that each locale file and `en.ts` were last touched in
the **same commit**. That was true and it was not evidence: the commit in
question had changed one pricing card, while the *signing* section and the three
trade-off bodies had been rewritten several commits earlier and never
translated.

The result was nine locales still telling readers "Sign what you see" — the
claim the founder had explicitly withdrawn as overstated, because a passkey
signs a hash. It is the worst shape this class of bug takes: the stale text is
the *more* appealing version, so nothing about reading the page suggests it is
wrong.

Two things changed because of it:

1. **A gate, not a report.** `messages.test.ts` now asserts that every
   translated string has the same paragraph count as its English source.
   Paragraph structure is the part of a string that survives translation into
   any language, so it catches a rewrite that a fingerprint can only flag once
   and a stamp can silence. Those fourteen tests would have failed all nine.
2. **A rule for the next person.** A stale row is cleared by reading the two
   texts, or by retranslating — never by `i18n:stamp`. Stamping is what you do
   *after* you have made the two agree.

### Why thirteen locales stay `drafted` after an R7 pass

Every High and Medium found is repaired, so the mechanical bar is met. The bar
the spec actually sets is a reader who notices what a competent non-native
cannot, and no such reader has seen them. Marking them `reviewed` would make
`review.json` say something the project cannot back — which is the same failure
as claiming an audit. `zh` is `reviewed` because it had a full-corpus pass and
because the founder reads it and can overrule this file.

### Still open

- **T055–T057** — the sixteen docs in the other thirteen locales. ~12,000 words
  per locale; `zh` is the worked example, including the one trap: markdown
  bodies are not run through the catalog's link localizer, so a translated doc
  must write `/<tag>/docs/...` itself.
- **T071–T073** — the quickstart matrix, the crawl diff for SC-005, and the
  on-chain counter in all fifteen locales.
- **T034** — still deliberately skipped; see the deviations above.

### T071–T073 — the verification pass · 2026-09-15

Run against `bun run preview` (wrangler on 8787) with the branch as committed.

**T071 — the quickstart matrix.** Every English path `200`, no redirect:
`/ /docs /docs/faq /about /roadmap /blog /privacy /terms /registry /get-started`.
`/ja` renders `<html lang="ja">` with Japanese in the response body and no
JavaScript — including the corrected signing heading
(パスキーが署名するのはハッシュであって、画面ではありません). The 404 set
(`/en/`, `/xx/`, `/ja/blog`, `/ja/privacy`) and the 308 set (`/pt`→`/pt-BR`,
`/pt-br/`→`/pt-BR/`, `/zh-CN`→`/zh`, `/zh-Hant`→`/zh-TW`, `/es`→`/es-MX`,
`/fr-CA/about`→`/fr/about`) both behave. `/ja/` → 307 → `/ja` is the
pre-existing SvelteKit trailing-slash behaviour recorded at T018, unchanged.

The fallback notice follows the doc, not the locale: `/zh/docs/faq` carries none,
`/ja/docs/faq` carries one.

**T072 — the path-set diff, measured rather than asserted.** `before` is the
**deployed** site's sitemap (getvela.app, still pre-059); `after` is this
branch's sitemap with the fourteen locale prefixes filtered out.

| | paths |
|---|---|
| before (deployed) | 22 |
| after (this branch, English only) | 28 |
| **disappeared** | **none** |
| added | `/get-started`, `/docs/account-contract`, `/docs/bybit-attack`, `/docs/clear-signing-self-host`, `/docs/signers`, `/docs/why-vela` |

SC-005 holds: no English URL moved or vanished. The six additions are new pages,
not relocations.

**T073 — the on-chain counter in fifteen locales, and a defect it surfaced.**

The counter rendered in all fifteen with the right label, and the RPC-failure
state behaves: with every RPC blocked, `/de` drops the whole block after the
fourth failed poll (`.counter` and `.counter-skeleton` both absent) rather than
leaving a seal with no number.

The defect: `displayCount.toLocaleString()` was called with **no locale**, so the
number was grouped by the *browser's* locale, not the page's. A German reader on
`/de` with a US browser got `1,234` where the rest of the page is German. Fixed
to `toLocaleString(data.locale)`.

Proof, with the browser pinned to `en-US` on every page:

| page | grouped by page | grouped by browser |
|---|---|---|
| `/de`, `/it`, `/pt-BR`, `/tr`, `/id`, `/vi` | `1.234.567` | `1,234,567` |
| `/fr`, `/ru` | `1 234 567` | `1,234,567` |
| `/en`, `/es-MX`, `/ja`, `/ko`, `/zh`, `/zh-TW`, `/zh-HK` | `1,234,567` | `1,234,567` |

Eight of fifteen locales now render a number the browser would have got wrong.
The seven that match are correct by coincidence of convention, not by accident of
code.
