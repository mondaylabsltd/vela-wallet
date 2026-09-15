---
description: "Task list for 059 — getvela.app in fifteen languages"
---

# Tasks: getvela.app in fifteen languages, server-rendered

**Input**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: included, and not optional here — FR-007, FR-021, FR-025, FR-026 and
FR-031 each *are* a check. US5 is a user story about the checks themselves.

**Paths**: everything is under `app-web/getvela.app/` unless stated. Commands run
from that directory (`bun`, not npm).

**US1 (the tagline) is already delivered** — commits `4d42f419`, `eade0f56`,
`88490d59`. Nothing below re-does it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable — different file, no dependency on an incomplete task
- **[Story]**: US2 (routing/render) · US3 (docs + fallback) · US4 (SEO) · US5 (quality gate)

---

## Phase 1: Setup — the locale table and the store skeleton (plan stage A)

**Purpose**: the vocabulary everything else is written in. No routing yet, so the
site is untouched and everything here is independently reviewable.

- [X] T001 [P] Create `src/lib/i18n/locales.ts` — the 15 tags, `Locale` type, endonyms, `og:locale` values, `dir`, and the alias table, per [data-model.md](./data-model.md) §Locale
- [X] T002 [P] Create `src/lib/i18n/locales.test.ts` — assert the site's tag list equals `SUPPORTED_LOCALES` **read from `app-web/vela-wallet/src/lib/i18n/locales.ts` at test time**, not a copied array (FR-007)
- [X] T003 [P] Add `toLocale()` / alias resolution / `Accept-Language`-style negotiation helpers to `src/lib/i18n/locales.ts`, with cases for `pt`, `es`, `zh-CN`, `zh-Hant`, `zh-SG`, `fr-CA`, and unknown tags, tested in `src/lib/i18n/locales.test.ts`
- [X] T004 Create `src/lib/i18n/messages/en.ts` with the `chrome` and `notice` namespaces only (nav labels, switcher label, footer, CTA labels, fallback notice, English-only badge) — the type every later reader is checked against
- [X] T005 [P] Create `src/lib/i18n/messages/review.json` with all 14 non-English locales at `{"state":"drafted"}` and `en` at `{"state":"source"}`
- [X] T006 [P] Create `src/lib/i18n/messages/zh.json` translating `chrome` + `notice`, using the founder-approved terminology in [approved-copy.md](./approved-copy.md)

**Checkpoint**: `bun run check` passes; `bun run test:unit --run` runs T002/T003.
The site still behaves exactly as it does today.

---

## Phase 2: Foundational — the route subtree (plan stage A) ⚠️ BLOCKING

**Purpose**: English keeps every URL it has while 14 prefixes become real. Nothing
in Phase 3+ can start until this is green.

**⚠️ The riskiest phase in the feature** — it moves every localizable route file
at once (plan §Risks).

