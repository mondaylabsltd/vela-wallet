# Feature Specification: getvela.app — the headline, and the fifteen languages

**Feature Branch**: `059-getvela-site-i18n`

**Created**: 2026-09-15

**Status**: Draft

**Input**: The founder, 2026-09-15, in three messages:
1. "使用 github speckit 059 来修改 getvela.app 落地页和文档。首先修改 Your keys. Your face. 要改为
   `An Ethereum wallet you actually own`"
2. "而且 getvela.app 中的内容要支持多语言，英语，简体中文，日语，葡萄牙语，就这四种"
3. "An Ethereum wallet you actually own 对应中文是 `真正属于你的以太坊钱包`" …then, on being asked
   about scope: "我觉得还是要支持更多语言，和 app-web/vela-wallet 支持一样多的语言，并且做好 seo ssr"

The last message supersedes the four-language list in message 2.

## Why this exists

Two separate things are wrong with the same page.

**The headline sells the mechanism, not the promise.** `Your keys. Your face.`
names two implementation facts — a key you hold, a face that unlocks it — to a
reader who has not yet been told what they would be getting. Ownership is the
product; the passkey is how it is delivered. `An Ethereum wallet you actually
own` says the thing the reader came to find out, and leaves the face to the
subtitle, where it already is.

This is not a copy tweak in one file. `Your keys. Your face.` is an *authority*
string: `docs/requirements/A02-brand-voice-honesty-audit-posture.md` FR-1 lists
it among the taglines to be used **verbatim**, and
`docs/CONTENT-SOURCE-100-CLUES.md` clue 2 says **"Never improvise alternative
taglines."** Any agent writing marketing copy tomorrow reads those two files and
reinstates the retired line. Retiring a tagline therefore means editing the
documents that make taglines binding — not just the page.

**The wallet speaks fifteen languages; the page that sells it speaks one.**
`rust/crates/vela-core/i18n/locales/` carries 15 catalogs and
`app-web/vela-wallet` prerenders a page per locale. `app-web/getvela.app` has no
i18n at all — not a locale file, not a `lang` attribute beyond `en`, not an
`hreflang`. A reader who runs the app in Japanese meets the product in English.
Worse, the site — not the app — is the SEO and AI-citation surface: the app is
behind a passkey and is not indexed, so every non-English search impression this
product could ever earn has to come from this one SvelteKit app.

## The rulings this cut carries

Decisions already made, recorded so they are not re-litigated mid-implementation:

| # | ruling | why / source |
|---|---|---|
| R1 | English tagline is **`An Ethereum wallet you actually own`**; Simplified Chinese is **`真正属于你的以太坊钱包`** | founder, verbatim, 2026-09-15 |
| R2 | The locale set is **exactly** `app-web/vela-wallet`'s `SUPPORTED_LOCALES` — `en, zh, zh-TW, zh-HK, ja, ko, vi, id, tr, es-MX, pt-BR, fr, de, ru, it` | founder: "和 app-web/vela-wallet 支持一样多的语言". One table, two consumers; drift between them is a defect |
| R3 | English stays at the **root** (`/`, `/docs/faq`, …); translations live under a prefix (`/ja/`, `/zh-TW/docs/faq`, …). No auto-redirect on first visit | founder's choice. Every existing URL, backlink and ranking survives untouched — and unlike the wallet app (whose `/` is a 307 that negotiates `Accept-Language`), a marketing root that redirects hands crawlers and shared links the wrong language |
| R4 | A page with no translation yet **renders English under a visible notice** in that locale's chrome | founder's choice. Never a half-translated page, never a dead link |
| R5 | Localized surface = the **evergreen face**: landing page, all 11 docs pages, `/about`, `/roadmap`. Blog and `/privacy` + `/terms` stay English-only | founder's choice. Blog posts are dated and would each need 14 more translations forever; legal text is authoritative in English |
| R6 | Site copy **does not enter the `vela-core` corpus** | the corpus compiles into the wasm the *app* ships, its per-locale packing is already near a `u16` ceiling (ru has ~150 bytes of headroom), and ~9,000 words of marketing prose per locale would be dead weight in every wallet client. The site keeps its own store |
| R7 | Translation review follows the founder's two methodologies, cited below | founder, 2026-09-15: "i18n 翻译参考原则" — see **The review standard** |

