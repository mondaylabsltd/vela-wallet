# Feature Specification: i18n / L10n in the shared Rust core

**Feature Branch**: `004-rust-i18n`

**Created**: 2026-07-31

**Status**: Implemented 2026-07-31 — all 73 tasks complete.

The corpus replays green on **all four surfaces** (SC-002): 18,975 cases in Rust,
40,470 through the shipped web artefact, 40,425 through the Swift bindings and
40,425 through Kotlin. `verify:i18n` adds 67,115 live comparisons against
`i18next@26.3.1` with zero divergences. SC-005 measured at a **92.0%** residency
reduction; SC-007 met at **2 allocations** and 0.30–0.63 µs per resolution; SC-009
flat across 1,000,000 distinct keys; wasm **652,393** bytes against a 1,000,000
ceiling.

**Out of scope by design**: the React Native runtime migration (FR-028). The app
consumes the generated `resources.ts` and is otherwise untouched — 1,437 jest tests
and `tsc --noEmit` pass unchanged.

**Input**: User description: "Reimplement the `i18next` JS library's functionality
completely and exactly in Rust, as a utility inside `rust/crates/vela-core`. It must
be a 100% faithful reproduction. First survey Vela Wallet's current i18n/L10n level —
whatever exists today must keep working; adopting this library must not break
anything. What we're really building is an efficient, fully-featured i18n/L10n
toolkit (L10n today means date, time, currency and number formats; add anything else
important that's missing). Later we move the translation JSON into
`rust/crates/vela-core` so that changing one place changes every platform. The Rust
library will be used from app-ios, app-web, app-android and app-desktop. Follow the
speckit process. Optimise for performance and memory — no memory blow-ups, no slow
paths. Keep the API close to `i18next`; variations and omissions are allowed, but the
existing translation files under `src/i18n/locales` must work **without a single line
changed**." Follow-up: "Each platform should load translation files **on demand**,
rather than loading every language at once."

## Why

Vela Wallet ships **15 languages** and **16,833 translated strings**. Every one of
them is a promise to a user who is about to move money: the difference between
"Send" and "Sign" is the difference between a transfer and an approval. Localization
in a wallet is not decoration — it is part of the surface a user reads before
authorising an irreversible action.

Today that surface is produced by three unrelated systems, and two of them are
already wrong:

1. **UI strings** come from `i18next@26.3.1` (`src/i18n/`). It resolves *every*
   plural through `Intl.PluralRules` — and **Hermes ships `Intl` without
   `PluralRules`**. i18next catches the `TypeError` and silently substitutes an
   English-shaped stub (`count === 1 ? 'one' : 'other'`). No warning is logged,
   because the warning only fires when `Intl` is entirely absent. **On iOS and
   Android today, Russian renders the wrong grammatical form** — `21 получателей`
   where CLDR requires `21 получатель`, across five base keys and 18 of the 675
   plural resolutions we measured. Jest runs on full-ICU Node, so
   the test suite reproduces the *correct* behaviour and can never catch the device
   bug. The comment at `src/i18n/index.ts:14-16` asserting that i18next's core
   "doesn't depend on Hermes' incomplete `Intl`/ICU data" is factually false, and
   that false belief is why the defect shipped.
2. **Number / date / time formats** come from `src/services/locale-format.ts`, a
   deliberately `Intl`-free preset system. It is correct about *why* it avoids
   `Intl`, but it has drifted: three independent digit-grouping implementations, two
   rival `formatTokenAmount` signatures, five hardcoded-`$` USD formatters, and a
   hardcoded `en-US` clock.
3. **Currency** comes from `src/services/currency.ts`, whose `formatFiat` is
   unconditionally `symbol + number` with no space and a binary 2-or-0 decimal rule.
   That is wrong for **21 of the 137 catalog currencies** — 6 that CLDR gives 3
   decimals (KWD, BHD, OMR, JOD, TND, LYD) and 15 that CLDR gives 0 but the app
   renders with 2 (LBP, PKR, MMK, LAK, COP, ALL, AFN, IRR, IQD, SYP, YER, SOS, BIF,
   MGA, SLL) — and structurally wrong for **7 of the 15 shipped UI locales**: vi, ru,
   fr, it and de place the symbol *after* the amount with U+00A0; pt-BR places it
   *before* but still separated by U+00A0; and `id` gains the same separator through
   CLDR `currencySpacing`, which inserts U+00A0 whenever the symbol is alphabetic
   (`Rp 1.235`). es-MX is **not** among them — measured with its own currency it is
   `$1,234.50`, already correct.

Meanwhile the platform story is diverging exactly the way feature 001 was built to
prevent. `app-ios`, `app-android`, `app-web` and `app-desktop` are scaffolds today,
but each will need the same 16,833 strings. Four copies of a translation corpus is
four opportunities for a wallet to say "Send" on one device and "Sign" on another.

There is also a cost being paid today for no benefit: **all 15 locales are loaded on
every launch**, on every platform. `src/i18n/resources.ts` statically imports all 240
JSON files and spreads them into one object, so a user who reads only Japanese still
carries 990,499 bytes of strings — roughly 15× what they need. Loading on demand is
therefore not a nice-to-have added late in this spec; it is the difference between a
shared core that improves every platform and one that merely relocates a problem.

The requester's framing — *change it in one place, every platform updates* — is the
right one. This feature makes `vela-core` that one place: the corpus lives in the
crate, the resolver lives in the crate, and every platform artefact is **generated**
from it, including the TypeScript resources file the current React Native app already
consumes.

Porting i18next is specification-worthy rather than an afternoon's work because its
observable behaviour is much larger than its documented behaviour, and the app
depends on the undocumented parts:

- The candidate-key list is built by `push` and consumed by `pop`, so the **actual**
  try order is the reverse of the source order: `key_ctx_zero`, `key_ctx_plural`,
  `key_ctx`, `key_zero`, `key_plural`, `key`. A bare key is a *last* resort, after
  the plural-suffixed lookup — which is the only reason 11 keys per locale that
  interpolate `{{count}}` without plural siblings render at all.
- The plural suffix is **recomputed per fallback language code**, not once per call.
  Under `lng='fr'` with `fallbackLng:'en'`, a `count` of 1,000,000 looks up `_many`
  in `fr`, misses, then looks up `_other` in `en` — which is why four locales
  currently leak English for large numbers.
- A missing interpolation variable is **left on screen as the literal `{{var}}`**
  (`skipOnVariables: true`), but a variable that is an own property of the options
  object — even `undefined` — becomes `''`. Those are different code paths with
  different user-visible results.
- Substitution is `String.replace(match[0], value)` — **first occurrence**, not
  splice-at-index — which is observable whenever a placeholder repeats.
- A `count` passed as a **string** silently disables plural resolution and returns
  the raw key.

A port that is 99% right produces a wallet that is subtly ungrammatical in some
languages and shows raw template syntax in others. Neither failure has a test that
catches it today.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One exact i18next implementation (Priority: P1)

As the wallet's maintainer, I have a translation resolver in `vela-core` that is
proven byte-identical to `i18next@26.3.1` across a generated conformance corpus, so
that every platform can render strings from one verified implementation instead of
maintaining a JS copy plus native reimplementations.

**Why this priority**: Nothing else in the feature is safe without it. It also
delivers value standalone: the corpus becomes the oracle any future platform
implementation must satisfy, and the Rust suite immediately cross-checks the JS
library the app ships today.

**Independent Test**: Run the Rust test suite. Every case in the committed
conformance corpus — generated by driving the real installed `i18next@26.3.1` over
the real 15-locale corpus plus a synthetic behaviour matrix — produces byte-identical
output. No app change required.

**Acceptance Scenarios**:

1. **Given** the conformance corpus (every key × every locale, plus the synthetic
   matrix of counts, contexts, `defaultValue` shapes, missing/`undefined`/object
   interpolation values, nesting, array keys, branch nodes and malformed keys),
   **When** each case is replayed through the Rust resolver, **Then** the returned
   string is byte-identical to i18next's.
2. **Given** a key that resolves through the fallback chain, **When** the active
   language is `fr` and the count selects a CLDR category `fr` does not supply,
   **Then** the Rust resolver recomputes the suffix for `en` and returns the same
   English string i18next returns — the defect is reproduced exactly, not silently
   repaired.
3. **Given** an input i18next handles by throwing (`t([])`) or by returning a
   diagnostic string (`t('home')` on a branch node), **When** the same input reaches
   the Rust resolver, **Then** it returns a typed error or the identical diagnostic
   string, and never panics.

---

### User Story 2 - Correct plurals on every platform (Priority: P1)

As a Russian-speaking user on iOS or Android, the wallet says "21 получатель", not
"21 получателей", so the app reads like it was written by someone who speaks my
language.

**Why this priority**: Equal to US1 because it is the concrete user-visible defect
that justifies the port. It is also the one thing a Rust engine gets *for free* that
no amount of JS work gets cheaply: CLDR plural rules compiled in, with no dependency
on the host engine's ICU build.

**Independent Test**: Replay the 10 affected Russian keys at counts
`{0,1,2,5,21,101,1000000}` through the Rust engine and assert they match the
full-ICU (`MODE A`) oracle, then confirm the same values on a device build where
`Intl.PluralRules` does not exist.

**Acceptance Scenarios**:

1. **Given** `lng='ru'` and `count=21`, **When** `send.recipientCount` is resolved,
   **Then** the result is `21 получатель` on **every** platform, including builds
   with no `Intl.PluralRules`.
2. **Given** any of the 15 shipped locales, **When** the engine reports the plural
   categories it supports, **Then** the set equals the CLDR cardinal category set
   for that locale — `[other]` for zh/zh-TW/zh-HK/ja/ko/vi/id, `[one, other]` for
   en/de/tr, `[one, many, other]` for fr/it/es-MX/pt-BR, `[one, few, many, other]`
   for ru.
3. **Given** the 16 missing CLDR `many` entries in fr/it/es-MX/pt-BR, **When** a
   count of 1,000,000 selects `many`, **Then** the locale's own translation is
   returned — not the English fallback it returns today. These entries are added by
   FR-017 precisely because MODE A would otherwise turn 16 correct localised strings
   into English.
4. **Given** the remaining known corpus defects (28 dead `_one` entries in the seven
   `other`-only locales, the 10 en-only keys, the `contacts.groupMembers` shape),
   **When** those keys are resolved, **Then** the engine reproduces today's rendered
   output exactly and a separate lint reports them — the port fixes the *engine*,
   not the *content*.

---

### User Story 3 - The crate is the single source of truth (Priority: P2)

As the wallet's maintainer, the 240 translation files live in `vela-core` and every
platform artefact is generated from them, so that adding a string is one edit in one
place and no platform can drift.

**Why this priority**: This is the requester's stated goal, but it depends on P1
being proven first — moving the corpus before the resolver is verified would relocate
strings without gaining confidence in how they render.

**Independent Test**: Move the corpus, run the generator, and confirm
`src/i18n/resources.ts` and every locale JSON are byte-identical to what the app
consumed before the move. `git diff --exit-code` after a regeneration is the gate.

**Acceptance Scenarios**:

1. **Given** the corpus relocated to `rust/crates/vela-core/i18n/locales/`, **When**
   the generator runs, **Then** the emitted `src/i18n/resources.ts` produces a
   resource object deep-equal to today's, and the React Native app renders every
   screen unchanged.
2. **Given** a contributor edits one string in one locale file, **When** they run
   the generator, **Then** the TypeScript resources, the Rust catalog tables and the
   conformance corpus all update, and CI fails if any of them was left stale.
3. **Given** the 39 values whose leading or trailing whitespace is load-bearing
   (sentence fragments concatenated at render time) and the 120 values containing
   escaped newlines, **When** the corpus round-trips through the generator, **Then**
   every byte is preserved — no trimming, no normalisation, no re-encoding.

---

### User Story 4 - Only the language I use gets loaded (Priority: P2)

As a user who reads one language, my device loads that language's strings — not all
fifteen — so the app starts faster and uses less memory, and switching language
loads the new one on demand rather than paying for all of them up front.

**Why this priority**: It is a requester requirement and it is the main reason a
shared core is better than fifteen static imports. It is also the constraint that
decides the API shape: an engine that can only be constructed with every locale
present cannot be retrofitted for lazy loading later.

**Independent Test**: Construct the engine with only `ja` and the `en` fallback
resident, resolve the full key set, then switch to `de` and assert `ja` can be
released. Measure resident catalog bytes at each step.

**Acceptance Scenarios**:

1. **Given** a cold start in Japanese, **When** the engine is initialised, **Then**
   only the `ja` catalog (75,608 bytes) and the `en` fallback (59,737 bytes) are
   resident — not the full 990,499-byte corpus.
2. **Given** the user switches to German, **When** the new catalog is requested,
   **Then** the engine loads `de` on demand, keeps `en`, and the previously active
   catalog can be dropped without invalidating the engine or leaking memory.
3. **Given** a platform that cannot load lazily (a fully static desktop build),
   **When** it compiles every locale in, **Then** the same API serves it — lazy
   loading is a host capability the engine permits, never one it requires.
4. **Given** the web artefact, **When** it is built with the default locale set,
   **Then** it stays under the 1,000,000-byte ceiling enforced by
   `rust/scripts/build-web.mjs`.

---

### User Story 5 - One correct formatting layer (Priority: P3)

As a user in any of the 15 shipped languages, amounts, dates, times and fiat values
are formatted the way my locale actually writes them, and identically on every
device.

**Why this priority**: The formatting layer is where the current implementation is
demonstrably wrong (currency) and where it has silently drifted (three grouping
implementations). It is separable from US1–US4 — the resolver ships first — but it is
the half of "i18n/L10n toolkit" the requester asked for that does not exist anywhere
today.

**Independent Test**: Replay `locale-format.ts`'s documented outputs through the Rust
formatter and assert byte-identity; then replay a CLDR currency matrix and assert the
21 wrong-decimal and 6 wrong-placement cases now match CLDR.

**Acceptance Scenarios**:

1. **Given** each of the 4 number presets, 5 date presets and 2 time presets, **When**
   the documented sample values are formatted, **Then** the Rust output is
   byte-identical to `locale-format.ts` — including Indian 2-3 grouping, the
   trailing-zero trimming rule, and the compact K/M/B/T tiers.
2. **Given** a fiat amount in KWD (3-decimal) or in EUR under `fr`, **When** it is
   formatted, **Then** the fractional digits and the symbol placement follow CLDR —
   `1,234.500 د.ك` and `1 234,50 €` with U+00A0 before the symbol — rather than
   today's `د.ك1234.50` / `€1234.50`. The *grouping* separator remains whatever the
   user's number preset specifies (FR-020), so the core composes CLDR placement with
   the user's separators rather than adopting CLDR's wholesale.
3. **Given** a relative timestamp, a date header, a weekday name and a day period,
   **When** they are produced, **Then** they come from compiled-in locale data with
   no host-`Intl` call reachable from the core, so that
   `src/services/activity.ts:121`'s `toLocaleDateString` and `formatTime`'s hardcoded
   `AM`/`PM` *can* be retired — retiring them in the app is follow-up (FR-028).
4. **Given** a user-controlled string (contact alias, ENS name, dApp origin)
   interpolated into a translated label, **When** the label is rendered, **Then** the
   substituted value is wrapped in Unicode isolate marks so a right-to-left name
   cannot reorder the surrounding sentence.

---

### User Story 6 - Cheap enough for a full screen of text (Priority: P3)

As a user on a low-end phone, switching language re-renders the app without a stall,
and the wallet's memory does not grow with the number of strings I have viewed.

**Why this priority**: Correctness first, but a resolver that is exact and slow is
not shippable when ~500 strings resolve per frame during a language switch. The
requester called this out explicitly: optimise time and memory, no blow-ups.

**Independent Test**: Benchmark resolution in isolation and assert the per-call
budget; assert the engine holds no per-key state, so resolving unboundedly many
distinct keys has flat memory.

**Acceptance Scenarios**:

1. **Given** a loaded catalog, **When** a key with two interpolations and a plural is
   resolved, **Then** it completes within the per-call budget (SC-007) with a bounded,
   documented allocation count.
2. **Given** a full application screen's worth of resolutions (≈500 keys), **When**
   they run in sequence, **Then** the total stays inside a single frame budget.
3. **Given** one million resolutions over distinct keys, **When** memory is sampled,
   **Then** resident memory attributable to the engine is flat — no per-key cache
   that can grow without bound.

---

### Edge Cases

- **`Intl.PluralRules` is absent on the host.** This is the norm, not the exception:
  Hermes 0.14.1 as shipped by React Native 0.83.6 compiles `Intl` with Collator,
  DateTimeFormat and NumberFormat only. The Rust engine MUST carry its own CLDR
  plural rules and MUST NOT consult any host facility.
- **A `count` that is not a number.** i18next's `t('key', {count: '3'})` returns the
  raw key, because `Intl.PluralRules.select` is never reached. The port must
  reproduce that, not "helpfully" coerce.
- **A `count` that is fractional or negative.** `1.5` selects `other` in en; `-1`
  selects `one`. Both are reachable from real call sites that pass computed values.
- **Both `count` and a same-named variable.** `ConfirmStep.tsx:247` passes
  `{count, n}` together. `count` wins the suffix; `n` is inert for pluralisation but
  still interpolates.
- **A missing interpolation variable.** Renders as the literal `{{ago}}` on screen.
  This is a rendering bug in the app, but it is *current behaviour*, and the port must
  reproduce it so that fixing it stays a separate, reviewable change.
- **Language-code normalisation is asymmetric.** `'ZH-tw'` normalises to `zh-TW`, but
  bare `'ZH'` does **not** normalise and falls through to `en`; `'zh_TW'` (underscore)
  silently degrades to `zh`, turning Traditional into Simplified.
- **`load: 'currentOnly'` means `zh-TW` never reads `zh`.** A key present in `zh` but
  not `zh-TW` resolves to the **English** value, skipping Simplified entirely.
- **A branch node is not an error.** `t('home')` returns the literal diagnostic string
  `key 'home (en)' returned an object instead of string.` — the port must emit that
  exact text, byte for byte.
- **`t([])` throws in i18next.** The port must return a typed error, never panic.
- **`defaultValue: null` is ignored; `defaultValue: ''` is honoured.** Two adjacent
  falsy values with opposite outcomes.
- **Flat dotted keys still resolve.** i18next's `deepFind` fallback means a JSON key
  literally named `"a.b.d"` resolves for `t('a.b.d')` even when `a` is a nested
  object. The corpus has none today, but the behaviour is part of the contract.
- **`$t()` nesting is active** with this configuration and is reachable from
  dApp-supplied text used as a translation key (`tx.intent`). See FR-003 and FR-020.
- **A key is requested for a locale that is not resident.** With on-demand loading
  this is reachable by construction — mid-flight language switches, a race between a
  render and a catalog load. The engine must define this (fall back to `en`, or a
  typed "catalog not loaded" error), never resolve against a half-loaded catalog.
- **The wasm size ceiling is a hard CI gate.** `rust/scripts/build-web.mjs:42` throws
  above 1,000,000 bytes; the module is 530,780 today. The full corpus is 990,499
  bytes. Any design that embeds all 15 locales into the web artefact fails the build.
- **Whitespace and newlines in values are load-bearing.** 39 values are sentence
  fragments whose leading/trailing spaces are concatenated at render time (and
  zh/zh-TW/zh-HK deliberately omit them), and 120 values embed escaped newlines. No
  trimming, ever.

## Requirements *(mandatory)*

### Functional Requirements

#### The resolver

- **FR-001**: The core MUST implement i18next's translation resolution as installed
  at `i18next@26.3.1`: key/namespace extraction, nested-object traversal with the
  `deepFind` flat-key fallback, the LIFO candidate-key order, context and plural
  suffixing, the per-language-code plural-suffix recomputation, and the fallback
  language chain under `load: 'currentOnly'` with `supportedLngs`.
- **FR-002**: The core MUST implement i18next's interpolation: the `{{var}}` and
  `{{- var}}` forms with unescape evaluated first, dotted variable paths,
  first-occurrence replacement semantics, the `skipOnVariables: true` distinction
  between an absent variable (left as `{{var}}`) and an own-property `undefined`
  (rendered as `''`), and JavaScript's string-coercion rules for numbers, booleans,
  `null`, arrays and objects.
- **FR-003**: The core MUST implement `$t()` nesting, including inherited options,
  the recursion limit, and the behaviour when the nested key is missing.
- **FR-004**: The core MUST implement the `defaultValue` precedence chain, including
  `defaultValue_<category>` variants keyed off the *active* language's suffix, the
  `null`-ignored / `''`-honoured asymmetry, and the rule that a found translation
  always wins.
- **FR-005**: For every case in the conformance corpus, the core's output MUST be
  byte-identical to `i18next@26.3.1`'s output when that library runs with a complete
  `Intl.PluralRules` implementation (**MODE A**). MODE A is the conformance target;
  the degraded Hermes behaviour (**MODE B**) is a defect being closed, not a contract.
- **FR-006**: The core MUST carry its own CLDR cardinal plural rules for the 15
  shipped locales and MUST NOT depend on any host `Intl`, ICU build or system locale
  facility. Adding a locale MUST NOT require a new third-party dependency.
- **FR-007**: The core MUST additionally offer a **legacy-compatible** plural mode
  that reproduces i18next's `dummyRule` (`count === 1 ? "one" : "other"`), so that
  the parity claim in FR-005 is provable against the behaviour currently shipping on
  native, and so any divergence is enumerable rather than assumed.
- **FR-008**: Where i18next throws, the core MUST return a typed error; where
  i18next returns a diagnostic string, the core MUST return that exact string. The
  core MUST NOT panic on any input, including malformed keys, empty key arrays,
  cyclic nesting and non-UTF-8-representable option values.

#### The corpus and on-demand loading

- **FR-009**: The 240 translation files MUST be consumable **byte-for-byte
  unchanged**. No re-indentation, no key reordering, no whitespace trimming, no
  Unicode normalisation, no re-encoding.
- **FR-010**: `rust/crates/vela-core/i18n/locales/` MUST become the single source of
  truth for translation content. Every downstream artefact — the TypeScript resources
  the React Native app imports, the Rust catalog tables, and the conformance corpus —
  MUST be **generated** from it by a committed, re-runnable generator, and CI MUST
  fail on drift.
- **FR-011**: The generated TypeScript resources MUST produce a resource object
  deep-equal to today's, so that the React Native app, its 1,029 call sites, its 92
  `useTranslation()` hooks and its `i18next.d.ts` key typing continue to work with
  **zero** source changes.
- **FR-012**: **Catalogs MUST load on demand, one locale at a time.** The engine MUST
  be constructible with a single locale resident and MUST accept additional catalogs
  after construction. Requiring every locale up front is explicitly forbidden — it is
  what the current `resources.ts` does and what this feature exists to end.
- **FR-013**: The engine MUST allow a resident catalog to be **released**, so a
  language switch does not monotonically grow memory. The active locale and the `en`
  fallback are the only catalogs the engine may require to be resident.
- **FR-014**: The generator MUST additionally emit **per-locale artefacts** suitable
  for lazy loading on each platform — one module per locale for the JS/TS route, one
  compiled-in table per locale behind a cargo feature for the Rust route — so a
  platform can fetch or link exactly the language it needs. The default cargo feature
  set MUST keep the web artefact under the 1,000,000-byte ceiling.
- **FR-015**: The core MUST also accept a catalog supplied at runtime from JSON
  bytes, so a host can load a locale the binary was not compiled with.
- **FR-016**: Resolution against a locale that is not resident MUST **fall back to
  the pinned `en` catalog** and MUST NOT panic, block, or read a partially-loaded
  catalog. This is reachable by construction under on-demand loading (a render racing
  a catalog load, a mid-flight language switch), so it is a normal path, not an error
  path. The engine MUST expose which locales are resident so a host can distinguish
  "fell back" from "translated".
- **FR-017**: The **16 missing CLDR `many` entries** in es-MX, pt-BR, fr and it MUST
  be added to the corpus as part of this feature. They are not merely a pre-existing
  defect: because MODE A selects `many` where the shipped native build selects
  `other`, adopting this engine without them would **regress 16 strings from correct
  localised text to English** at large counts (measured — see SC-003). In all 16 cases
  the correct `many` wording for these phrases is identical to the existing `_other`
  value, so the change is mechanical and reviewable.
- **FR-018**: The remaining known corpus defects — 10 en-only keys leaking English in
  13 locales, 28 dead `_one` entries, the inconsistent `contacts.groupMembers` shape,
  and 11 keys per locale using `{{count}}` with no plural siblings — are
  **pre-existing content bugs and are out of scope**. The port MUST reproduce today's
  rendered output for each. A lint MUST report them so they can be fixed separately,
  and MUST fail CI on *new* occurrences, including any future missing CLDR category.

#### The formatting layer

- **FR-019**: The core MUST reproduce `src/services/locale-format.ts`'s contract
  byte-identically: 4 number presets, 5 date presets, 2 time presets, Indian 2-3
  grouping, the trailing-zero trimming rule, the compact K/M/B/T tiers, the
  magnitude-dependent token-amount precision ladder, and `parseLocaleNumber`'s
  normalisation (including Arabic-Indic digit mapping).
- **FR-020**: The core MUST format fiat amounts per CLDR:
  - **Fractional digits** — 3 for the 6 codes CLDR gives 3 (KWD, BHD, OMR, JOD, TND,
    LYD); 0 for **every** CLDR zero-decimal code in the catalog (30 of the 137, of
    which the app currently gets 15 right and 15 wrong); 2 otherwise.
  - **Symbol placement and spacing** — symbol-after separated by U+00A0 for vi, ru,
    fr, it and de; symbol-before separated by U+00A0 for pt-BR; symbol-before with
    no separator for the remaining 9 (en, zh, zh-TW, zh-HK, ja, ko, id, tr, es-MX).
    On top of that, CLDR `currencySpacing` inserts U+00A0 between an **alphabetic**
    symbol and the digits even in the no-space class — live for Vela, since the
    catalog supplies `CHF`, `Rp`, `zł`, `kr` and any code with no glyph. The pattern
    space is **always U+00A0**, never U+0020 and never U+202F.
  Number *grouping* MUST remain under the user's explicit preset preference — the
  preset system is a deliberate product decision, not an accident — so the core MUST
  compose CLDR currency placement with the user's chosen separators rather than
  substituting CLDR's.
- **FR-021**: The core MUST provide relative-time formatting, date headers
  (today / yesterday), weekday names and day periods from compiled-in locale data,
  so that the last host-`Intl` call (`src/services/activity.ts:121`) and the
  hardcoded English `AM`/`PM` in `formatTime` **can** be removed. Performing that
  removal in the app is follow-up work, not part of this feature — see FR-028.
- **FR-022**: The core MUST provide bidirectional isolation for interpolated values:
  a documented option that wraps substituted text in `U+2068`/`U+2069` so
  user-controlled content (contact aliases, ENS names, dApp origins, Arabic-script
  currency symbols) cannot reorder the surrounding sentence. It MUST default to
  **off** so FR-005 byte-parity holds, and be explicitly opt-in per call site.
- **FR-023**: The core MUST expose the text direction (LTR/RTL) for a locale, so
  platforms can drive layout mirroring from the same source of truth.

#### Cross-cutting

- **FR-024**: The engine MUST hold no unbounded mutable state. Resolving unboundedly
  many distinct keys MUST NOT grow memory. Any cache MUST be explicitly bounded and
  documented, or live in the caller.
- **FR-025**: The public API MUST stay recognisably close to i18next's (`t`,
  `exists`, `change_language`, `language`, `languages`, `dir`, `get_fixed_t`), so the
  1,029 JS call sites map across without redesign.
