# R7 review — `es-MX` (Mexican Spanish)

Date: 2026-09-15 · Method: [spec §R7](../spec.md) · Reference frame: **English
and Chinese** (founder, 2026-09-15).

**Scope: the four page namespaces** — `home`, `about`, `roadmap`, `getStarted`.
The sixteen docs are not translated into `es-MX` yet; they render English under a
notice, which is the designed fallback, not a finding.

## Findings

### es-MX-1 — `home.signing.*` and `home.tradeoffs.items[*].body` — accuracy — **High** — fixed

The English signing section was rewritten on 2026-09-15 after the founder ruled
that *"Sign what you see"* is the wrong claim to lead with — a passkey signs a
**hash**, and the decoded summary above it is produced by the same app that
built the transaction. The rewrite also expanded the three trade-offs from one
paragraph each into four, six and six.

`es-MX` kept the retired version: the old heading (`Firma lo que ves.`) and single-paragraph
trade-offs. So this locale was still making, in Mexican Spanish, exactly the claim the
English had withdrawn as overstated — the accuracy axis at its sharpest, because
the softened version is the one that reads better.

Repaired: `signing.heading`, `signing.lede`, `signing.today.body`,
`signing.next.title`, `signing.next.body`, `signing.aside.*`,
`tradeoffs.items[0..2].body` and `tradeoffs.items[1].title` retranslated from the
current English.

**How it hid.** `i18n:status` had flagged this locale stale. The flag was cleared
by re-stamping, on the reasoning that the file had been edited in the same commit
as `en.ts` — true, and irrelevant: that commit touched one pricing card. A
paragraph-structure gate now runs in `messages.test.ts` so the same drift cannot
be stamped away again.

## Axes with no further findings

**Naturalness and register.** Internally consistent throughout — the headline,
the CTA and the body address the reader the same way. Checked against the
locale's own page, not against a back-translation.

**Consistency.** One term per concept inside this locale: the passkey term, the
relay, self-custody and clear signing each appear one way only.

**Technical.** `href` set, inline tags and — since this pass — paragraph counts
match the English, all three asserted by `messages.test.ts` rather than read.

**Honesty posture.** "No audit is scheduled" is not softened into "planned"
anywhere; asserted mechanically for every locale.

## State

`review.json` → stays **`drafted`**, and deliberately so. Every High and Medium
found here is repaired, but this pass was a reading by the session that wrote the
repair, not by a native speaker of Mexican Spanish. The bar this project set is a reader
who would notice what a competent non-native cannot. Until that happens,
`drafted` is the honest label.