### The review standard (R7)

The founder supplied two references, and they govern what "translated" means here:

- <https://shelchin.com/prompts/native-quality-i18n-translation-review/> — corpus-wide review
- <https://shelchin.com/prompts/single-string-i18n-locale-review/> — one string across all locales

Their core directive, which this spec adopts verbatim as its acceptance bar: the
goal is **not** to make all languages match the English, but that **each locale's
reader sees text that reads as if it were written natively in that language**, by
that market's product conventions — never an English or Chinese template pushed
mechanically through fifteen locales.

The five review axes, and what each means on this site:

| axis | on getvela.app |
|---|---|
| **Accuracy** | no omission, reversal or over-claim — and the honesty posture (no audit, alpha, "we architecturally can't") must survive translation intact, because softening it in one locale is a false claim about custody |
| **Naturalness** | reads as a native marketing/docs page, not a translated one; no English syntax carried over; headline and CTA lengths that work in that language, not the English word count |
| **Cultural & offence risk** | no commanding, arrogant or overselling tone; `zh-HK` is spoken Cantonese in this project and must not be silently served Traditional Mandarin |
| **Consistency** | one term per concept across landing, docs and switcher (passkey, self-custody, gas, clear signing), one formality level per locale |
| **Technical correctness** | placeholders, links, markdown and inline HTML preserved; number, date and currency formats per locale; length within the UI's constraints |

Findings are reported severity-ordered (High / Medium / Low) with file, locale,
key, current text, problem type, and the recommended fix; **High and Medium are
repaired**, placeholders and key structure untouched.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — The headline says what you get (Priority: P1)

A stranger lands on getvela.app and reads one line that tells them what they
would have if they used this: an Ethereum wallet they actually own. The face and
the absent seed phrase are still on the page — one line down, where they explain
*how*.

**Why this priority**: it is the founder's first instruction, it ships without
any of the i18n work, and it is the single most-read sentence the product has.

**Independent Test**: load `/` and read `<h1>`, `<title>`, `og:title` and the
footer tagline; then grep the repo for `Your keys. Your face.` and find it only
in files that describe history.

**Acceptance Scenarios**:

1. **Given** the landing page, **When** it is rendered, **Then** the `<h1>` reads
   `An Ethereum wallet you actually own` and no longer reads `Your keys. Your face.`
2. **Given** the document head, **When** a link is shared, **Then** `<title>` and
   `og:title` carry the new line.
3. **Given** the site footer on every page, **When** it renders, **Then** its
   tagline no longer contradicts the hero.
4. **Given** `docs/requirements/A02-…md` and `docs/CONTENT-SOURCE-100-CLUES.md`,
   **When** an agent reads them to write copy, **Then** the approved-tagline list
   names the new line and marks the old one retired, so the "use verbatim / never
   improvise" rule now protects the new line.
5. **Given** the hero illustration (a face becoming a key), **When** the headline
   no longer says "face", **Then** the visual's `aria-label` and alt text still
   describe what is drawn, not the retired tagline.

---

### User Story 2 — The site answers in the reader's language (Priority: P1)

A reader whose browser asks for Japanese opens getvela.app, is told in Japanese
that a Japanese version exists, clicks once, and gets the whole evergreen site in
Japanese at `/ja/` — served fully rendered, not assembled in the browser.

**Why this priority**: it is the load-bearing half of the feature; every later
story is scaffolding on top of the routing and the message store.

**Independent Test**: request `/ja/` with JavaScript disabled and read Japanese
copy in the HTML response; request each of the 15 locale roots and get a 200 with
`<html lang>` matching.

