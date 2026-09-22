# Implementation Plan: getvela.app — every sentence true, findable, and ready for an expert reader

**Branch**: `080-site-content-accuracy` | **Date**: 2026-09-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/080-site-content-accuracy/spec.md`

## Summary

A content feature, not a code feature. The work is: (1) build a source-verified
**claim ledger** for every public page and the README; (2) turn every false,
stale, overstated, contradictory, unclear or unfindable item into a **finding**
with a severity; (3) repair English and Simplified Chinese together, add the
missing **self-hosting guide** and a **task-oriented docs home**; (4) re-align
the other 13 locales to the finalized English/Chinese and review each one with
the single-string method; (5) add **drift tracking for translated docs** so the
next English edit cannot silently leave 13 locales describing the old product;
(6) correct the internal authority documents that would otherwise reinstate the
retired facts.

Research (Phase 0) is four source-verification reports run against `main` at
`dd482131` plus the service repositories, consolidated in [research.md](research.md).
The verdict on each claim lives in [claim-ledger.md](claim-ledger.md); the defect
list with severities lives in [audit-report.md](audit-report.md).

## Technical Context

**Language/Version**: Markdown (mdsvex) for docs and blog; TypeScript message
catalog (`en.ts`) + 14 JSON catalogs for page copy; Svelte 5 components for
page structure; Markdown README.

**Primary Dependencies**: SvelteKit 2 site at `app-web/getvela.app` (Cloudflare
adapter), mdsvex, the site's own i18n store (spec 059). No new dependency.

**Storage**: Files in the repository only.

**Testing**: `bun run check` (unknown keys are type errors), `bun run test:unit`
(catalog shape, chrome completeness, href/tag parity per translation, hreflang,
docs loader), `bun run build` (prerender of every locale × page), `bun run
i18n:status --gate` (stale translations), a link check over the built output,
and screenshots of changed pages at desktop and phone width (the project's
"look before done" rule).

**Target Platform**: getvela.app (Cloudflare Workers), GitHub README.

**Project Type**: Documentation / marketing site content.

**Performance Goals**: N/A (copy). Page weight must not grow meaningfully; no new
client-side script.

**Constraints**:
- Site copy must never enter the vela-core i18n corpus (spec 059 R6).
- Spec 059 rulings stand: headline and its Chinese, locale set, English at root,
  two-link landing nav, first-screen layout, blog/legal English-only.
- A02 voice rules: approved taglines verbatim; mandated audit phrasing; no
  "beta / tolerate bugs" banners; plain and specific.
- No existing URL may break (spec R7).
- Several Claude sessions work in this repository at once: work happens in the
  dedicated worktree `vela-wallet-080`, paths are staged explicitly, never
  `git add -A` under `app-web/`.

**Scale/Scope**: 16 English docs (+1 new), 5 blog posts, 5 localized pages,
privacy, terms, header/footer, README; 15 locales → about 240 translated doc
files and 14 catalogs touched; plus 3 internal authority docs.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

`.specify/memory/constitution.md` is still the unfilled template, so there are no
ratified principles to check. The binding rules for public copy in this
repository are the product requirements and prior spec rulings, used here as the
gate:

| Gate | Source | Pre-design | Post-design |
|---|---|---|---|
| Approved taglines verbatim; retired tagline not reinstated | A02 FR-1 | Pass — headline untouched | Pass |
| Audit phrasing: Safe audited; Vela's integration not audited, none scheduled; never "planned" | A02 FR-2 | **Fail on `main`**: roadmap lists "an independent security audit" under upcoming | Pass after fix (T-roadmap) |
| Alpha stated honestly, no fear banners | A02 FR-3 | Pass | Pass |
| Canonical URLs only | A02 FR-4 | Pass | Pass — new guide names self-hosted domains as the reader's own, not ours |
| No token; no tracking claims match reality | A02 FR-5, A03 | Unverified — see research §4 | Pass after privacy fixes |
| Site copy stays out of the vela-core corpus | Spec 059 R6 | Pass | Pass — nothing added to `rust/crates/vela-core/i18n` |
| No broken URLs | Spec 080 R7 | Pass | Pass — new page added, none moved |
| Translation review method | Spec 059 R7, founder 2026-09-22 | Only zh reviewed | Pass after per-locale reviews |

No violations need justifying; the one failing gate is a defect this feature fixes.

## Project Structure

### Documentation (this feature)

```text
specs/080-site-content-accuracy/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0: consolidated source verification
├── claim-ledger.md      # Phase 0/1: every shared fact, its value, its evidence
├── audit-report.md      # Phase 1: findings with severity (Fatal/High/Medium/Low) and fix
├── data-model.md        # Phase 1: claim, finding, service, locale-review shapes
├── contracts/
│   ├── docs-information-architecture.md   # sidebar, docs home tasks, new page
│   ├── self-hosting-guide.md              # what the guide must contain, per service
│   └── translation-alignment.md           # en+zh reference, review record, doc fingerprints
├── quickstart.md        # how to validate the whole feature
├── reviews/<locale>.md  # per-locale single-string review records (implementation)
├── tasks.md             # Phase 2
└── results.md           # delivery report (implementation)
```

### Source Code (repository root)

```text
README.md                                         # root README — facts, status, pointer to guide
docs/CONTENT-SOURCE-100-CLUES.md                  # guardrail block corrected (authority doc)
docs/requirements/F01-supported-networks-registry.md   # network count corrected
docs/store-submission/store-listing-copy.md       # factual lines corrected
app-web/getvela.app/
├── src/content/docs/<slug>.md                    # 16 English docs + self-hosting.md (new)
├── src/content/docs/<tag>/<slug>.md              # 14 locales × 17 docs
├── src/content/blog/*.md                         # dated correction notes only
├── src/lib/i18n/messages/en.ts                   # landing / about / roadmap / get-started / chrome
├── src/lib/i18n/messages/<tag>.json              # 14 catalogs
├── src/lib/content/sidebar.ts                    # IA: new page, regrouping
├── src/routes/[[locale=locale]]/+page.svelte     # fact links (self-hosting proof target)
├── src/routes/[[locale=locale]]/docs/…           # docs home entry points
├── src/routes/privacy/+page.svelte               # data flows, key types, date
├── src/routes/terms/+page.svelte                 # fees, store purchases, date
├── src/lib/components/SiteFooter.svelte          # infrastructure column → guide
└── scripts/i18n-stamp.ts, i18n-status.ts         # doc fingerprints (drift tracking)
```

**Structure Decision**: Everything lives where it already lives. One new English
doc (`self-hosting.md`) and its translations; the docs home becomes a task index
inside the existing introduction page rather than a new route, so `/docs` keeps
its URL and content model.

## Complexity Tracking

None.