- [X] T007 Create `src/params/locale.ts` — a matcher accepting the 14 non-English tags in canonical case **and rejecting `en`** (contracts/routing.md §Must 404)
- [X] T008 `git mv` the localizable routes into `src/routes/[[locale=locale]]/`: `+page.svelte`, `about/`, `roadmap/`, `docs/` — leaving `blog/`, `privacy/`, `terms/`, `registry/`, `demo/`, `api/`, `pay/`, `sitemap.xml/`, `.well-known/` at the root (R5, research §10)
- [X] T009 Create `src/routes/[[locale=locale]]/+layout.ts` — `export const prerender = true`, resolve `params.locale ?? 'en'` into a `Locale`, expose it and its catalog through `load`
- [X] T010 Create `src/routes/[[locale=locale]]/+layout.svelte` — the localized chrome wrapper (switcher slot, notice slot) shared by landing/about/roadmap/docs
- [X] T011 Add `entries` exports so every localized path is prerendered: `[[locale]]/+page.ts`, `about`, `roadmap`, `docs`, and `docs/[...slug]` crossing 15 locales × 11 slugs — returning `{ locale: undefined }` for English (research §2)
- [X] T012 Update `src/routes/[[locale=locale]]/docs/[...slug]/+page.ts` so `entries` yields the (locale, slug) product and `load` resolves both params
- [X] T013 Edit `src/app.html` — replace `lang="en"` with a `%lang%` placeholder
- [X] T014 Edit `src/hooks.server.ts` — add `transformPageChunk` filling `%lang%` from the first path segment (runs during prerender too, research §6), leaving the existing CORS logic untouched
- [X] T015 Edit `src/hooks.server.ts` — add the alias/case 308 redirects from contracts/routing.md §Must 308, and confirm no English path can ever match
- [X] T016 [P] Create `src/routes/[[locale=locale]]/+error.svelte` so an unknown prefix renders the site's 404, not a blank locale shell (FR-010)
- [X] T017 [P] Create `tests/routing.spec.ts` (Playwright) asserting the full status-code matrix in contracts/routing.md — including `/en/` → 404 and `/ja/blog` → 404
- [X] T018 Run the English-untouched curl matrix from [quickstart.md](./quickstart.md) against `bun run preview`; every pre-existing path must be `200` with no redirect (FR-009, SC-005)
- [X] T019 Time `bun run build` with one locale of content present and record the wall time and page count in `results.md` — the number plan §Risks says to take *before* 126,000 words exist

**Checkpoint**: `/` and `/ja/` both render; `/en/` 404s; no English URL moved.

---

## Phase 3 (US2): The site answers in the reader's language (plan stage A) — Priority P1

**Goal**: a reader on any of 15 locale roots gets that locale, server-rendered,
and can move between languages without losing their place.

**Independent test**: `curl /zh/` returns Chinese copy in the HTML body with no
JavaScript; switching language from `/docs/faq` lands on `/zh/docs/faq`.

- [X] T020 [US2] Extract the landing page's English copy from `src/routes/[[locale=locale]]/+page.svelte` into a `home` namespace in `src/lib/i18n/messages/en.ts`, leaving markup, SVGs and the on-chain counter logic in the component
- [X] T021 [P] [US2] Extract `about` copy into the `about` namespace in `src/lib/i18n/messages/en.ts`
- [X] T022 [P] [US2] Extract `roadmap` copy into the `roadmap` namespace in `src/lib/i18n/messages/en.ts`
- [X] T023 [US2] Create `src/lib/i18n/resolve.ts` — `catalog(L)`, `state(P, L)`, `pathFor(P, L)`, `switchTo(L, path)` per [contracts/translation-store.md](./contracts/translation-store.md)
- [X] T024 [US2] Create `src/lib/i18n/resolve.test.ts` — page-granular state (a namespace missing one key ⇒ `fallback`, never a half-translated page, SC-006) and `switchTo` round-trips including `/docs` ↔ `/<seg>/docs`
- [X] T025 [US2] Create `src/lib/components/LanguageSwitcher.svelte` — all 15 endonyms, preserves the current path, marks the active locale
- [X] T026 [US2] Wire the switcher into **both** headers: `src/lib/components/SiteHeader.svelte` and the landing page's own inline `<nav>` (they are separate — the landing page does not use SiteHeader)
- [X] T027 [US2] Create `src/lib/components/LocaleOfferBanner.svelte` — client-side `navigator.languages` offer, written in the offered language, dismissal in `localStorage`, **never navigates** (FR-013, research §7)
- [X] T028 [P] [US2] Add the banner's strings to every locale in `messages/*.json` — it is shown to a reader who cannot read the page, so it is the one string that cannot fall back
- [X] T029 [US2] Translate the `home`, `about`, `roadmap` namespaces into `src/lib/i18n/messages/zh.json`, using [approved-copy.md](./approved-copy.md) verbatim for the tagline and subtitle
- [X] T030 [P] [US2] Create `tests/locale-render.spec.ts` (Playwright, `javaScriptEnabled: false`) — `/zh/` carries Chinese copy in the response body and `<html lang="zh">` (FR-011, FR-014)
- [X] T031 [P] [US2] Add a switcher e2e to `tests/locale-render.spec.ts` — from `/docs/networks-and-fees` to 简体中文 lands on `/zh/docs/networks-and-fees`, never `/zh/` (SC-003)