- **FR-026**: The core MUST include property-based tests asserting that no input
  panics, that output is stable across repeated calls, and that every corpus key in
  every locale resolves to a non-empty string or a documented fallback.
- **FR-027**: The conformance corpus MUST be regenerable from a clean checkout with
  documented commands and MUST be byte-identical on re-run, so any diff is a real
  behaviour change.
- **FR-028**: The React Native application MUST NOT be migrated to the Rust engine in
  this feature. Its only permitted change is consuming the **generated** resources in
  place of the hand-maintained `resources.ts`. Runtime migration, and the switch from
  static to lazy imports on the RN side, are follow-up work tracked separately.

### Key Entities

- **Catalog**: one locale's complete key→value map, plus its CLDR plural categories
  and text direction. Loaded on demand; compiled in behind a per-locale feature, or
  parsed from JSON at runtime.
- **Catalog registry**: the set of currently resident catalogs, mutable at runtime by
  load and release, with the `en` fallback pinned.
- **Key path**: a `.`-separated path into the nested JSON, optionally prefixed by a
  namespace and `:`. 1,141 distinct paths exist, at a maximum depth of 3.
- **Candidate key**: one of up to six suffixed forms tried in LIFO order for a single
  `(key, language)` pair.
- **Resolve hierarchy**: the ordered language codes tried for one lookup. Under
  `load: 'currentOnly'` this is always `[active, "en"]`, or `["en"]` when active is
  `en`.
