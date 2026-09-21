# Feature Specification: getvela.app — every sentence true, findable, and ready for an expert reader

**Feature Branch**: `080-site-content-accuracy`

**Created**: 2026-09-22

**Status**: Draft

**Input**: The founder, 2026-09-22 (translated from Chinese):

> getvela.app has a lot of copy and documentation — roadmap, whitepaper, blog, terms,
> privacy — plus the repository's root README.md.
>
> First, make sure the content is accurate: verify it against the source code and
> against decisions that have been made. Second, make sure it reads clearly and is
> indexed clearly, so that a user with a problem finds the solution easily; the
> organisation and navigation have to be good.
>
> Three places in particular:
> 1. The landing page — does it attract our potential customers, is it full of
>    marketing vocabulary and AI flavour, is it accurate, does it win potential users
>    over so they try it?
> 2. Our most important selling point is "an Ethereum wallet you actually own". Do we
>    deliver it — can a user easily set up and run everything Vela Wallet needs? Are
>    the docs clear and easy to follow step by step, and do they reach that goal?
> 3. If we were recommending Vela Wallet to Ethereum's founder tomorrow, which copy
>    and which documents have fatal problems, why, how to fix them — and fix them.
>
> This is a long task; no human intervention — do it yourself. Last: translation
> accuracy, following https://shelchin.com/prompts/single-string-i18n-locale-review/.
> Use the GitHub speckit workflow.

Two follow-ups the same day: **"spec 号从 080 开始"** (number this spec 080) and
**"source of truth 应该是英语和中文结合起来看"** (the source of truth is English and
Chinese read together).

## Why this exists

The site was rebuilt in spec 059 around one promise — *An Ethereum wallet you
actually own* — and its landing page has been edited line by line since. The
documentation underneath it has not kept pace with the product. A first read on
2026-09-22 (before any source verification) found the same fact stated
differently on different pages, and several claims that a careful reader would
catch on sight:

| Where | What it says | Why it is a problem |
|---|---|---|
| Landing, docs, whitepaper vs README | "12 networks" | README says 24; at most one of them is true |
| Create wallet, recovery, whitepaper, passkeys, privacy, terms | One passkey, synced by iCloud or Google, unlocked by face or fingerprint | The wallet creates up to seven keys at once, including security keys that sync nowhere; the docs that explain the account never say so |
| Audits page | The passkey signer contract holds "exactly one passkey per Safe" | Irreconcilable with "up to seven keys" unless other contracts are involved — which the contract list does not name |
| Account contract | "Any Safe-compatible interface can drive it" if Vela is gone | A passkey bound to `getvela.app` cannot sign in a Safe interface served from another domain |
| Whitepaper, threat model | "Compromised Vela server — yields no signing ability" | For a web wallet, whoever controls the served code controls what you are asked to sign; that is the Bybit lesson the site itself teaches |
| Whitepaper vs landing FAQ | Domain loss has "no consumer-grade path" vs "sign from the extension; your keys work there" | Opposite answers to the most important ownership question |
| Install, introduction, create wallet | "Nothing to download, no app store; mobile coming soon" | Desktop apps and an extension ship from the get-started page |
| Roadmap | Address book "next"; desktop dApp connect "without your phone" next; an audit on the list | Contacts shipped; that connection method was dropped; no audit is scheduled |
| Passkeys page, blog | "The same technology that protects Apple Pay" | Not the same technology |
| Several | "The single biggest cause of lost crypto" / "the single most common way" | Unsourced superlatives |
| Landing trade-offs vs fees doc | Trade-offs list three costs; the fees doc describes a non-refundable per-chain gas-account deposit | If the deposit is real, it is a cost the landing page omits |

The ownership promise is the biggest gap. The landing page's fourth claim —
*access doesn't depend on Vela staying online; self-host the app, the relay and
every backend service* — links to a README table of four repositories and a list
of health-check rules. There is no page that tells a capable reader, in order,
what to run, what each piece needs, how to point the wallet at it, how to check
it worked, and which parts cannot be replaced (the passkey's binding to
`getvela.app`) together with the escape hatches that exist for them.

## The rulings this cut carries

