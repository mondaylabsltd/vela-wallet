# Feature Specification: getvela.app/notes — why Vela is the way it is, one page at a time

**Feature Branch**: `101-site-notes`

**Created**: 2026-10-09

**Status**: Draft

**Input**: The founder, 2026-10-09 (translated from Chinese):

> I want to copy bun.sh's guides and build the same thing for getvela.app. The difference:
> we're not teaching people to do an operation. We're telling them our design decisions,
> our principles, and what all the small things in the UI are for.
>
> 1. It should give us a large number of indexed pages.
> 2. It should be fragments, small stories, for understanding the design behind Vela Wallet.
> 3. It has to be organised by module, in order.
>
> Eventually fill it to 200-plus stories. No AI flavour: sincere, concise, easy and
> comfortable to read, true.

And the same day, after the plan: **"基本可以的，按你说的做"** (that's basically right, do it
the way you said), plus one part the plan missed:

> Vela Wallet actually has a lot of settings and ways to use it well: saving money, getting
> on chain faster, and so on, and layered security. I want those made too.

Then, while the pilot was being written: mixing Chinese and English on one page reads badly
("这种中英混杂阅读体验很差的"), and notes are English only, for native English readers.

## Why this exists

The site explains *what* Vela is (docs) and *what we think* (blog). It does not explain *why
each piece is the way it is*, and that is the part a careful reader checks before trusting a
wallet. The reasons already exist, scattered through 100 specs, rulings, issues and code
comments ("A badge is not a gate", "a defaulted 1 under a ¥ is a lie"). Nobody outside the
repository will ever read them there.

A harvest on 2026-10-09 (`docs/notes/INVENTORY.md`, at `8e70878dd`) found ~320 candidate
stories with a source behind each, so 200+ pages is a matter of writing, not of finding
material. The risk is the opposite one: the material is a mix of current and superseded
facts (WalletPair, the gas account, "one passkey per wallet", "12 networks" all still appear in
`docs/CONTENT-SOURCE-100-CLUES.md` and `docs/requirements/`). A page built from a stale lead
publishes a false sentence under the founder's name.

## Decisions (2026-10-09)

| # | Decision | Why |
|---|---|---|
| D1 | The section is **Notes**, at `/notes`, `/notes/<module>`, `/notes/<module>/<slug>` | Short, neutral, and leaves room for both kinds of page (why, and how to use it well) |
| D2 | Notes are written in **plain, neutral technical prose, after the bun.sh guides**: Vela and the reader are the subjects, no first person, no quotable lines (founder, 2026-10-09: "文风读起来有一种AI味……太完美，金句太多……能像 bun 学习吗"). Replaces the first draft's "I" | The first pilot, in the first person with rulings quoted, read as machine-written. bun's guides read as written by an engineer who knows the thing |
| D3 | **English only**, outside the locale subtree (like `/blog`), written for native English readers. No translations (founder, 2026-10-09: "notes 仅仅支持英文就行了，并且是给英文母语者阅读的") | The readers who self-host and compile read English. One language means one voice to get right, and no page that reads as translated |
| D4 | Incidents are published once fixed; internal business plans are not | A fixed incident, told plainly, is the most credible page a wallet can have. Pricing plans, the 90-day gates and marketing policy are the founder's to publish, not ours |
| D5 | The claim ledger (`specs/080-…/claim-ledger.md`) outranks every note | One fact, one value, every surface (080) |
| D6 | Fee pages that state the formula or the default speed wait for `fix/eth-mainnet-fee` | That branch changes both; ledger C-fee-1 and C-fee-3 change first |
| D7 | **Find-in-page, not a search box.** The index lists every note title as plain text, the sidebar lists every note, nothing is collapsed or loaded late (founder: "我说的搜索是网页内搜索，不是我们提供搜索框") | The browser's own ⌘F is the search people already use; it only works if the words are on the page |
| D8 | **Understood in seconds.** The answer under the title, numbers in an optional facts strip, short paragraphs, plain section names where they help (founder: "秒懂百科，一个东西简单的讲清楚"). A habit, not a template: "笔记四段式不用这么强迫症，我觉得只要阅读体验好就行" | The first pilot read as stacked blocks of text; reading ease is the measure, not conformance to a shape |
| D9 | **bun.sh/guides typography on every reading page**: docs, blog and notes. System face for text at 88% ink, condensed Archivo 800 for headings, Martian Mono for code; the blog's serif body is retired. CJK pages load no web font (founder, 2026-09-22) | "字体选择不如 bun … 这种视觉阅读体验应该延续到文档 blog 等" |
| D10 | **Nothing on a reading page competes with the text**: no accent colour (where you are is ink on a light tint), no capitals in labels, quotes on a neutral rule, links in ink with a quiet underline, facts on a tint without borders | "颜色上感觉对比太强烈，到处在抢眼球 … 想要顺利读下去有点分裂"; "视觉也要更加耐看，不要让人容易疲劳" |

## User Scenarios & Testing

### User Story 1 — A reader arrives from a search with one question (Priority: P1)

Someone searches "why does my smart wallet's first transaction cost more" or "passkey wallet
name too long". They land on one note, get the answer in the first sentence, can check it
against the code the note links to, and can follow two or three related notes.

**Independent Test**: open any published note with JavaScript disabled. The title, the answer,
the source link at a fixed commit and the related notes are all in the prerendered HTML.

**Acceptance Scenarios**:

1. **Given** a published note, **When** it is fetched, **Then** the HTML has one `<h1>` (the
   note's title), a meta description equal to the note's own one-sentence answer, a canonical
   URL, `TechArticle` and `BreadcrumbList` JSON-LD, and at least one link to source code
   pinned to a commit.
2. **Given** a note lists related notes, **When** it renders, **Then** each one appears as a
   link with its real title, and a missing one fails the build rather than rendering empty.

### User Story 2 — A reader browses the whole thing (Priority: P1)

Like bun.sh/guides: one index page that lists every module and every note title, so the whole
design can be skimmed in one scroll, and one page per module.

**Acceptance Scenarios**:

1. **Given** `/notes`, **Then** every published note is listed under its module, modules under
   their part, in the order of the module registry.
2. **Given** a module page, **Then** it has the module's one-line introduction and its notes in
   order, and each note page links back to it.

### User Story 3 — A reader wants to use Vela well (Priority: P1)

The second kind of note: what each setting does and when to change it, how to pay less, how to
get on chain faster, and how to add security layer by layer, each layer saying what it protects
against and what it doesn't.

**Acceptance Scenarios**:

1. **Given** the "Using Vela well" part, **Then** its pages follow the same rules as every
   other note (one point, a source, the cost stated).
2. **Given** a tip about money, **Then** it is true for the current code and carries the number
   it rests on, or it is not published.

### User Story 4 — The founder or an agent adds a note (Priority: P2)

Add one Markdown file in the right module folder; the index, the module page, the sitemap and
the related-note links pick it up. The tests say what is wrong before review does: a source path
that doesn't exist, a related note that doesn't exist, a description that's too long, words from
the banned list.

### User Story 5 — The code changes under a note (Priority: P2)

A source file is moved or deleted. The build fails on the note that cites it, instead of the
page quietly pointing at nothing (the fate of ~135 anchors in `docs/requirements/`).

## Requirements

### Functional

- **FR-001** Notes are Markdown files at `app-web/getvela.app/src/content/notes/<module>/<slug>.md`.
  The module is the folder; there is no second place that says which module a note is in.
- **FR-002** A single module registry (`src/lib/content/notes-modules.ts`) orders the parts and
  modules and gives each module a title and a one-line introduction. A note folder that is not
  in the registry fails the tests.
- **FR-003** Frontmatter: `title`, `description` (the one-sentence answer, ≤ 160 characters),
  `checked` (ISO date the facts were last checked against the code), `commit` (the commit they
  were checked at), `sources` (one or more repo-relative paths, optionally `#Lx-Ly`, or full
  URLs for other repositories), optional `related` (`<module>/<slug>`), optional `docs` (a docs
  slug), optional `order`, optional `draft`.
- **FR-004** Routes `/notes`, `/notes/<module>`, `/notes/<module>/<slug>` are prerendered
  (`'auto'` on the two dynamic routes, so a build with no published notes still builds). A
  `draft: true` note renders in `vite dev` only and is absent from the production build, the
  index, the module page and the sitemap.
- **FR-005** The note page renders the title from frontmatter (the body has no `# h1`), a
  breadcrumb (Notes › Module), the body, then a footer: source links pinned to `commit` on
  GitHub, "Checked against the code on <date>", related notes, the related doc, and the
  previous/next note in the module.
- **FR-006** Each published note, module page and the index are in the sitemap as English-only
  URLs; notes carry `lastmod` = `checked`.
- **FR-007** The site header links to `/notes` (English-only, with the existing EN badge in other
  locales). `chrome.nav.notes` is translated in all fifteen locales, as chrome must be.
- **FR-008** Tests fail when: a source path doesn't exist in the repository; a related note or
  doc doesn't exist; a required field is missing or a description is over 160 characters; a note
  is under 60 or over 500 words, or has a paragraph over 80 words; a fact isn't "Label | value"
  or there are more than four; a sidebar label is over 40
  characters; the prose contains CJK text; a note uses a phrase from the banned list
  (`docs/notes/STYLE.md`); a note has more than two em dashes.
- **FR-009** `docs/notes/STYLE.md` is the writing contract: voice, structure, the banned list
  and the fact rules. The tests enforce the mechanical part of it.
- **FR-010** The live-site errors found by the harvest are fixed in this branch, because notes
  link into those pages and must not contradict them (INVENTORY.md, "Found while harvesting").
- **FR-011** What the lists, sidebar and sitemap know about a note comes from
  `virtual:notes-index` (frontmatter only, built by the plugin in `vite.config.ts`); a note's
  body is imported only by its own page, so no page carries every note's text.
- **FR-012** Frontmatter also takes `nav` (sidebar label, required when the title is over 40
  characters), `facts` (two to four `"Label | value"`) and `featured` (a "Start here" card).
- **FR-013** Reading pages (docs, blog, notes) share one typography and colour treatment (D9,
  D10) through `Prose`, the `.reading` scope in `tokens.css` and `ReadingFonts`.

### Content

- **CR-001** Every note's facts are re-read from the cited source when it is written. The
  inventory is a list of leads, not a source.
- **CR-002** A note never contradicts the claim ledger, and uses the ledger's wording where it
  states a ledger fact.
- **CR-003** A note never describes as shipped something that hasn't shipped (081 FR-020).
- **CR-004** The first batch is ten pilot notes across modules, written as drafts for the
  founder to edit. STYLE.md is revised from those edits before the next batch.

## Success Criteria

- **SC-001** The section builds, prerenders and passes `bun run check` and the unit tests with
  the pilot notes in it.
- **SC-002** Every published note has a working source link at a fixed commit, and the test that
  checks source paths runs in CI.
- **SC-003** 200+ published notes, in batches the founder has reviewed.
- **SC-004** No published note contradicts the claim ledger (checked per batch).

## Out of scope (for now)

- Any translation. Notes are English only (D3); the header label is translated because it is
  chrome, and links to /notes with the existing EN badge.
- An RSS feed or search for notes.
- Notes the founder marked 🗣 in the inventory, until each has the founder's answer.
