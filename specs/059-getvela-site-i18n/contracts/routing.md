# Contract — URLs and status codes

The site's public surface after 059. `<seg>` is any of the 14 non-English tags.
Every row is a test.

## Must not change (FR-009, SC-005)

| URL | after |
|---|---|
| `/` | 200, English, same content |
| `/docs`, `/docs/<slug>` (11) | 200, English |
| `/about`, `/roadmap`, `/privacy`, `/terms`, `/registry` | 200, English |
| `/blog`, `/blog/<slug>` (5) | 200, English |
| `/sitemap.xml`, `/blog/rss.xml`, `/api/**`, `/pay`, `/.well-known/**` | unchanged |

No redirect is added to any of them. A crawl before and after returns the same
path set with the same status codes.

## New

| URL | status | body |
|---|---|---|
| `/<seg>/` | 200 | landing page, that locale (or English + notice) |
| `/<seg>/docs` | 200 | `introduction`, that locale |
| `/<seg>/docs/<slug>` | 200 | that doc, that locale |
| `/<seg>/about`, `/<seg>/roadmap` | 200 | that locale |

All 210 are prerendered and served as static assets.

## Must 404

| URL | why |
|---|---|
| `/en/`, `/en/docs`, … | English has one URL, and it is unprefixed (FR-008) |
| `/xx/`, `/fr-CA/`, `/zzz/` | not a supported tag and not an alias (FR-010) |
| `/<seg>/blog`, `/<seg>/privacy`, `/<seg>/terms`, `/<seg>/registry` | outside the locale subtree by R5 (research §10) |

A 404 here is the page's own 404 — not a blank locale shell, and never a soft-200.

## Must 308 (one canonical URL per page)

| from | to |
|---|---|
| `/pt-br/…`, `/PT-BR/…`, `/ZH-tw/…` | the canonically-cased tag |
| `/pt/…` | `/pt-BR/…` |
| `/es/…` | `/es-MX/…` |
| `/zh-CN/…`, `/zh-Hans/…`, `/zh-SG/…` | `/zh/…` |
| `/zh-Hant/…`, `/zh-MO/…` | `/zh-TW/…` |
| `/<base>-<region>/…` where `<base>` is supported | `/<base>/…` |

308 (not 302): the alias is permanent and the method must be preserved. These are
the only redirects the feature adds, and none of them touch an English URL.

## Negotiation

`Accept-Language` changes **nothing** about what a URL returns (R3). The only
place it is read is the client-side offer banner, from `navigator.languages`
(FR-013):

- shown at most once per visitor per locale suggestion, dismissal persisted;
- shown in the language being offered;
- never auto-navigates;
- absent entirely when the visitor is already on that locale, or has dismissed it,
  or prefers English.

## Language switcher (FR-012)

On every page, in every locale, links to the same page in all 15 locales:
`switchTo(locale, currentPath)` maps `/ja/docs/faq` → `/docs/faq` (en) or
`/pt-BR/docs/faq`. It never maps to `/`. On the English-only pages the switcher
is absent (there is no other version of that page to go to); the navigation
instead marks those links `EN` (FR-020).
