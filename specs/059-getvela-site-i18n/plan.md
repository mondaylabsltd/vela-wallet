# Implementation Plan: getvela.app in fifteen languages, server-rendered

**Branch**: `059-getvela-site-i18n` | **Date**: 2026-09-15 | **Spec**: [spec.md](./spec.md)

**Input**: [spec.md](./spec.md) US2–US5 (US1, the tagline, shipped in commits `4d42f419` / `eade0f56`)

## Summary

Move the evergreen surface — the landing page, eleven docs pages, `/about`,
`/roadmap` — under one optional-param route subtree so English keeps its current
URLs and fourteen locales get a prefix, prerender all 210 resulting pages, and
annotate them so a search engine can tell the versions apart. Copy comes from a
site-local store (never the wallet's wasm corpus), a page is either fully
translated or renders English under a notice in the reader's own language, and a
locale is only called done after a review against the founder's methodology.

The load-bearing insight from research: **`TranslationRecord` is computed once and
feeds three consumers** — what a page renders, which `hreflang` alternates it
emits, and what the sitemap lists. Generating those three from separate lists is
how sites end up advertising a translation that does not exist.

## Technical Context

**Language/Version**: TypeScript 5.9, Svelte 5 (runes), SvelteKit 2.50

**Primary Dependencies**: `@sveltejs/adapter-cloudflare` 7.2, `mdsvex` 0.12, `shiki` 4.2, `satori`/`resvg` (OG images). No new runtime dependency is introduced — no i18n library.

**Storage**: files. `src/lib/i18n/messages/*` (UI copy) and `src/content/docs/<tag>/*.md` (docs). Review state in `messages/review.json`.

**Testing**: `vitest` for every file-level invariant (locale parity, message shape, placeholder integrity, `hreflang` reciprocity, sitemap ↔ page-set); `@playwright/test` for the rendered claims (locale root renders without JS, switcher preserves the path, banner appears once).

**Target Platform**: static assets on Cloudflare, served by the existing Worker; the Worker keeps only `/api/**`, `/pay` and the alias redirects.

**Project Type**: SvelteKit marketing + docs site (`app-web/getvela.app`), single package.

**Performance Goals**: prerendered HTML — no locale is a runtime cost. Build must stay under ~5 min for 210 pages (Shiki is the dominant term, 11 docs × 15).

**Constraints**: zero change to any existing English URL (FR-009); no translation may block an English change from shipping (FR-027); site copy never enters `rust/crates/vela-core/i18n/` (R6).

**Scale/Scope**: 15 locales × 14 pages = 210 prerendered pages; ~9,000 English words to translate per locale (~126,000 total); ~150 UI message keys.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled Spec Kit template — every
principle is still a `[PRINCIPLE_N_NAME]` placeholder — so there are no project
gates to evaluate and none to violate. The standards this plan is actually held
to are the repo's own, and they are checked explicitly:

| standard | source | how this plan meets it |
|---|---|---|
| Brand voice used verbatim; honesty posture (no audit, alpha) never softened | `docs/requirements/A02` FR-1/FR-2/FR-3 | FR-032 + the `docs.honesty` check in [contracts/translation-store.md](./contracts/translation-store.md) |
| Content grounded in source, never asserted from memory | repo practice | [research.md](./research.md) cites a file and line for every claim; the defect in §1 was found that way |
| One implementation, no parallel copies | spec 168/169 | one route subtree, not a duplicated `/[locale]` tree (research §2) |
| The wallet corpus is a shared, capacity-bound artifact | `reference_i18n_corpus_gates` | R6 — the site keeps its own store |

**Post-design re-check**: unchanged. The design adds no runtime dependency, no
second source of truth, and no new deployment surface.

## Project Structure

### Documentation (this feature)

```text
specs/059-getvela-site-i18n/
├── spec.md
├── plan.md                    # this file
├── research.md                # Phase 0
├── data-model.md              # Phase 1
├── quickstart.md              # Phase 1
├── contracts/
│   ├── routing.md             # URLs, status codes, redirects
│   ├── head-and-sitemap.md    # hreflang / canonical / og / sitemap
│   └── translation-store.md   # file layout, resolution, checks
├── reviews/<tag>.md           # R7 findings, one per locale (implementation)
└── tasks.md                   # /speckit-tasks
```

### Source code

```text
app-web/getvela.app/src/
├── params/
│   └── locale.ts                    # NEW — matcher: the 14 tags, rejects `en`
├── hooks.server.ts                  # EDIT — add transformPageChunk (%lang%) + alias 308s
├── app.html                         # EDIT — lang="%lang%"; DELETE the stale hardcoded og/twitter title
├── lib/i18n/                        # NEW
│   ├── locales.ts                   #   tags, endonyms, og locales, aliases, negotiation
│   ├── messages/en.ts               #   source of truth + the type
│   ├── messages/<tag>.json          #   14 translations
│   ├── messages/review.json         #   per-locale review state
│   ├── resolve.ts                   #   catalog(), state(), path↔locale helpers
│   └── *.test.ts                    #   parity / shape / placeholders / chrome-complete
├── lib/content/
│   ├── docs.ts                      # EDIT — key by (locale, slug); edit-URL follows the rendered file
│   └── sidebar.ts                   # EDIT — per-locale titles; order stays single-source
├── lib/components/
│   ├── Seo.svelte                   # EDIT — alternates, og:locale, inLanguage
│   ├── LanguageSwitcher.svelte      # NEW
│   ├── LocaleOfferBanner.svelte     # NEW — client-side, no redirect
│   ├── FallbackNotice.svelte        # NEW
│   ├── SiteHeader/SiteFooter.svelte # EDIT — switcher, localized links, EN badges
└── routes/
    ├── [[locale=locale]]/           # NEW subtree — MOVED, not copied:
    │   ├── +layout.svelte           #   chrome + notice + switcher
    │   ├── +layout.ts               #   prerender = true; locale in load data
    │   ├── +page.svelte             #   ← src/routes/+page.svelte
    │   ├── about/, roadmap/         #   ← existing routes
    │   └── docs/                    #   ← existing docs subtree, entries × locales
    ├── blog/, privacy/, terms/, registry/, api/, pay/   # unchanged, English-only
    └── sitemap.xml/+server.ts       # EDIT — locale loop + xhtml:link alternates
```

**Structure Decision**: one optional-param subtree (`[[locale=locale]]`) that the
existing route files **move into**. English paths are unchanged because an absent
optional param drops the segment (research §2). The English-only pages stay
outside it so they keep exactly one URL each.

## Delivery order

Each stage is independently shippable and leaves the site correct.

| stage | what lands | proves |
|---|---|---|
| **A — the plumbing, one locale** | matcher, `locales.ts`, `resolve.ts`, the route move, `%lang%`, prerender entries, alias 308s. Only `zh` has content. | US2 end to end, on the locale the founder can read. English URLs unchanged. |
| **B — the annotations** | `Seo.svelte` alternates, `x-default`, `og:locale`, JSON-LD `inLanguage`, sitemap alternates, **deletion of the stale `app.html` og:title**, reciprocity + sitemap tests | US4 |
| **C — extraction** | every English string on landing/about/roadmap into `messages/en.ts`; docs loader keyed by (locale, slug); `FallbackNotice`; switcher; offer banner; `EN` badges | US3's machinery, still 1 translated locale |
| **D — the other thirteen** | drafts for the remaining locales, chrome + notice first (they must be complete everywhere), then docs | 15 locales live, all `drafted` |
| **E — the review** | `bun run i18n:status`, `review.json`, R7 pass per locale, High+Medium repaired, findings recorded | US5, SC-010/SC-011 |

Stage A is where the risk is. Everything after it is repetitive.

## Risks, and what is done about each

| risk | mitigation |
|---|---|
| The route move touches every page file at once and a mistake is invisible until deploy | Stage A ends with the curl matrix in [quickstart.md](./quickstart.md) — all English paths `200`, no redirect. It is a test, not an eyeball. |
| 210 prerendered pages blow up build time (Shiki, 165 markdown compiles) | Timed at the end of Stage A, with one locale of docs present — before 126,000 words exist to make it worse. |
| `config.prerender.entries: '*'` silently strips the optional param and only English is emitted | Every localized path comes from a route-level `entries` export; the build's page count is the assertion (quickstart). |
| A key-level fallback quietly produces half-translated pages | Fallback is page-granular by construction (data-model, `TranslationRecord`), and the completeness check is per namespace. |
| Machine translation softens the honesty posture ("audited", "beta", a promised audit) | `docs.honesty` check + FR-032; A02 FR-2 wording is pinned per locale. |
| Fourteen unreviewed locales look finished | Nothing rendered claims a review state; `review.json` + the R7 findings files are the only place "reviewed" exists (FR-028). |

## Open item carried from the spec

The taglines for the 13 locales other than `en`/`zh` are drafts pending a native
reading (FR-006). Stage D ships them as `drafted`; a locale keeps the English
headline until its own line is approved. This blocks nothing.