Decisions made for this spec. The founder asked for no interruptions, so they are
recorded here to be overruled, not asked about.

| # | Ruling | Source / reason |
|---|---|---|
| R1 | **Code is the authority.** Where a page and the code disagree, the page changes. Where two shells disagree, the page states the difference or the narrower truth | Founder: "根据源码…去判断验证" |
| R2 | **English and Chinese together are the reference.** They are finalized first and must say the same thing; the other 13 locales are re-aligned to their shared meaning, not to either one word for word | Founder, 2026-09-22; the single-string review method |
| R3 | Spec 059's rulings stand: the headline and its Chinese, the 15-locale set, English at the root, the two-link nav, the first-screen layout, blog/privacy/terms English-only, site copy never in the vela-core corpus | Spec 059 |
| R4 | Standing positions stand: no third-party audit and none scheduled (never "planned"); no token; web free, store builds paid one-time, building from source free; no scary beta banners; honesty over disclaimers | Recorded founder decisions |
| R5 | **Blog posts are dated records.** They are not rewritten into the present. A post whose claim is now false, or was never true, gets a dated correction note; an unsourced superlative is corrected in place with a note | Rewriting history silently is its own honesty problem |
| R6 | **A new self-hosting guide is part of the docs,** in the docs' own navigation, translated like every other doc. The README keeps a short pointer to it and the developer-facing build details | The landing page's fourth claim needs a page that proves it |
| R7 | The docs navigation may gain pages and regroup, but **no existing URL breaks** — a moved or merged page keeps its URL or redirects | Links from the landing page, blog, README and external sites |
| R8 | Where an honest statement costs us something (a fee, a limit, a missing piece), **say it plainly and link to the evidence** rather than soften it | The site's own stated posture |

## User Scenarios & Testing *(mandatory)*

### User Story 1 — An expert reads the site and finds nothing false (Priority: P1)

A reader who knows Ethereum, account abstraction and WebAuthn deeply — the
standard is "Ethereum's founder, tomorrow" — follows the landing page into the
whitepaper, the account-contract page, the audits page and the self-hosting
guide, checking each claim against what they know and against the linked code.

**Why this priority**: One false or overstated sentence found by this reader
costs more than every other improvement in this spec earns. The product's whole
pitch is "check it yourself"; the pages must survive being checked.

**Independent Test**: Take the claim ledger (every factual sentence on the
landing page, whitepaper, account contract, audits, passkeys, signers, recovery,
fees and FAQ), and for each claim confirm a code citation, a cited external
source, or a recorded founder decision. Separately, have a reviewer who did not
write the copy read those pages adversarially and list anything false,
overstated or evasive.

**Acceptance Scenarios**:

1. **Given** the whitepaper's threat model, **When** the reader asks "what can a
   compromised Vela make me sign?", **Then** the page answers it truthfully —
   separating backend services (no signing power) from the delivery of the app
   itself (can present a malicious transaction) — and names the mitigations that
   actually exist.
2. **Given** any page that says who can sign for a wallet if Vela disappears,
   **When** the reader compares it with every other page that says the same,
   **Then** all of them give the same answer, and that answer matches what the
   code and the extension can do.
3. **Given** the contract list on the audits and account-contract pages, **When**
   the reader checks it against a real multi-key wallet on a block explorer,
   **Then** every contract that wallet actually uses is listed, with the audit
   status that applies to it.
4. **Given** any number stated anywhere (networks, keys, fee multiple, minimums),
   **When** compared across pages and with the code, **Then** it is the same
   everywhere and matches the code.

---

### User Story 2 — A capable user runs Vela without Vela (Priority: P1)

A technically capable user who took the headline literally wants to confirm that
their wallet does not depend on Vela's company or servers. They open the
self-hosting guide and follow it step by step.

**Why this priority**: "An Ethereum wallet you actually own" is the headline.
If the steps that prove it do not exist, the headline is a slogan.

