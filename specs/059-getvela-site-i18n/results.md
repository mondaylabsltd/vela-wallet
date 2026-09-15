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

## Founder pass on the landing page — 2026-09-15 (outside the task list)

Not a numbered task: a founder review of `/` arrived mid-phase and rebuilt the
page's information architecture. Recorded here because it changes copy that
T020/T029 had already extracted and translated, and because two rulings came out
of it that later work has to respect.

### What the page is now

`hero → why (short) → trade-offs → sign what you see → compare → pricing → how →
networks → FAQ`. Removed: "A wallet that does less" and the whole bottom
"Ready to try it?" block (its social links already live in the footer). The nine
paragraphs of "Why we built Vela" moved to a new doc, `/docs/why-vela`, with the
landing page keeping three sentences and a link.

### Two founder rulings

1. **The independent signing page may be advertised, labelled.** `app-web/clearsigning`
   is built and tested but not deployed and not wired into the wallet
   (HANDOVER.md §还没做 item 4), so the section carries "Built and tested — not
   live yet" and the comparison row reads "in testing". The draft line "it's off
   by default — turn it on" was false today and is not on the page.
2. **No price number until the stores are configured.** Pricing says "one-time
   purchase — never a subscription"; the `$39.99` / regional figures in
   docs/marketing/pricing-analysis.md §99 stay out of public copy for now.

### Accuracy fixes made while writing

- The draft claimed a hardware key as a second signer removes the
  Apple/Google-account risk. It does not: signers are **1-of-n**, so a second key
  is a way back in, not a second lock. The trade-off item, the FAQ answer and
  `/docs/why-vela` now say that, and point at `KeyMethod::SecurityKey` — the
  first key's method is a choice — as the actual answer to that risk.
- "Platforms" in the comparison says "Web today; desktop, mobile and an extension
  … in testing", matching `/get-started`, not the draft's "desktop (Windows /
  macOS / Linux)" which reads as shipped.
- Fees are described as the relayer quote (network cost + service fee, locked
  into what you sign, small minimum, gas-account activation deposit), per
  docs/networks-and-fees.md, not as a generic "bundler fee".

### The hero, after three rounds of founder notes

The subtitle is one sentence again; its second half ("open source and
self-hostable … even if we disappear") became one of five **hooks** on the right,
each a single line with no explanatory paragraph and a link to the page that
proves it. Written from the reader's fear rather than our architecture: *No one
can freeze your money. Not even us. / One address. Every chain you use. / Lose
your phone. Keep your wallet. / If Vela disappears, your wallet does not. / Never
sign what you cannot read.* Hrefs live in `FACT_LINKS` in the component, beside
`COMPARE_TONES`, for the same reason: a translation may not move a destination.

Layout: both hero columns stretch to a shared top and bottom edge, the on-chain
seal is the bottom anchor of the left column, and the CTA buttons are 230×62.
**A latent bug surfaced here**: `.btn` is declared *after* `.btn-hero` in this
stylesheet, so at equal specificity the hero buttons had silently never had their
own padding — every previous size change to `.btn-hero` did nothing. Fixed as
`.btn.btn-hero`.

### Checks

`bun run check` 0 errors · `vitest --project server` 41 passed ·
`bun run test:e2e` **56 passed** · `bun run build` green, 11 docs × 15 locales
(`/docs/why-vela` prerendered and in the sitemap).

`zh.json` was rewritten alongside `en.ts`, so `namespaceState('home','zh')` is
still `translated` — had it not been, `/zh` would have fallen back to English
under a notice and SC-006's demo would have regressed silently.

**Hazard repeated from Phase 2**: `bun run format` is `prettier --write .` and
reformatted ~30 files this session that nothing in this work had touched. They
were reverted; format single files instead while the tree is shared.
