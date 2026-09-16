# Research — 059, the fifteen-language site

**Date**: 2026-09-15 · **Branch**: `059-getvela-site-i18n`

Everything below was read out of the working tree or `node_modules`, not recalled.
Line references are to the tree at commit `eade0f56`.

## 1. What the site is today

| fact | evidence |
|---|---|
| SvelteKit 2.50 + Svelte 5 (runes), mdsvex for markdown, Shiki at build time, adapter-cloudflare | `package.json`, `svelte.config.js` |
| **Mixed render mode.** `/docs/**`, `/about`, `/blog/**`, `/sitemap.xml`, `/blog/rss.xml` are prerendered; `/` (the landing page), `/roadmap`, `/privacy`, `/terms`, `/registry` have **no** `prerender` export and are therefore rendered per request on the Worker | `grep prerender src/` — flags live in `src/routes/docs/+layout.ts`, `about/+page.ts`, `blog/**` |
| No i18n of any kind: no locale file, no `lang` handling, `app.html` hardcodes `lang="en"` | `src/app.html:2` |
| Docs = 11 markdown files loaded by one eager glob, keyed by filename | `src/lib/content/docs.ts:9` |
| Docs order/grouping is a hand-written table, and it is the single source of truth for the sidebar and the sitemap | `src/lib/content/sidebar.ts` |
| Docs slugs prerender through `entries` | `src/routes/docs/[...slug]/+page.ts:5` |
| Two SEO paths coexist: a local `Seo.svelte` (used by about/docs/blog/registry) and hand-written `<svelte:head>` on the landing page | `src/lib/components/Seo.svelte`, `src/routes/+page.svelte:252` |
| `@shelchin/seo-sveltekit` is a dependency but the site uses its **own** `Seo.svelte`; only the OG-image route touches the package | `package.json`, `src/routes/api/og/+server.ts` |
| Word volume: docs 8,017 · blog 3,291 · landing+about+roadmap prose ≈ 1,900 | `wc -w` |

### Defect found while reading (fix inside this feature)

`src/app.html` emits a **third, stale tagline** — `og:title` and `twitter:title` are
hardcoded to *"Vela Wallet — No seed phrases. Just your fingerprint."* on **every
page of the site**, duplicating the per-page OG tags that `Seo.svelte` and the
landing page already emit. Two `og:title` tags on one page is a real SEO defect
(crawlers take the first), and the first one is copy no one has owned for months.
It has to go before `hreflang` work means anything.

## 2. Routing: can English stay at `/` while 14 locales get a prefix?

**Decision: one `[[locale=locale]]` optional-param subtree, with a param matcher
that accepts only the 14 non-English tags.**

The risk was that optional params and prerendering do not mix. They do:
`resolve_route()` drops a segment whose optional param is absent
(`node_modules/@sveltejs/kit/src/utils/routing.js:258-259`), and route-level
`entries` are resolved through exactly that function
(`src/core/postbuild/prerender.js:545`). So an `entries` generator returning
`[{locale: undefined}, {locale: 'ja'}, …]` prerenders `/` **and** `/ja` from one
route file.

Two consequences worth stating up front:

- `config.prerender.entries: '*'` **strips** optional params
  (`prerender.js:531`), so the wildcard alone would only ever produce the English
  pages. Every localized path must come from a route-level `entries` export or
  from the crawler following links. The language switcher links to all 15
  versions of the current page, so the crawler finds them too — but `entries` is
  the guarantee, not the crawl.
- The matcher must reject `en`. If `/en/` also resolved, every English page would
  have two URLs. `/en/*` returns 404; `x-default` and the canonical both point at
  the unprefixed path.

**Alternatives rejected**

- *A parallel `/[locale]/…` subtree duplicating each route file.* Two copies of
  every page to keep in sync; the first drift is a page that exists in English
  only by accident.
- *A `handle` hook rewriting `/ja/x` → `/x` and setting a locale in `locals`.*
  Invisible in the route tree, breaks `entries`-based prerendering, and makes
  `hreflang` generation a special case instead of a function of the route.
- *The wallet app's pattern (`/` is a 307 that negotiates `Accept-Language`,
  `src/routes/+server.ts`).* Correct for an app behind a passkey; wrong for the
  indexed surface — R3. Adopted from it: the locale table and the negotiation
  code, not the redirect.

## 3. Where the fifteen tags come from

`app-web/vela-wallet/src/lib/i18n/locales.ts:11` — `SUPPORTED_LOCALES` = `en, zh,
zh-TW, zh-HK, ja, ko, vi, id, tr, es-MX, pt-BR, fr, de, ru, it`. That file already
carries a comment saying it is copied from the app and must be kept in step; the
site becomes the third copy, so the parity check (FR-007) reads the wallet file at
test time rather than trusting a duplicated array.

The corpus on disk confirms the same fifteen:
`rust/crates/vela-core/i18n/locales/` has one `<tag>.json` + `<tag>/` per locale.

URL segments use the tag verbatim, lowercased for matching only: `/pt-BR/` is
canonical, `/pt-br/` and `/PT-BR/` resolve to it with a 308 so one page never has
three URLs. `/pt/`, `/es/`, `/zh-CN/`, `/zh-SG/` are *aliases*, not catalogs —
they 308 to the nearest supported tag using the same base-language rule the wallet
uses, so a link written by hand still lands somewhere sensible.