**Independent Test**: Follow the guide from top to bottom on a clean machine
(where a step needs paid infrastructure or a funded account, verify the step
against the service's code and README and state exactly what is needed). Each
step names what to run, what it requires, how to point the wallet at it, and how
to confirm it worked.

**Acceptance Scenarios**:

1. **Given** the guide, **When** the user reads its first screen, **Then** they
   learn which pieces Vela runs by default, which of them they can replace, and
   which they cannot — with the passkey's binding to `getvela.app` explained and
   the ways around it listed.
2. **Given** each replaceable service (relay, public-key index, chain data index,
   exchange rates, the web app, the signing page), **When** the user follows its
   section, **Then** they reach a running instance and a wallet pointed at it, or
   the section states exactly which step they cannot complete without an account,
   funds or a domain, and why.
3. **Given** the scenario "getvela.app is gone tomorrow", **When** the user looks
   for what to do, **Then** one section gives the complete answer — which apps and
   extensions can still sign for an existing wallet, with which kinds of keys —
   and every other page that touches the question links to it.

---

### User Story 3 — A prospective user decides to try Vela (Priority: P2)

Someone who holds or is about to hold ETH and stablecoins lands on getvela.app
from a search, a post or a link. In one scroll they should understand what Vela
is, who it is for, what it costs them, what it cannot do, and how to start.

**Why this priority**: The landing page is the entry point for every visitor;
it decides whether anyone reads the rest.

**Independent Test**: Read the landing page top to bottom and check every
sentence against three questions: is it true, is it specific, would a person say
it this way? Count marketing and AI-pattern phrases; list every claim and its
proof link.

**Acceptance Scenarios**:

1. **Given** the landing page, **When** a visitor reads it, **Then** every claim
   is specific and checkable, and every cost to the user that the product imposes
   today is either on the page or one click from it.
2. **Given** the landing page's language, **When** reviewed for filler,
   superlatives, empty contrast ("not X — it's Y") and slogan cadence, **Then**
   none remain that a plain statement could replace.
3. **Given** the call to action, **When** the visitor clicks it, **Then** they
   land on a page that tells them which platform to pick and what it costs,
   accurately.

---

### User Story 4 — A user with a problem finds the answer (Priority: P2)

A user arrives with a concrete question: "I lost my phone", "why is the fee this
high", "can I add a key", "is my chain supported", "what if getvela.app is
down", "how do I connect to a dApp", "which platforms exist and what do they
cost", "is it audited".

**Why this priority**: Findability decides whether accurate documentation helps
anyone.

**Independent Test**: For a fixed list of at least 12 common questions, start at
the docs home and count clicks to the page and section that answers it.

**Acceptance Scenarios**:

1. **Given** the docs home, **When** a user scans it, **Then** they see the
   common tasks and problems by name, each linking straight to its answer.
2. **Given** any docs page, **When** it mentions a concept explained elsewhere,
   **Then** it links there, and it ends with a sensible next step rather than a
   dead end.
3. **Given** the sidebar, **When** a user reads its groups, **Then** the grouping
   follows what a user is trying to do, and the self-hosting guide is in it.

---

### User Story 5 — A reader in another language gets the same truth, natively (Priority: P3)

A reader in any of the 13 non-English, non-Simplified-Chinese locales opens the
landing page or a doc in their language.

**Why this priority**: Wrong or unnatural copy in a locale undoes the accuracy
work for that reader; it follows the English and Chinese work, which it depends
on.

**Independent Test**: For each locale, check that every changed page says what
the finalized English and Chinese say, reads as native copy, keeps every link,
tag and placeholder intact, and uses the locale's established terms.

**Acceptance Scenarios**:

1. **Given** a finalized English/Chinese page, **When** the locale's version is
   compared, **Then** no claim is missing, added, or inverted.
2. **Given** each locale's review, **When** findings are graded, **Then** every
   High and Medium finding is fixed and recorded.
3. **Given** a page not yet translated in a locale, **When** a reader opens it,
   **Then** they see the English body under the notice in their own language, as
   spec 059 designed — never a half-translated page.

---

### User Story 6 — A developer reads the README (Priority: P3)

A developer lands on the GitHub repository and reads the README to understand
what Vela is, what state each platform is in, and how to build and self-host.

**Why this priority**: The README is the second front door and is what an expert
reads first; it currently disagrees with the site.

**Independent Test**: Check every factual statement in the README against the
code and the site; confirm it links to the site's self-hosting guide.

**Acceptance Scenarios**:

1. **Given** the README's platform status, network count and fee model, **When**
   compared with the code and the site, **Then** they agree.

### Edge Cases

- **A claim cannot be verified from code** (e.g. an external fact such as an
  audit report or incident detail): it keeps its cited source, or it goes.
- **Two shells behave differently** (e.g. one supports a setting another lacks):
  the page states the difference rather than the most flattering case.
- **The code is in flux** (a fix merged the same week): the page describes what
  `main` does at the time of writing and dates the page.
- **A self-hosting step needs things we cannot provide** (a domain, a funded key,
  a cloud account): the guide says so and says what exactly is needed.
- **A translation's natural phrasing drops a qualifier** ("up to", "by default",
  "not necessarily"): the qualifier is a fact, not style, and must survive.