**Acceptance Scenarios**:

1. **Given** any of the 15 locales, **When** its landing page is requested,
   **Then** the response body already contains that locale's copy and
   `<html lang="…">` carries the tag.
2. **Given** an unknown prefix (`/xx/`, `/es/`), **When** it is requested,
   **Then** the reader gets a 404 — not a blank locale shell.
3. **Given** a reader on any page, **When** they open the language switcher,
   **Then** they see all 15 languages in their **own** endonyms (日本語, 简体中文,
   Português (Brasil)…) and choosing one lands on the *same page* in that
   language, not on the home page.
4. **Given** a first-time visitor whose `Accept-Language` prefers a non-English
   locale, **When** they land on an English URL, **Then** they are offered the
   translation once, in that language, and are **not** redirected; dismissing it
   is remembered.
5. **Given** the English site, **When** any existing URL is requested, **Then**
   it responds exactly as it does today — same path, no redirect.

---

### User Story 3 — The docs read in the reader's language, and admit when they don't (Priority: P2)

A reader in `/pt-BR/docs/` gets the docs in Portuguese; where a page has not been
translated yet, they get the English text under one honest line — in Portuguese —
saying so, with the sidebar, pager and "edit on GitHub" link still working.

**Why this priority**: the docs are two-thirds of the site's words and the part
that earns search traffic and AI citations; the fallback rule is what lets the
feature ship before every word is translated.

**Independent Test**: delete one locale's `faq` translation, build, and confirm
`/<locale>/docs/faq` renders English body + localized notice + localized chrome,
and that no `hreflang` alternate advertises that page for that locale.

**Acceptance Scenarios**:

1. **Given** a translated doc, **When** it is requested under its locale prefix,
   **Then** body, title, sidebar group names and prev/next labels are all in that
   language.
2. **Given** an untranslated doc, **When** it is requested under a locale prefix,
   **Then** the English body renders beneath a notice written in the reader's
   language, and the page is not advertised as a translation to search engines.
3. **Given** any docs page in any locale, **When** the reader clicks "edit this
   page", **Then** the link points at the markdown file that actually produced
   what they are reading.
4. **Given** the blog and the legal pages (English-only by R5), **When** a reader
   arrives there from a localized page, **Then** navigation stays coherent and
   they are told the page is English-only rather than shown an empty translation.

---

### User Story 4 — A search engine can tell the fifteen versions apart (Priority: P2)

Google, Bing and an AI crawler each index 15 versions of the landing page and the
docs without treating them as duplicates, and each serves the right one to the
right reader.

**Why this priority**: the founder asked for "seo ssr" by name, and localization
without the annotations is 14 duplicate-content copies that can *lose* ranking.

**Independent Test**: fetch any page in any locale and verify the reciprocal
`hreflang` set, `x-default`, self-canonical, `og:locale` and JSON-LD `inLanguage`;
fetch the sitemap and find every localized URL with its alternates.

**Acceptance Scenarios**:

1. **Given** any localized page, **When** its head is read, **Then** it lists an
   `hreflang` alternate for **every locale that actually has that page**, the
   alternates are reciprocal, and `x-default` points at the English URL.
2. **Given** any localized page, **When** its canonical is read, **Then** it
   points at itself, never at the English original.
3. **Given** the sitemap, **When** it is fetched, **Then** it contains every
   localized URL with its `xhtml:link` alternates, and no URL that 404s.
4. **Given** a shared link in any locale, **When** it is unfurled, **Then**
   title, description and `og:locale` are in that page's language.

---

### User Story 5 — Nobody can quietly ship a half-translated site (Priority: P3)

A zero-exit check names every missing translation, every string that changed in
English after it was translated, and which locales have and have not been reviewed
against the R7 standard.