## 4. Message store

**Decision: `src/lib/i18n/messages/en.ts` (typed, source of truth) +
`messages/<tag>.json` (partial, machine-drafted then reviewed), merged at build.**

- `en.ts` is TypeScript so the shape is a type, and every component reading a key
  is checked by `svelte-check` — the existing `bun run check` becomes the guard
  against a typo'd key.
- Translations are JSON so a translator (human or machine) never edits code and
  can never break the build.
- Merge is **page-granular, not key-granular**: for page P in locale L, if any key
  P needs is missing, the whole page renders English under the notice. A
  key-level merge would produce exactly the half-translated page SC-006 forbids.

**Rejected: putting site copy in `rust/crates/vela-core/i18n/`** — R6. That corpus
is compiled into the wasm every *wallet* client ships and is already near a `u16`
packing ceiling; ~9,000 words of marketing prose per locale has no business in it.

## 5. Docs content per locale

**Decision: `src/content/docs/<slug>.md` stays the English source; translations
live at `src/content/docs/<tag>/<slug>.md`.** One eager glob
(`/src/content/docs/**/*.md`) keyed by `(locale, slug)`; no directory ⇒ no
translation ⇒ fallback. `sidebar.ts` keeps owning order and grouping (it is also
what the sitemap walks), and gains a per-locale title lookup rather than a second
ordering table per language — order is a property of the docs, not of the
language.

The "edit this page on GitHub" link (`docs.ts:45`) must resolve to the file that
actually rendered, i.e. the translation when there is one and the English source
when the page fell back — otherwise a reader clicks "edit" and gets the wrong
file (FR-019).

## 6. `<html lang>` on a prerendered page

`app.html` is static. SvelteKit's supported seam is `transformPageChunk` in the
`handle` hook, which **also runs during prerendering**. `src/hooks.server.ts`
already exports `handle` (CORS for `/api/*`), so this is an addition to an
existing function, not a new one: replace a `%lang%` placeholder in `app.html`
with the locale resolved from the first path segment.

## 7. The Accept-Language offer, without a redirect

Prerendered pages are identical for every visitor, so the offer cannot be
server-rendered — it must be a small client-side banner reading
`navigator.languages`, shown once, dismissal in `localStorage`. This is the only
way to have R3 (no redirect, cacheable at the edge) and FR-013 (the offer) at the
same time. It is also why the banner's own text must ship in all 15 locales in
the initial payload — it is shown *to a reader who cannot read the page*.

## 8. SEO annotations

- `hreflang`: emitted per page for **only the locales that genuinely have that
  page** (FR-018/FR-021) — a falling-back page is not a translation and must not
  be advertised as one, or Google serves an English page to a Japanese searcher
  and the pair is scored as duplicate content. Reciprocity is mechanical because
  both sides are generated from the same translation record.
- `x-default` → the unprefixed English URL.
- Canonical: self, always (`Seo.svelte` already does this; the landing page needs
  to stop hand-rolling its head and use the same component).
- Sitemap: `xmlns:xhtml` + one `<xhtml:link>` per real alternate. The existing
  generator already walks `flatSidebar` and `getAllPosts`, so it grows a locale
  loop, not a rewrite.
- JSON-LD: `inLanguage` on the WebSite node, and the Organization `description`
  in the served language.
- OG: `og:locale` + `og:locale:alternate`. The OG **image** stays
  language-neutral (out of scope) — it is a rendered PNG and localizing the text
  inside it would mean 15 renders per page.

## 9. Volume and build cost

14 localized pages (landing + 11 docs + about + roadmap) × 15 locales = **210
prerendered pages**, plus the English-only blog (5 posts + index), legal (2) and
registry. Today's build prerenders ~20. Shiki highlighting is per-markdown-file
and dominates docs build time: 11 files → 165. This is the one number in the plan
that could surprise, so the build is timed in Phase 1 before the translation work
starts, not after.

## 10. Blog and legal, reached from a localized page

R5 keeps them English and **outside** the locale subtree, so they have exactly one
URL each and no duplicate-content exposure. FR-020 is then satisfied by the
localized navigation marking those links — an `EN` badge and an `hreflang="en"` on
the link — rather than by 75 canonicalized copies of the blog.

**Rejected: `/{locale}/blog` rendering English posts in localized chrome.** It
reads nicer, but it is 105 extra pages that all canonicalize elsewhere, for
content nobody asked to have translated.

## 11. Test seams that already exist

`vitest` (unit, with `vitest-browser-svelte`) and `@playwright/test` are both
configured, and `src/lib/registry.test.ts` shows the house pattern for a pure unit
test. The locale-parity check (FR-007), the translation-completeness report
(FR-026) and the `hreflang` reciprocity check (FR-021) are all pure functions over
files on disk — they belong in vitest, not in a bespoke script, so they run in the
same `bun run test` everything else runs in. The end-to-end claims (a locale root
returns rendered copy with JS disabled; switching language keeps the path) are
Playwright's.