- **A new doc exists in English and Chinese but not yet in a locale**: that locale
  shows the English body under the fallback notice; it is not advertised as a
  translation.
- **A blog post's historical claim is now false**: a dated correction note is
  added; the post is not silently rewritten.

## Requirements *(mandatory)*

### Functional Requirements

**Accuracy**

- **FR-001**: Every factual claim on the landing page, get-started, about,
  roadmap, chain-setup copy, all docs, privacy policy, terms and README MUST be
  traceable to code, a cited external source, or a recorded decision; the
  traceability is recorded in a claim ledger in this spec's folder.
- **FR-002**: Every number and named fact that appears on more than one page
  (network count and list, maximum keys, key rules, fee formula and minimums,
  contract set, platforms and prices, service list) MUST be identical everywhere
  it appears.
- **FR-003**: No page may describe a single-passkey, iCloud/Google-only model as
  the whole story; key types, the maximum at creation, the 1-of-n rule, and the
  fixed-at-creation rule MUST be stated consistently wherever keys are explained.
- **FR-004**: The contract list MUST include every contract a wallet with more
  than one key actually depends on, with the audit status that applies to each
  version deployed.
- **FR-005**: The threat model MUST distinguish backend services from the delivery
  of the app's code, and MUST NOT claim that a compromise of Vela's
  infrastructure cannot lead to a malicious signing request.
- **FR-006**: Every statement about who can sign for a wallet without Vela
  (Safe interfaces, extensions, self-built apps, key types) MUST be consistent
  across pages and match what the code does.
- **FR-007**: Pages describing install and wallet creation MUST describe every
  platform that ships today and the creation flow as it is today.
- **FR-008**: The roadmap MUST NOT list shipped work as upcoming, dropped work as
  planned, or an audit as scheduled; it SHOULD record the major shipped
  milestones.
- **FR-009**: Every cost the product imposes on a user today (relay fee and how
  it is computed, any per-network deposit, store prices) MUST be stated on the
  fees page and be reachable in one click from the landing page's trade-offs.
- **FR-010**: Unsourced superlatives and comparisons that are not true
  ("the single biggest cause", "the same technology as Apple Pay") MUST be
  removed or sourced, including in blog posts (with a dated note).
- **FR-011**: The privacy policy MUST disclose every data flow the software and
  site actually perform that involves user data, and MUST describe keys and
  storage accurately; its "last updated" date MUST change when it changes.
- **FR-012**: The terms MUST NOT contradict the product (fees, paid store builds,
  key types, services) and MUST change their date when they change.

**The ownership promise**

- **FR-013**: The docs MUST include a self-hosting guide that lists every piece of
  infrastructure the wallet uses by default, says for each whether and how it can
  be replaced, and gives ordered steps with requirements and a way to confirm each
  step worked.
- **FR-014**: The guide MUST state what cannot be self-hosted — at minimum the
  passkey's binding to `getvela.app` and its consequences for a self-hosted web
  app — and MUST list every existing way to keep signing for an existing wallet
  if `getvela.app` is gone, by key type.
- **FR-015**: The landing page's ownership claim, the whitepaper's "if Vela
  disappears" section, the FAQ answers on shutdown, the terms' service
  availability section and the README MUST link to that guide rather than to a
  README table.

**Landing page**

