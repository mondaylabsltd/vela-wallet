# R7 review records

One file per locale, per [spec.md](../spec.md) §R7 and the founder's two
methodologies. A locale may be marked `reviewed` in
`app-web/getvela.app/src/lib/i18n/review.json` **only** when its file here has no
open High or Medium finding.

Each file says what the pass actually covered. That matters: a pass over the four
page namespaces is not a pass over the sixteen docs, and saying "reviewed"
without that distinction is the kind of claim this project does not make.

`single-string.md` is the second methodology — one string across all fifteen
locales, which catches what a per-locale read cannot: the one language that took
a literal path everyone else avoided.

## Severity

| level | means |
|---|---|
| **High** | the meaning is wrong, reversed, or the honesty posture softened — repair before shipping |
| **Medium** | reads as translated rather than written; terminology inconsistent within the locale — repair |
| **Low** | improvable, no reader is misled — record, fix when the string is next touched |

High and Medium are repaired in the same pass that finds them. A finding stays in
the file after repair, marked `fixed`, because the record of what went wrong is
the point.
