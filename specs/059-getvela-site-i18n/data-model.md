# Data model — 059

Four things exist. Everything else in the feature is a function over them.

## Locale

One of the fifteen tags. Not a free string anywhere: a `Locale` type derived from
one array, and a route param matcher that rejects everything else.

| field | value | rule |
|---|---|---|
| `tag` | `en` `zh` `zh-TW` `zh-HK` `ja` `ko` `vi` `id` `tr` `es-MX` `pt-BR` `fr` `de` `ru` `it` | MUST equal `app-web/vela-wallet/src/lib/i18n/locales.ts` `SUPPORTED_LOCALES`, asserted by a test that reads that file (FR-007) |
| `segment` | the tag verbatim, or absent for `en` | `en` has **no** segment; `/en/*` is 404 (FR-008, FR-010) |
| `endonym` | `English` `简体中文` `繁體中文` `繁體中文（香港）` `日本語` `한국어` `Tiếng Việt` `Bahasa Indonesia` `Türkçe` `Español (México)` `Português (Brasil)` `Français` `Deutsch` `Русский` `Italiano` | written in its own language, never in English (FR-012) |
| `ogLocale` | `en_US` `zh_CN` `zh_TW` `zh_HK` `ja_JP` `ko_KR` `vi_VN` `id_ID` `tr_TR` `es_MX` `pt_BR` `fr_FR` `de_DE` `ru_RU` `it_IT` | FR-023 |
| `dir` | `ltr` for all fifteen | recorded so a sixteenth locale cannot be added without confronting it |
| `review` | `source` \| `drafted` \| `reviewed` | `en` is `source`; the rest start `drafted` (FR-028) |

**Aliases** (not locales — they only resolve): `pt`→`pt-BR`, `es`→`es-MX`,
`zh-CN`/`zh-Hans`/`zh-SG`→`zh`, `zh-Hant`/`zh-MO`→`zh-TW`, plus any
`<base>-<region>` falling back to `<base>` when that base is supported.

## Page

The localizable unit. Fourteen of them, and the set is closed by R5.

| field | value |
|---|---|
| `id` | `home` · `about` · `roadmap` · `docs:<slug>` (11 slugs from `sidebar.ts`) |
| `path(locale)` | `home` → `/` or `/<seg>/`; `docs:introduction` → `/docs` or `/<seg>/docs`; others mirror the English path under the segment |
| `source` | for `docs:*`, the markdown file; for the other three, a namespace of the message catalog |

Not pages, and deliberately: `/blog/**`, `/privacy`, `/terms`, `/registry`,
`/demo`, `/api/**`. They keep exactly one URL each (research §10).

## MessageCatalog

- `src/lib/i18n/messages/en.ts` — the **source of truth and the type**. Nested by
  page namespace (`home`, `about`, `roadmap`, `chrome`, `notice`).
- `src/lib/i18n/messages/<tag>.json` — a translation. Same shape or a subset.

Rules:

1. A key may exist in a translation only if it exists in `en.ts` (an extra key is
   a stale key and is reported, FR-026).
2. Placeholders, links and inline markup inside a value MUST match the English
   value's set exactly — same names, same count (FR-031).
3. `chrome` and `notice` MUST be complete in **every** locale, because they are
   what a fallback page is wrapped in. A missing `chrome` key is the one
   translation failure that is not survivable by falling back — the fallback
   itself would be in the wrong language.
4. Completeness is evaluated **per page namespace**, never per key (research §4).

## LocalizedDoc

`src/content/docs/<slug>.md` (English source) and
`src/content/docs/<tag>/<slug>.md` (translation). Front-matter `title` and
`description` are part of the translation. Absent file ⇒ no translation ⇒
fallback.

## TranslationRecord — the derived one

Computed at build time, not hand-maintained. For every (Page, Locale):

```
state    = "source"      when locale is en
         | "translated"  when every key/file that page needs exists for that locale
         | "fallback"    otherwise
reviewed = the locale's review state
```

It is the single input to three separate outputs, which is why it exists at all:

| consumer | rule |
|---|---|
| page render | `fallback` ⇒ English body + notice in the reader's language (FR-017) |
| `hreflang` | emit an alternate for a locale **only** when its state is `translated`/`source` (FR-018, FR-021) |
| sitemap | same rule — a fallback URL is listed under no locale but its own (FR-025) |

### State transitions

```
absent ──translate──▶ drafted ──R7 review, High+Medium repaired──▶ reviewed
   ▲                     │
   └──English changed────┘        (the English source changing marks the
                                   translation stale = drafted again, FR-026)
```

A page is publishable in state `drafted` — R4 and FR-027 are explicit that
missing or unreviewed translations never block a release. What `reviewed` buys is
the right to say the locale is done, and nothing on the page may claim it before
then (FR-028).
