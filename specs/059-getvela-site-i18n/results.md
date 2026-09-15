# Results — 059

Running record. Each entry is what was actually run, not what was intended.

## Phase 1 — the locale table (T001–T006) · 2026-09-15

`src/lib/i18n/locales.ts` + `messages/{en.ts,zh.json,review.json}`.

- `bun run test:unit --run --project server` — **27 passed**, including the
  parity test that parses `app-web/vela-wallet/src/lib/i18n/locales.ts` and
  `src/lib/settings/fixtures.ts` off disk (FR-007).
- The site was byte-identical at this point — nothing imported any of it yet.

## Phase 2 — the route subtree (T007–T019) · 2026-09-15

`src/params/locale.ts`, `src/routes/[[locale=locale]]/**`, `%lang%`, alias 308s,
`src/routes/+error.svelte`.

### T019 — the build budget, measured before the words arrive

| | value |
|---|---|
| `bun run build` wall time | **~5.3 s** |
| prerendered HTML files | **216** |
| of which localized | 210 = 14 pages × 15 locales |
| the other 6 | blog index + 5 posts (English-only, R5) |

Well under the ~5 min budget in plan.md. The reason is worth recording, because
it is what will change: **Shiki compiles each markdown file once, not once per
page**, so 15 locales of the same 11 English docs cost almost nothing today.
Stage D adds 11 × 14 = 154 *new* markdown files, and that is where build time
will actually move. Re-measure at T059 rather than assuming this number holds.

### T018 — the English site is untouched (FR-009, SC-005)

Against `wrangler dev`, every pre-existing path: `200`, no redirect.

```
/  /docs  /docs/faq  /docs/whitepaper  /about  /roadmap
/blog  /blog/hello-world  /privacy  /terms  /registry
/sitemap.xml  /blog/rss.xml
```

`/docs/`, `/about/`, `/blog/` still 307 to their unslashed form — pre-existing
SvelteKit trailing-slash behaviour, verified unchanged, not something 059 added.

### T017 — the URL contract, as tests

`bun run test:e2e` — **48 passed**. Covers all four tables in
contracts/routing.md: English untouched, 14 locale roots rendering with the
right `<html lang>`, the 404 set (`/en`, `/xx/`, `/ja/blog`, `/ja/privacy`…),
and the 308 set including query-string preservation.

Two config defects were fixed to get there, both pre-existing:

- `playwright.config.ts` waited on port **4173**, which nothing in this project
  ever serves (`bun run preview` is `wrangler dev`). The e2e suite could not have
  run before this. It now uses 8788 — its own port, because 4173 is shared with
  `app-web/vela-wallet`'s suite and two sessions on one port silently test each
  other's build.
- `reuseExistingServer` is off, for the same reason.

### Deviation from tasks.md

- **T016** creates `src/routes/+error.svelte`, not one inside the locale subtree.
  An unknown prefix matches *no* route, so the subtree's layout never runs and an
  error page inside it would never render. The root one is what a 404 actually
  reaches.
- **T017** lives at `e2e/routing.e2e.ts`, not `tests/routing.spec.ts` — the
  project's Playwright `testMatch` is `**/*.e2e.ts`.

### Incident — a moved file came back

Mid-phase, `bun run build` began failing with

```
The "/" and "/[[locale=locale]]" routes conflict with each other
```

Cause: `src/routes/+page.svelte` had been `git mv`d into the locale subtree while
it was **open and dirty in the founder's editor**. The editor's next save
recreated it at the old path, and two routes then claimed `/`.

Recovery: the resurrected copy carried the founder's own in-progress hero edit
(five facts cut to three). That edit was merged into the moved file — it was not
discarded — and the stale root copy removed. See the commit for what was merged
and the one wording judgement made on a half-typed line.

**Standing hazard**: an editor holding the old path will break the build again on
its next save. The landing page now lives at
`src/routes/[[locale=locale]]/+page.svelte`.