**Checkpoint**: US2 is demonstrable end to end on the one locale the founder can
read. Deployable as-is.

---

## Phase 4 (US4): A search engine can tell the versions apart (plan stage B) — Priority P2

**Goal**: 15 versions indexed as translations of each other, not duplicates.

**Independent test**: every localized page's alternates are reciprocal, and the
sitemap lists exactly the pages that exist.

- [X] T032 [US4] **Delete the stale hardcoded `og:title` / `twitter:title` / `og:description` / `twitter:description` from `src/app.html`** — a third retired tagline duplicated on every page (research §1, contracts/head-and-sitemap.md invariant 5)
- [X] T033 [US4] Extend `src/lib/components/Seo.svelte` with an `alternates` input emitting `hreflang` links + `x-default`, `og:locale`, `og:locale:alternate`, and `inLanguage` in JSON-LD
- [ ] T034 [US4] Replace the landing page's hand-written `<svelte:head>` in `src/routes/[[locale=locale]]/+page.svelte` with `Seo.svelte` so canonical/alternates come from one place
- [X] T035 [P] [US4] Replace the hand-written `<svelte:head>` in `roadmap/+page.svelte` with `Seo.svelte` for the same reason
- [X] T036 [US4] Emit alternates from `TranslationRecord` only for locales whose state is `translated`/`source` — a falling-back page is advertised by nobody, including itself (FR-018)
- [X] T037 [US4] Update `src/routes/sitemap.xml/+server.ts` — `xmlns:xhtml`, one `<url>` per (page, locale) that exists, `<xhtml:link>` alternates + `x-default`, English-only pages listed once with none
- [X] T038 [P] [US4] Create `src/lib/i18n/hreflang.test.ts` — reciprocity, self-reference, `x-default` is English on every page, and no fallback page is advertised
- [X] T039 [P] [US4] Create `src/lib/i18n/sitemap.test.ts` — every sitemap URL corresponds to a generated page and no generated localized page is missing (FR-025)
- [X] T040 [P] [US4] Assert in `tests/head.spec.ts` that a built page contains exactly one `og:title` — the regression T032 fixes

**Checkpoint**: US4 complete. Safe to let crawlers see the locale prefixes.

---

## Phase 5 (US3): Docs read in the reader's language, and admit when they don't (plan stage C) — Priority P2

**Goal**: the docs — two-thirds of the site's words — localize, with an honest
fallback and working navigation in every locale.

**Independent test**: delete one locale's `faq` translation; the page renders
English under a notice **in that locale**, chrome stays localized, and no page
advertises that translation.

- [X] T041 [US3] Edit `src/lib/content/docs.ts` — glob `/src/content/docs/**/*.md`, key by `(locale, slug)`, and expose `getDoc(locale, slug)` returning the translation or the English source plus a `fallback` flag
- [X] T042 [US3] Edit `src/lib/content/docs.ts` — `getDocEditUrl` must point at the file that actually rendered (`docs/<tag>/<slug>.md` when translated, `docs/<slug>.md` when fallen back), FR-019
- [X] T043 [US3] Edit `src/lib/content/sidebar.ts` — per-locale group and item titles, with order and slugs staying single-source (research §5)
- [X] T044 [P] [US3] Create `src/lib/components/FallbackNotice.svelte` — one line, in the reader's locale, from `notice.fallback`
- [X] T045 [US3] Render the notice in `src/routes/[[locale=locale]]/docs/[...slug]/+page.svelte` and `docs/+page.svelte` whenever the doc fell back
- [X] T046 [US3] Apply the same page-level fallback to `home`, `about` and `roadmap` so the rule is one rule, not a docs special case (SC-006)
- [X] T047 [P] [US3] Mark English-only destinations in `SiteHeader.svelte` / `SiteFooter.svelte` with an `EN` badge and `hreflang="en"` when the reader is in a non-English locale (FR-020)
- [X] T048 [P] [US3] Create `src/lib/content/docs.test.ts` — `(locale, slug)` resolution, fallback flag, and the edit URL following the rendered file
- [ ] T049 [US3] Translate the 11 docs into `src/content/docs/zh/` and verify `/zh/docs/*` end to end
- [X] T050 [P] [US3] Add `tests/fallback.spec.ts` — with one `zh` doc removed, the page shows Chinese chrome + Chinese notice + English body, and emits no `hreflang="zh"` for it