**Why this priority**: 15 locales × ~9,000 words is far past what anyone can eye
by hand, and this project's own record says machine translation without a native
eyeball leaks translator notes and spoken-register mistakes into shipped copy.
The check is the only thing that keeps the fallback in R4 from becoming permanent
and invisible.

**Independent Test**: run the check on a tree with one key removed and one English
string edited; it names both and exits non-zero only when asked to gate.

**Acceptance Scenarios**:

1. **Given** the message store, **When** the check runs, **Then** it prints per
   locale: keys missing, keys stale (English changed since translation), and pages
   falling back.
2. **Given** a new English string added without translations, **When** the check
   runs, **Then** it is listed — and the site still builds (R4), so a missing
   translation never blocks an English fix.
3. **Given** the review record, **When** the check runs, **Then** it states which
   locales are drafted-but-unreviewed and which have passed an R7 review, and the
   site says nothing to the reader that claims otherwise.
4. **Given** a completed R7 review pass on a locale, **When** its findings are
   recorded, **Then** each is severity-ranked with locale, key, current text,
   problem type and fix, and every High and Medium finding is repaired before that
   locale is marked reviewed.
5. **Given** one string reviewed across all 15 locales (the single-string method),
   **When** one locale renders it literally or in the wrong register, **Then** that
   is a finding even though the string is present and grammatical — presence is not
   the bar; reading natively is.

---

### Edge Cases

- **Three Chinese locales.** `zh`, `zh-TW` and `zh-HK` are separate catalogs, and
  `zh-HK` is spoken Cantonese in this project's corpus, not "Traditional with a
  different flag". A reader asking for `zh-MO` or `zh-SG` must land somewhere
  deliberate, and `真正属于你的以太坊钱包` is the **Simplified** line only.
- **`pt-BR` in a URL.** The path segment must match the catalog tag exactly and
  case-insensitively resolve; `/pt/` and `/pt-br/` must not silently become two
  URLs with the same content.
- **The retired tagline lives in six documents**, three of which (`store-listing-copy.md`,
  `100-marketing-leads.md`, `C01-welcome-onboarding.md`) describe surfaces this
  spec does **not** ship (app store listings, the in-app welcome). They must be
  updated as *records* so no one copies a retired line into a store submission —
  changing the store listing itself is a separate act.
- **Prerender volume.** 14 evergreen pages × 15 locales ≈ 210 prerendered pages on
  top of the English blog and legal pages. Build time and the deploy's file count
  are a real limit, not a footnote.
- **None of the 15 locales is RTL**, so no bidirectional layout work is in scope —
  recorded so nobody adds Arabic casually later and finds the CSS unprepared.
- **A locale whose landing page is translated but whose docs are not** must still
  present a coherent site: switcher, sidebar and footer localized, bodies English
  under notice.
- **The on-chain wallet counter and RPC failover** on the landing page must keep
  working identically in all 15 locales, including its number formatting and its
  failure state.

## Requirements *(mandatory)*

### Functional Requirements

**Tagline (US1)**

- **FR-001** — The landing page `<h1>`, `<title>` and `og:title` MUST read
  `An Ethereum wallet you actually own`.
- **FR-002** — The site footer tagline MUST be replaced with a line consistent
  with the new headline; it MUST NOT reassert `Your keys. Your face.`
- **FR-003** — `docs/requirements/A02-brand-voice-honesty-audit-posture.md` and
  `docs/CONTENT-SOURCE-100-CLUES.md` MUST list the new tagline as approved-verbatim
  and mark the old one retired with its retirement date.
- **FR-004** — `docs/marketing/100-marketing-leads.md`,
  `docs/store-submission/store-listing-copy.md` and
  `docs/requirements/C01-welcome-onboarding.md` MUST NOT present the retired line
  as current brand voice.
- **FR-005** — The Simplified Chinese tagline MUST be `真正属于你的以太坊钱包`,
  verbatim.
- **FR-006** — Every other locale's tagline MUST be drafted and MUST be marked as
  awaiting a native review, and MUST NOT be presented as founder-approved until it
  is. Until a locale's tagline is approved, that locale MAY show the English line.