- **Plural category**: a CLDR cardinal category (`zero`/`one`/`two`/`few`/`many`/
  `other`) selected from a count, compiled in per locale.
- **Interpolation options**: the 42 option names the app actually passes —
  `defaultValue`, `count`, and 40 interpolation variables.
- **Format preset**: one of the 4 number / 5 date / 2 time presets the user picks in
  Settings, each with an `auto` variant resolved from the device.
- **Core error**: the existing flat `CoreError`, extended with the classification for
  i18n inputs the engine cannot resolve and for catalogs that are not resident.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the conformance corpus passes with zero byte differences
  against `i18next@26.3.1` running in MODE A. The corpus is the **full 1,141 × 15 =
  17,115 key/locale cross-product** — not the sum of per-locale leaf counts,
  because the difference is *exactly* the set that exercises
  `fallbackLng: 'en'` and is therefore the highest-value coverage in the suite —
  plus a behaviour matrix over counts `{-1, 0, 1, 1.5, 2, 5, 21, 101, 1e6, NaN,
  "3", 5n}`, context, `defaultValue` shapes, absent/`undefined`/object/array
  interpolation values, nesting, array keys, branch nodes, malformed keys and
  language-code normalisation, and a plural suite run in both MODE A and MODE B.
  Total **18,975 cases**.