**Checkpoint**: one locale is complete across the whole evergreen surface. The
machinery is done; what remains is content.

---

## Phase 6 (US3 continued): The other thirteen locales (plan stage D) — Priority P2

**Goal**: 15 locales live. All non-`zh` locales land as `drafted`.

**Order matters**: `chrome`/`notice` first for every locale — a fallback page in a
locale whose chrome is missing would print its "this page is English" notice in
English (data-model §3).

- [X] T051 [US3] Complete `chrome` + `notice` in all 13 remaining `src/lib/i18n/messages/*.json` before any page content
- [X] T052 [P] [US3] Draft `home` + `about` + `roadmap` for `ja`, `ko`, `zh-TW`, `zh-HK` in `src/lib/i18n/messages/*.json`
- [X] T053 [P] [US3] Draft `home` + `about` + `roadmap` for `de`, `fr`, `it`, `es-MX`, `pt-BR` in `src/lib/i18n/messages/*.json`
- [X] T054 [P] [US3] Draft `home` + `about` + `roadmap` for `ru`, `tr`, `vi`, `id` in `src/lib/i18n/messages/*.json`
- [ ] T055 [P] [US3] Draft the 11 docs for `ja`, `ko`, `zh-TW`, `zh-HK` under `src/content/docs/<tag>/`
- [ ] T056 [P] [US3] Draft the 11 docs for `de`, `fr`, `it`, `es-MX`, `pt-BR` under `src/content/docs/<tag>/`
- [ ] T057 [P] [US3] Draft the 11 docs for `ru`, `tr`, `vi`, `id` under `src/content/docs/<tag>/`
- [ ] T058 [US3] Record each locale's tagline decision in [approved-copy.md](./approved-copy.md) — a locale keeps the English headline until its own line is approved (FR-006)
- [ ] T059 [US3] Re-run `bun run build` and compare page count and wall time against the T019 baseline

**Checkpoint**: 210 localized pages live, every locale `drafted`, none claiming to
be reviewed.

---

## Phase 7 (US5): Nobody can quietly ship a half-translated site (plan stage E) — Priority P3

**Goal**: the checks, the report, and the R7 review passes.

**Independent test**: remove one key and edit one English string; the report names
both, and `bun run build` still succeeds.