- **FR-016**: The landing page MUST keep spec 059's headline, layout rulings and
  nav, and MUST have every sentence reviewed for accuracy, specificity and
  natural voice; filler, unsupported superlatives and slogan patterns are
  removed.
- **FR-017**: Every claim on the landing page MUST link to, or sit one click
  from, the page that proves it.

**Findability**

- **FR-018**: The docs home MUST offer task- and problem-oriented entry points
  covering at least the common questions listed in SC-004.
- **FR-019**: Docs pages MUST cross-link concepts explained elsewhere and end with
  a next step; no internal link may be broken.
- **FR-020**: The sidebar grouping MUST follow user intent and include every docs
  page, including new ones; no existing docs URL may break.

**Translation**

- **FR-021**: English and Simplified Chinese MUST be finalized together and agree
  in meaning on every changed page and string.
- **FR-022**: Every changed page and string MUST be re-aligned in the other 13
  locales to the finalized English/Chinese meaning and reviewed on accuracy,
  naturalness, cultural risk, terminology consistency and technical integrity;
  High and Medium findings are fixed; findings are recorded per locale.
- **FR-023**: Links, inline tags and placeholders in translations MUST match the
  English exactly; the site's existing checks MUST pass.
- **FR-024**: Blog, privacy and terms stay English-only; new docs are translated
  or fall back per spec 059.

**README**

- **FR-025**: The root README MUST agree with the code and the site on every
  shared fact, describe each platform's current state, and point to the
  self-hosting guide.

### Key Entities

- **Claim**: one factual sentence on a public page — page, locale, text, the
  evidence that supports it (code path, external source, or decision), and a
  verdict (true / corrected / removed).
- **Finding**: a defect found by the audit — page, category (false, stale,
  overstated, contradicts page X, unclear, unfindable, AI-voice, translation),
  severity (Fatal / High / Medium / Low), explanation, fix.
- **Service**: a piece of infrastructure the wallet uses — default instance, what
  depends on it, whether it is replaceable, where its code lives, what running it
  requires.
- **Locale review**: per locale — pages checked, findings with severity, fixes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero claims in the claim ledger are unverified or false at the end;
  every Fatal and High finding in the audit report is fixed.
- **SC-002**: Every shared fact (FR-002) has exactly one value across all pages,
  all 15 locales and the README — checked by search, with zero mismatches.
- **SC-003**: An adversarial review of the landing page, whitepaper, account
  contract, audits, passkeys, signers, recovery, fees, FAQ and self-hosting
  guide, done after the fixes, produces no finding of a false or overstated
  claim.
- **SC-004**: Each of these questions is answered within two clicks of the docs
  home: lost phone; lost or deleted passkey; add or change keys; why this fee;
  per-network deposit; is my chain supported / add a chain; getvela.app offline;
  run my own services; connect to a dApp; which platform to install and what it
  costs; is it audited; what data is public.
- **SC-005**: The self-hosting guide covers 100% of the default infrastructure
  found in code; every step has requirements and a confirmation check; every step
  that could be run locally was run.
- **SC-006**: The landing page contains no unsupported superlative and no claim
  without a proof link or proof section.
- **SC-007**: All 13 re-aligned locales have a recorded review for every changed
  page with zero open High or Medium findings; the site's type check, unit tests
  and build pass; zero broken internal links.

## Assumptions

- The audience of the landing page is the one spec 059 and the marketing notes
  name: people who hold ETH and stablecoins, are wary of seed phrases, and are
  able to check a claim — not first-time crypto users.
- "Run everything Vela needs" is read as: the services the wallet contacts by
  default, the app itself (web deployment or a self-built native app), and the
  independent signing page. Blockchain RPC nodes are the user's choice already
  and are covered by pointing at existing documentation rather than by a
  node-running guide.
- Steps that need paid infrastructure, a funded key or a registered domain are
  verified against the code and the service's own README and marked as such; they
  are not executed against production networks.
- Native store listing status and prices are taken from the repository's recorded
  decisions and the site's own download logic at the time of writing.
- The blog, privacy policy and terms remain English-only (spec 059 R5).
- Visual layout is unchanged except where copy changes need it; spec 059's layout
  rulings apply.