- **SC-002**: The corpus produces byte-identical results on Rust, Kotlin, Swift and
  wasm.
- **SC-003**: **Against the web/Jest baseline (MODE A), zero rendering changes.**
  Replaying every `t()` call site's key set produces byte-identical strings. Against
  the *native* baseline (MODE B, what iOS and Android render today) the change is
  bounded, enumerated and entirely in the plural surface: over the 5 plural base keys
  × 15 locales × 9 counts (675 resolutions), exactly **42 change**, in ru (18),
  pt-BR (8), fr (8), es-MX (4) and it (4). Every one of the 42 MUST be listed in the
  PR with its before/after string. **Zero of them may be a regression** — which is
  what FR-017 exists to guarantee, since without the 16 added `many` entries, 16 of
  the 42 would replace correct localised text with English.
- **SC-004**: The Russian plural keys — 5 base keys (`send.recipientCount`,
  `send.batchApply`, `send.batchRejected`, `contacts.sends`, `contacts.groupMembers`)
  whose `_few`/`_many` forms are unreachable on native today — render their
  CLDR-correct form on a build with no `Intl.PluralRules`. 18 of the 675 measured
  resolutions change; each is enumerated with its count, before string and after
  string.
- **SC-005**: **Cold start in any single language loads at most that language plus
  the `en` fallback.** Measured resident catalog bytes for `ja` were ≤ 135,345
  (75,608 + 59,737), against 990,499 then — a ≥86% reduction. After a switch to
  `de` and a release of `ja`, resident bytes return to at most `de` + `en`.

  *2026-09-23, owner:* the corpus grew into that measurement, and the test's
  constant was doubled to 270,690. The requirement is the sentence above — one
  language plus `en`, never fifteen — and it is unchanged; the number is now a
  guard against bloat, which a locale pair reaches only by doubling. The
  reduction it still permits is 72.7%.