**Locales & routing (US2)**

- **FR-007** — The site MUST support exactly the 15 tags in
  `app-web/vela-wallet`'s `SUPPORTED_LOCALES`, and a check MUST fail if the two
  lists diverge.
- **FR-008** — English MUST remain at unprefixed paths; every other locale MUST be
  served under `/<tag>/…` mirroring the English path.
- **FR-009** — No existing English URL may change, redirect, or lose content.
- **FR-010** — An unsupported or malformed locale prefix MUST 404.
- **FR-011** — Every localized page MUST be rendered server-side (or at build
  time) so its copy is present in the HTML with JavaScript disabled.
- **FR-012** — Every page MUST carry a language switcher listing all 15 locales in
  their own endonyms, preserving the current path.
- **FR-013** — A visitor whose `Accept-Language` prefers a supported non-English
  locale MUST be offered that locale once, in that locale's language, without
  being redirected; the dismissal MUST persist.
- **FR-014** — `<html lang>` MUST match the rendered locale on every page.

**Content & fallback (US3)**

- **FR-015** — The localized surface MUST be: landing page, all 11 docs pages,
  `/about`, `/roadmap`. Blog and legal pages remain English-only.
- **FR-016** — Site translations MUST live in the site's own store, not in
  `rust/crates/vela-core/i18n/`.
- **FR-017** — An untranslated localized page MUST render the English source under
  a notice written in the requested locale, with all surrounding chrome localized.
- **FR-018** — A falling-back page MUST NOT be advertised as a translation to
  search engines.
- **FR-019** — Docs navigation (sidebar groups, prev/next, "edit this page") MUST
  resolve correctly per locale, and the edit link MUST point at the file that
  produced the rendered body.
- **FR-020** — Reaching an English-only page (blog, legal) from a localized page
  MUST tell the reader it is English-only rather than appear broken.

**SEO (US4)**

- **FR-021** — Every localized page MUST emit reciprocal `hreflang` alternates for
  each locale that genuinely has that page, plus `x-default` pointing at English.
- **FR-022** — Every page MUST self-canonicalize.
- **FR-023** — `title`, `meta description`, `og:title`, `og:description` and
  `og:locale` MUST be localized per page.
- **FR-024** — JSON-LD MUST declare the page's language, and the organization's
  description MUST match the locale being served.
- **FR-025** — The sitemap MUST list every localized URL with its alternates and
  MUST NOT list a URL that 404s.

**Quality gate (US5)**

- **FR-026** — A repo check MUST report, per locale: missing keys, stale keys
  (English changed after translation), and pages currently falling back.
- **FR-027** — Missing translations MUST NOT block an English-only change from
  shipping.
- **FR-028** — The review state of each locale (drafted vs. R7-reviewed) MUST be
  recorded in the repo and reported by the check.
- **FR-029** — Every locale MUST be reviewed against the R7 axes — accuracy,
  naturalness, cultural/offence risk, consistency, technical correctness — before
  it is marked reviewed; findings MUST be severity-ranked and every High and
  Medium finding MUST be repaired.
- **FR-030** — Translations MUST NOT preserve English sentence structure or word
  count at the cost of naturalness; each locale's copy MUST follow that language's
  own product-writing conventions, including headline and CTA length.
- **FR-031** — Placeholders, links, markdown and inline HTML MUST survive
  translation unchanged, and numbers, dates and currency MUST render by the
  locale's own conventions.