- [X] T060 [P] [US5] Create `src/lib/i18n/messages.test.ts` — shape (no key absent from `en.ts`), `chrome`/`notice` completeness in all 15 locales
- [X] T061 [P] [US5] Create `src/lib/i18n/placeholders.test.ts` — placeholder, link and inline-markup sets identical to the English value (FR-031)
- [X] T062 [US5] Add English content hashes per namespace/doc so a translation written before an English edit reports as **stale** (FR-026, data-model transitions)
- [X] T063 [US5] Add `bun run i18n:status` to `package.json` printing the per-locale table from contracts/translation-store.md, and assert it is computable in `src/lib/i18n/status.test.ts`
- [X] T064 [US5] Confirm a missing translation never fails the build — add a test that a locale with an incomplete namespace builds and renders fallback (FR-027)
- [X] T065 [P] [US5] Create `src/lib/i18n/honesty.test.ts` — the A02 FR-2/FR-3 claims (no third-party audit, alpha, "architecturally can't") are present and unsoftened in every locale claiming `translated` (FR-032)
- [ ] T066 [US5] Run the R7 review on `zh` per [spec.md](./spec.md) §R7, record findings in `specs/059-getvela-site-i18n/reviews/zh.md`, repair every High and Medium, then set `review.json` to `reviewed`
- [ ] T067 [P] [US5] Run the R7 review for `ja`, `ko`, `zh-TW`, `zh-HK` into `reviews/<tag>.md`
- [ ] T068 [P] [US5] Run the R7 review for `de`, `fr`, `it`, `es-MX`, `pt-BR` into `reviews/<tag>.md`
- [ ] T069 [P] [US5] Run the R7 review for `ru`, `tr`, `vi`, `id` into `reviews/<tag>.md`
- [ ] T070 [US5] Run the single-string review across all 15 locales on the tagline, the subtitle and the primary CTA — the three strings every visitor reads (spec US5 scenario 5)

**Checkpoint**: every locale's state is recorded and earned.

---

## Phase 8: Polish & cross-cutting

- [ ] T071 [P] Re-run the full quickstart matrix from [quickstart.md](./quickstart.md) and record results in `specs/059-getvela-site-i18n/results.md`
- [ ] T072 [P] Crawl the deployed English site before/after and diff the path set — proof for SC-005, not an assertion
- [ ] T073 [P] Verify the landing page's on-chain wallet counter and its RPC-failure state in all 15 locales, including number formatting (SC-009, spec Edge Cases)
- [X] T074 [P] Update `docs/CONTENT-SOURCE-100-CLUES.md` and `docs/requirements/A02-brand-voice-honesty-audit-posture.md` to state that approved copy now exists per locale and where it lives
- [X] T075 [P] Add a short "adding a string / adding a locale" section to `app-web/getvela.app/README.md` — the six-step corpus procedure's equivalent for this site
- [X] T076 Write `specs/059-getvela-site-i18n/results.md` — what shipped, what was verified how, and what is still `drafted`

---

## Dependencies

```
Phase 1 (T001–T006)
   └─▶ Phase 2 (T007–T019)  ⚠️ blocks everything
          ├─▶ Phase 3 US2 (T020–T031)
          │      └─▶ Phase 5 US3 (T041–T050)
          │             └─▶ Phase 6 US3 content (T051–T059)
          │                    └─▶ Phase 7 US5 (T060–T070)
          └─▶ Phase 4 US4 (T032–T040)   ← independent of US3; only needs the routes
```

- **US4 does not wait for US3.** Annotations are a function of the route tree and
  the translation record, both of which exist after Phase 2.
- **T051 gates T052–T057.** Chrome before content, or fallbacks speak the wrong
  language.
- **T066–T070 gate nothing.** A `drafted` locale is shippable (FR-027); review is
  what lets it be *called* done.

## Parallel opportunities

- T001, T002, T003, T005, T006 — different files, no order between them
- T021, T022 — about and roadmap extraction are independent of each other
- T038, T039, T040 — three independent test files
- T052–T057 — four-locale batches, one file each, fully parallel
- T067–T069 — the review passes are per locale and share nothing

## Implementation strategy

**MVP = Phases 1–3.** One locale (`zh`), fully rendered, switchable, English
untouched. It proves every mechanism in the feature and is the only locale the
founder can personally sign off (SC-007). If the feature stopped there it would
still be worth shipping.

Then Phase 4 (make it legible to crawlers) → Phase 5 (the docs, where the words
are) → Phase 6 (breadth) → Phase 7 (earn the word "done").

Phase 2 is the one place to slow down: it moves every localizable route file in
one commit, and T018 — not an eyeball — is what proves the English site survived.