- **SC-006**: The generated TypeScript resources produce a resource object deep-equal
  to the hand-maintained one they replace, and regenerating the corpus leaves
  `git diff --exit-code` clean.
- **SC-007**: A single resolution — key lookup, plural selection and two
  interpolations — completes in **under 1 µs** on a development machine, with **at
  most two heap allocations**. A 500-key screen resolves in under 0.5 ms. Loading a
  catalog completes in under 5 ms.
- **SC-008**: The web artefact stays under the 1,000,000-byte ceiling with the
  default locale set compiled in, and the measured delta from the i18n module is
  recorded.
- **SC-009**: Memory attributable to the engine is flat across one million
  resolutions over distinct keys, and returns to baseline after every loaded catalog
  is released.
- **SC-010**: The formatting layer reproduces every documented `locale-format.ts`
  output byte-identically; a CLDR currency matrix over all 137 catalog codes × the 15
  shipped locales confirms the **21 wrong-decimal** codes and the **7 wrong-placement**
  locales are corrected; and relative time, date headers, weekday names and day
  periods are produced for all 15 locales with no host-`Intl` call reachable from the
  core.
- **SC-011**: `text_direction` returns `ltr` for all 15 shipped locales and `rtl` for
  a representative RTL tag (`ar`, `he`, `fa`), and bidi isolation — when explicitly
  enabled — wraps every interpolated value in `U+2068`/`U+2069` while leaving output
  byte-identical when disabled, so FR-005 parity is unaffected.
