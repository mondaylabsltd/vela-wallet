# Contract — the translation store and its checks

## Layout

```
app-web/getvela.app/src/
├── lib/i18n/
│   ├── locales.ts              # the 15 tags, endonyms, og locales, aliases
│   ├── messages/
│   │   ├── en.ts               # SOURCE OF TRUTH + the type every reader is checked against
│   │   ├── zh.json  ja.json  … # 14 partials
│   │   └── review.json         # per-locale review state (drafted | reviewed) + date
│   └── resolve.ts              # locale → catalog, page → state, path ↔ locale
└── content/docs/
    ├── <slug>.md               # English source, 11 files (unchanged)
    └── <tag>/<slug>.md         # translations
```

## Resolution

```
catalog(L)      = deepMerge(en, messages[L])          // for rendering chrome
state(P, L)     = "source"      if L = en
                | "translated"  if every key of namespace(P) is present in messages[L]
                                   (or, for docs, the file content/docs/L/slug.md exists)
                | "fallback"    otherwise
render(P, L)    = state = fallback ? english(P) + notice(L) : localized(P, L)
```

The notice is `catalog(L).notice.fallback` — which is why `notice.*` and
`chrome.*` must be complete in all 15 locales at all times (data-model §3): the
one string a reader of a fallback page must be able to read is the one saying the
rest is English.

## Checks — all vitest, all in `bun run test`

| check | fails when | requirement |
|---|---|---|
| `locales.parity` | the site's tag list ≠ `app-web/vela-wallet/src/lib/i18n/locales.ts` `SUPPORTED_LOCALES`, read from disk | FR-007 |
| `messages.shape` | a translation has a key `en.ts` does not | FR-026 |
| `messages.placeholders` | a value's placeholder/link/markup set ≠ the English value's | FR-031 |
| `messages.chrome-complete` | any locale is missing a `chrome.*` or `notice.*` key | data-model §3 |
| `hreflang.reciprocity` | the alternate sets are not symmetric, or a fallback page is advertised | FR-021, FR-018 |
| `sitemap.no-404` | a sitemap URL does not correspond to a generated page | FR-025 |
| `docs.honesty` | the strings pinned by A02 FR-2/FR-3 (no audit, alpha) are absent from a locale that claims `translated` | FR-032 |

**None of these block on a missing translation** (FR-027): a locale with an
incomplete page is `fallback`, which is a legal state, not a failure. The failures
above are all *inconsistencies* — a key that exists nowhere in English, a broken
placeholder, an asymmetric annotation.

## The report

`bun run i18n:status` prints, and `i18n.status.test.ts` asserts it can be
computed:

```
locale   pages  translated  fallback  stale  review
ja          14          14         0      2  drafted
pt-BR       14           9         5      0  drafted
zh          14          14         0      0  reviewed   2026-09-20
```

*stale* = the English source changed after the translation was written, detected
by a content hash of the English namespace/file stored alongside the translation.
Stale is reported, never fatal, and it moves the locale back to `drafted`
(data-model, transitions).

## Review record

`messages/review.json`:

```json
{ "zh": { "state": "reviewed", "date": "2026-09-20", "findings": "specs/059-getvela-site-i18n/reviews/zh.md" },
  "ja": { "state": "drafted" } }
```

A locale may only be `reviewed` when its R7 findings file exists and contains no
open High or Medium finding (FR-029, SC-011). Nothing rendered to a reader claims
a review state — the record is for the team (FR-028).
