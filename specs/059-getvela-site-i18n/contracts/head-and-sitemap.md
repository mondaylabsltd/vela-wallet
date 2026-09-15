# Contract — what every page emits, and what the sitemap says

## Per page

Given page `P`, locale `L`, and `A = { locales whose state for P is translated or source }`:

```html
<html lang="{L.tag}">                          <!-- FR-014, via transformPageChunk -->
<title>{localized}</title>                     <!-- FR-023 -->
<meta name="description" content="{localized}">
<link rel="canonical" href="https://getvela.app{path(P, L)}">   <!-- self, always, FR-022 -->

<!-- one per member of A, including L itself; nothing for a locale in fallback -->
<link rel="alternate" hreflang="{a.tag}" href="https://getvela.app{path(P, a)}">
<link rel="alternate" hreflang="x-default" href="https://getvela.app{path(P, en)}">

<meta property="og:locale" content="{L.ogLocale}">
<meta property="og:locale:alternate" content="…">   <!-- one per a ∈ A, a ≠ L -->
<meta property="og:title" …> <meta property="og:description" …>   <!-- localized -->
<meta property="og:url" content="{canonical}">
```

Invariants, each a test:

1. **Reciprocity.** If `p(L₁)` lists `L₂`, then `p(L₂)` lists `L₁`. Mechanical:
   both are rendered from the same `A`.
2. **Self-reference.** `A` always contains `L` (a page always lists itself).
3. **x-default is English**, on every page, in every locale.
4. **No fallback page is advertised.** If `P` in `L` is state `fallback`, then no
   page anywhere emits `hreflang="{L}"` for `P` — including `P` in `L` itself,
   which emits the rest of the set but not its own tag. It is an English page at
   a localized URL; claiming otherwise is what earns the duplicate-content
   penalty (FR-018).
5. **One `og:title` per page.** `src/app.html` currently hardcodes a second,
   stale `og:title`/`twitter:title` on every page (research §1) — it is removed;
   per-page tags are the only source.
6. **JSON-LD** carries `inLanguage: L.tag`, and the Organization `description` in
   `L` (FR-024).

## Sitemap

```xml
<urlset xmlns="…/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
  <url>
    <loc>https://getvela.app/docs/faq</loc>
    <xhtml:link rel="alternate" hreflang="ja" href="https://getvela.app/ja/docs/faq"/>
    …one per member of A, plus x-default…
  </url>
  …one <url> per (page, locale) with state translated or source…
</urlset>
```

- English-only pages (blog, legal, registry) appear once, with no alternates.
- A URL that would 404 never appears (FR-025) — which follows from generating the
  file from the same `TranslationRecord` the router and the head use, rather than
  from a second list.
- Existing `lastmod`/`changefreq`/`priority` behaviour is unchanged.

## Robots

Unchanged. No locale is disallowed, and no fallback page is `noindex` — it is a
legitimate English page at a localized URL, just not a translation.