- **SC-012**: A contributor can regenerate the catalogs, the TypeScript resources and
  the conformance corpus, and run the full suite, with documented commands on a clean
  checkout.

## Assumptions

- **Technology mandate (from the requester, fixed)**: the implementation language is
  Rust; it lives in `rust/crates/vela-core` (the requester wrote "velac-core"; the
  crate in this repository is `vela-core`, and that is the target). Distribution
  reuses the binding routes feature 001 established — uniffi 0.32 for Kotlin/Swift,
  wasm-bindgen for web — rather than adding new ones.
- **`i18next@26.3.1` is the behavioural source of truth**, exactly as resolved in
  `package-lock.json` today, running with complete `Intl.PluralRules` (**MODE A**).
  This is a requester decision recorded during specification: MODE A is CLDR-correct,
  it is what the web build and the entire Jest suite already produce, and targeting
  the degraded native behaviour would bake a confirmed defect into the shared core.
  The consequence — Russian plurals visibly change on native — is intended.
- **Corpus distribution is per-locale, crate-sourced, and loaded on demand**
  (requester decisions): the JSON moves into `vela-core`, a generator emits every
  downstream artefact, compiled-in catalogs are selected per locale, and no platform
  loads more than the language in use plus the `en` fallback. Embedding all 15 locales
  into the web artefact was considered and rejected: it exceeds the CI gate by 52%,
  and base64-wrapping the corpus into wasm makes the *wire* size 45% worse than
  shipping it as JSON.