- **FR-032** — The honesty posture (alpha status, "no third-party audit", "we
  architecturally can't access your keys") MUST NOT be softened, strengthened or
  dropped in any locale.

### Key Entities

- **Locale** — one of 15 tags; carries an endonym, a URL segment, an
  `og:locale` value, and a review state.
- **Message catalog** — the site's own per-locale store of UI chrome strings
  (nav, CTAs, switcher, fallback notices, footer, docs chrome).
- **Localized document** — a per-locale markdown body for one of the 11 docs
  pages, plus its title and description; absent ⇒ fallback (R4).
- **Page translation record** — which locale/page pairs are genuinely translated;
  drives both `hreflang` emission (FR-021) and the fallback notice (FR-017).

## Success Criteria *(mandatory)*

- **SC-001** — A reader landing on the English page reads
  `An Ethereum wallet you actually own` as the first line, and the retired tagline
  appears nowhere on the site.
- **SC-002** — Every one of the 15 locales returns a fully rendered landing page,
  in that language, with JavaScript disabled.
- **SC-003** — A reader can switch language from any page and stay on the same
  page, never being bounced to the home page.
- **SC-004** — Every localized URL declares its alternates, and an SEO crawl of
  the deployed site reports zero duplicate-content or missing-`hreflang`-return-tag
  errors across the 15 versions.
- **SC-005** — No existing English URL changes; a crawl before and after the change
  finds the same English paths, all still 200.
- **SC-006** — A reader on a not-yet-translated page always sees, in their own
  language, why the text below is English — and never sees a mixed-language page.
- **SC-007** — The Simplified Chinese site can be read end to end by the founder
  without meeting an English word on the localized surface.
- **SC-008** — Adding a new English sentence and running the check names all 14
  locales that now need it, in under a minute, and the English change still ships.
- **SC-009** — The landing page's on-chain wallet counter reads and formats
  correctly in all 15 locales, and degrades the same way when every RPC fails.
- **SC-010** — A native speaker reading any locale marked reviewed cannot tell it
  was translated: no English syntax, no register mismatch, no term used two ways —
  the R7 bar, not "all locales say the same words".
- **SC-011** — Zero High or Medium R7 findings remain open on any locale marked
  reviewed, and every open finding is listed with its severity.

## Assumptions

1. **`pt` means `pt-BR`.** The founder said 葡萄牙语; the wallet corpus carries
   `pt-BR`. R2 makes the site match the corpus, so the URL segment is `pt-BR` and
   the register is Brazilian.
2. **Japanese and Portuguese taglines are drafts.** Proposed: ja
   `本当にあなたのものになるイーサリアムウォレット`, pt-BR
   `Uma carteira Ethereum realmente sua`. Per FR-006 these — and the other eleven —
   are unapproved until read by someone who speaks the language. The founder was
   asked and answered about the locale list instead; the question stands open.
3. **Translations start machine-assisted, then get an R7 pass.** ~9,000 evergreen
   words × 14 target locales is beyond hand translation here, so the first draft is
   produced mechanically and then reviewed against the founder's methodology
   (R7 / FR-029) locale by locale. The project's own history — translator notes
   leaking into shipped strings, `zh-HK` rendered as spoken Cantonese — is why the
   review is a requirement and not a claim of quality made up front.
4. **The subtitle keeps the mechanism.** "no seed phrase / passkey / no company
   that can lock you out" stays on the page; only the headline changes what it
   leads with.
5. **The hero illustration stays.** A face turning into a key still illustrates the
   subtitle; only its label wording is in scope (US1 scenario 5).
6. **The app's own UI is out of scope.** This spec changes the marketing site and
   the repo's brand-authority documents. The in-app welcome copy (C01) and the app
   store listings are recorded, not shipped, here.
7. **No new locale is invented.** If a reader asks for `zh-SG`, `pt-PT` or `es-ES`,
   they resolve to the nearest supported catalog by the same rules the wallet
   already uses; the site does not grow a sixteenth catalog.

## Out of Scope

- Translating the blog or the legal pages (R5).
- Changing App Store / Play listing copy, or the in-app onboarding copy.
- Adding locales beyond the wallet's 15, or any RTL language.
- Localized OG images (text-in-image); the OG image stays language-neutral.
- Any change to `rust/crates/vela-core/i18n/` (R6).