- **The React Native app is not migrated in this feature** (requester decision),
  mirroring how feature 003 landed. The engine and its corpus are proven first; the
  runtime swap is tracked separately. This keeps `react-i18next`'s subscription model,
  the `i18next.d.ts` key typing and the 92 `useTranslation()` hooks untouched, and
  avoids making the `_layout.tsx` full-tree remount load-bearing. The lazy-import
  switch on the RN side is enabled by FR-014's per-locale artefacts but is not
  performed here.
- **API parity is by shape, not by signature.** Rust returns `Result` where JS throws,
  uses `&'static str` where JS returns interned strings, and omits i18next's
  plugin/backend/detector architecture, its async initialisation, its logging and
  event surface, its `Trans` component support and its formatting plugin — none of
  which the app uses, and all of which belong to a framework layer rather than to a
  shared core. This is the "variations and omissions allowed" latitude the requester
  granted.
- **Scope of "i18next functionality" is what the app can reach.** The 42 option names
  the app passes are the mandatory surface. Behaviours the app does not use but that
  are cheap and observable (context, ordinals, array keys, namespaces) are included
  because the conformance corpus can prove them; behaviours requiring I/O or a plugin
  host are not.
- **The L10n scope is the full four-part set** the requester selected: reproduce the
  `locale-format.ts` contract, fix currency formatting, move relative-time / date
  headers / weekday names into the core, and add bidirectional isolation. Bidi
  isolation defaults off so it cannot break FR-005 parity; no RTL language is shipped
  today, so it is forward investment rather than an active requirement.
- **Corpus content defects are not this feature's to fix.** They are enumerated,
  reproduced exactly, linted, and filed separately. Silently "improving" a translated
  string during an engine port would make any parity claim unverifiable.
- **Rendering, layout mirroring and language detection stay outside the core.** The
  core reports direction and resolves strings; turning that into a mirrored layout or
  reading the device locale remains each platform's job.
