# Tasks: getvela.app — every sentence true, findable, and ready for an expert reader

**Input**: Design documents from `specs/080-site-content-accuracy/`
**Prerequisites**: plan.md, spec.md, research.md, claim-ledger.md, data-model.md, contracts/, quickstart.md

**Tests**: The spec asks for existing gates to pass and adds two checks (doc
fingerprints, stable anchors); no other test tasks.

**Paths**: `G/` = `app-web/getvela.app/`. English docs `G/src/content/docs/<slug>.md`;
Chinese `G/src/content/docs/zh/<slug>.md`; catalogs `G/src/lib/i18n/messages/`.
Every English change is made together with its Chinese counterpart (R2) and
checked against [claim-ledger.md](claim-ledger.md).

## Phase 1: Setup

- [ ] T001 Record the baseline: run `bun run check`, `bun run test:unit -- --run`, `bun run build` in `G/` and note pre-existing failures in `specs/080-site-content-accuracy/results.md`; keep the "before" screenshots from the scratchpad
- [x] T002 Fix the `/api/transactions` API-key leak (allowlist the network slug) in `G/src/routes/api/transactions/+server.ts` (research D8)

## Phase 2: Foundational (blocks all stories)

- [ ] T003 Write `specs/080-site-content-accuracy/audit-report.md`: every finding with category, severity (Fatal/High/Medium/Low), why, fix — from research.md and the five reports, including the external-citation report
- [ ] T004 [P] Add doc fingerprints: `source:` front-matter stamp for translated docs in `G/scripts/i18n-stamp.ts`, STALE docs reported and gated in `G/scripts/i18n-status.ts`, page count includes the new doc (data-model §5)
- [ ] T005 [P] Add a stable-anchor test in `G/src/lib/content/docs.test.ts`: each anchor listed in contracts/docs-information-architecture.md exists as `id="…"` in every locale's copy of its page
- [ ] T006 Restructure the sidebar in `G/src/lib/content/sidebar.ts` (groups `keys`, `selfHost`; slug `self-hosting`; order per the IA contract) and add `chrome.docs.groups.keys`, `chrome.docs.groups.selfHost`, `chrome.docs.titles['self-hosting']` to `G/src/lib/i18n/messages/en.ts` and all 14 `<tag>.json` (chrome must be complete in every locale)

**Checkpoint**: findings list exists; tooling ready; sidebar has the new slots.

## Phase 3: User Story 1 — An expert reads the site and finds nothing false (P1) 🎯 MVP

**Goal**: No false, overstated or evasive claim on the pages an expert reads; shared facts identical everywhere.
**Independent test**: claim-ledger checks return only ledger values; adversarial review (T093) finds nothing open.

- [ ] T010 [US1] Rewrite `G/src/content/docs/whitepaper.md` + `zh/whitepaper.md`: architecture (Rust core + native apps + extension), keys (1–7, kinds, 1-of-n, fixed), address from all keys, contracts (factory, fallback handler), fees (C-fee-1/2/3), threat model separating backend services from app delivery and naming the domain risk, recovery (any key; registry; single-key two-signature fallback), "If Vela disappears" → self-hosting guide, privacy (index, relay logs), licences (C-lic-1), gas wording in "No token"
- [ ] T011 [P] [US1] Rewrite `G/src/content/docs/account-contract.md` + zh: contract table incl. SafeWebAuthnSignerFactory/singleton and the 4337 module as fallback handler; replace "any Safe-compatible interface can drive it" with C-safeui-1; audit paragraph per C-audit-1
- [ ] T012 [P] [US1] Update `G/src/content/docs/security-audits.md` + zh: signer factory and singleton with their audit coverage; fallback handler; replace the legacy index with registry V13 `0x94fD…EA9` + domain registry `0x5266…edaf` under "not audited"; 24 networks; drop "we state it in the site header"; apply external-citation corrections; new "Last reviewed" date
- [ ] T013 [P] [US1] Rewrite `G/src/content/docs/passkeys.md` + zh: kinds of key (C-keys-2), user verification (C-auth-1), remove the Apple Pay comparison and the "single most common way" superlative, relay submits (not "Vela broadcasts"), what passkeys do not protect against (signing a bad transaction → clear signing)
- [ ] T014 [P] [US1] Update `G/src/content/docs/signers.md` + zh: why owners can't change later incl. the cross-chain divergence reason; NFC only through browser/OS sheets; native security-key limits
- [ ] T015 [P] [US1] Rewrite `G/src/content/docs/recovery.md` + zh: sign in with any key; registry (index first, then contract on Gnosis/Ethereum); single-key two-signature fallback and its limit; security keys; no account sync (C-sync-1)
- [ ] T016 [P] [US1] Rewrite `G/src/content/docs/networks-and-fees.md` + zh: 24-network table; fee formula (C-fee-1), speed (C-fee-3), no deposit + treasury bootstrap (C-fee-2), relay compatibility (C-relay-1), custom networks + `/chain-setup`, RPC pool
- [ ] T017 [P] [US1] Update `G/src/content/docs/clear-signing.md` + zh: descriptor sources and what "verified" means (C-clear-1); approval guard scope incl. permits (C-approve-1)
- [ ] T018 [P] [US1] Update `G/src/content/docs/bybit-attack.md` + zh: signing page status (C-signpage-1); a dApp request can target the account itself and is decoded but not blocked; user-verification wording; self-hosting link
- [ ] T019 [P] [US1] Update `G/src/content/docs/clear-signing-self-host.md` + zh: how to get the folder (clone), no Vela app sends requests to it yet, BLE untested, where it fits
- [ ] T020 [P] [US1] Rewrite `G/src/content/docs/faq.md` + zh: free vs paid, fees without the deposit, what Vela sees (index, relay), licences, 24 networks, keys
- [ ] T021 [P] [US1] Update `G/src/content/docs/why-vela.md` + zh: key kinds, callout link target, facts per ledger
- [ ] T022 [US1] Landing copy in `G/src/lib/i18n/messages/en.ts` + `zh.json` (`home.*`): trade-off 1 (fee is a multiple of the on-chain cost), trade-off 3 (add Vela's own code unaudited), compare rows (an extra check → in development; custom networks; losing a key), networks heading 24, FAQ answers (NFC, dApp platforms, getvela.app offline), meta descriptions
- [ ] T023 [US1] Landing component `G/src/routes/[[locale=locale]]/+page.svelte`: 24 networks with logos from `ethereum-data.getvela.app`; FACT_LINKS[3] → `/docs/self-hosting`; COMPARE_TONES for changed rows
- [ ] T024 [P] [US1] Privacy policy `G/src/routes/privacy/+page.svelte`: key kinds; index service; relay logging/retention; `X-Rpc-Url`; chain-data paths; selector/AAGUID/tunnel/Cloudflare/Google Fonts; on-chain record in full; sign-out vs erase; server retention; licences; controller scope + company no.; privacy contact email; date
- [ ] T025 [P] [US1] Terms `G/src/routes/terms/+page.svelte`: key kinds and 1-of-n risk; fees and non-refundable treasury top-ups; paid store builds; services list; self-hosting limits; unaudited integration; company details; statutory carve-out; governing law; contact; date
- [ ] T026 [P] [US1] Blog dated correction notes in `G/src/content/blog/why-no-seed-phrase.md`, `hello-world.md`, `vela-is-in-alpha.md` (superlative, Apple Pay, single-key model, "all backend MIT")

**Checkpoint**: English + Chinese of every expert-read page match the ledger.

## Phase 4: User Story 2 — A capable user runs Vela without Vela (P1)

**Goal**: A step-by-step guide that proves the ownership claim, honest about limits.
**Independent test**: quickstart §5 walk; SC-005.

- [ ] T030 [US2] Write `G/src/content/docs/self-hosting.md` per contracts/self-hosting-guide.md, commands copied from the service repos, stable anchors
- [ ] T031 [US2] Write `G/src/content/docs/zh/self-hosting.md` (same meaning, native Chinese)
- [ ] T032 [US2] Walk the guide locally where possible (signing page served locally; vela-currency and ethereum-data READMEs; web wallet build) and record run-vs-checked per step in results.md
- [ ] T033 [P] [US2] Point every ownership reference at the guide: footer Infrastructure column in `G/src/lib/components/SiteFooter.svelte`, terms §4, whitepaper, FAQ, README

## Phase 5: User Story 3 — A prospective user decides to try Vela (P2)

**Goal**: Landing, get-started, about and roadmap are accurate, specific and plain.
**Independent test**: voice review (no superlatives/slogan patterns) + ledger checks.

- [ ] T040 [US3] Voice and accuracy pass on `home.*` beyond T022 (hero facts, why, pricing cards incl. desktop free download) in `en.ts` + `zh.json`
- [ ] T041 [P] [US3] `getStarted.*` in `en.ts` + `zh.json`: "one codebase" → shared core; mobile status; extension browsers; web has no dApp connection; fix stale comment in `G/src/routes/[[locale=locale]]/get-started/+page.svelte`
- [ ] T042 [P] [US3] `about.*` in `en.ts` + `zh.json`: remove unsourced superlative and "face or fingerprint" passkey definition; plain voice
- [ ] T043 [US3] Roadmap: rewrite `roadmap.*` in `en.ts` + `zh.json` (shipped milestones through September; upcoming only what is true; no audit; no non-P-256 path) and the dates in `G/src/routes/[[locale=locale]]/roadmap/+page.svelte`

## Phase 6: User Story 4 — A user with a problem finds the answer (P2)

**Goal**: SC-004's twelve questions answered within two clicks of `/docs`.
**Independent test**: click-count table in results.md; link crawl.

- [ ] T050 [US4] Rewrite `G/src/content/docs/introduction.md` + zh: short what-it-is + "Find an answer" list (IA contract)
- [ ] T051 [P] [US4] Rewrite `G/src/content/docs/install.md` + zh: every platform, cost, status, device requirements, official URLs, `#dapps` anchor
- [ ] T052 [P] [US4] Rewrite `G/src/content/docs/create-wallet.md` + zh: the multi-key creation flow, `#what-is-public` anchor (C-reg-1), deployment on first send
- [ ] T053 [P] [US4] Update `G/src/content/docs/send-and-receive.md` + zh: split / sweep / batch, contacts, name resolution list, fee asset and speed, history sources, links
- [ ] T054 [US4] Cross-link and next-step pass over all 17 English docs and their Chinese; crawl internal links on the dev server (quickstart §3)

## Phase 7: User Story 6 — A developer reads the README (P3)

- [ ] T060 [US6] Root `README.md`: features (24 networks, keys, dApp surfaces), platform status, fee model (in-band, formula, no deposit), recipient resolution, rpId rule, self-hosting pointer to `/docs/self-hosting`, licences
- [ ] T061 [P] [US6] Authority docs: correction block in `docs/CONTENT-SOURCE-100-CLUES.md` guardrails; `docs/requirements/F01-supported-networks-registry.md`; factual lines in `docs/store-submission/store-listing-copy.md`; links in `docs/ROADMAP.md` and `docs/WHITEPAPER.md`

## Phase 8: User Story 5 — The same truth, natively, in 13 more locales (P3)

**Goal**: Every changed page re-aligned to the finalized en+zh and reviewed.
**Independent test**: per-locale review record with no open High/Medium; tests pass.

- [ ] T070 [P] [US5] zh-TW: catalog (`zh-TW.json`: home, getStarted, about, roadmap, chrome) + all 17 docs under `G/src/content/docs/zh-TW/`; review record `specs/080-site-content-accuracy/reviews/zh-TW.md`
- [ ] T071 [P] [US5] zh-HK: same, `zh-HK.json`, `docs/zh-HK/`, `reviews/zh-HK.md`
- [ ] T072 [P] [US5] ja: same, `ja.json`, `docs/ja/`, `reviews/ja.md`
- [ ] T073 [P] [US5] ko: same, `ko.json`, `docs/ko/`, `reviews/ko.md`
- [ ] T074 [P] [US5] vi: same, `vi.json`, `docs/vi/`, `reviews/vi.md`
- [ ] T075 [P] [US5] id: same, `id.json`, `docs/id/`, `reviews/id.md`
- [ ] T076 [P] [US5] tr: same, `tr.json`, `docs/tr/`, `reviews/tr.md`
- [ ] T077 [P] [US5] es-MX: same, `es-MX.json`, `docs/es-MX/`, `reviews/es-MX.md`
- [ ] T078 [P] [US5] pt-BR: same, `pt-BR.json`, `docs/pt-BR/`, `reviews/pt-BR.md`
- [ ] T079 [P] [US5] fr: same, `fr.json`, `docs/fr/`, `reviews/fr.md`
- [ ] T080 [P] [US5] de: same, `de.json`, `docs/de/`, `reviews/de.md`
- [ ] T081 [P] [US5] ru: same, `ru.json`, `docs/ru/`, `reviews/ru.md`
- [ ] T082 [P] [US5] it: same, `it.json`, `docs/it/`, `reviews/it.md`
- [ ] T083 [US5] Record zh as reviewed against the new en in `reviews/zh.md`; update `G/src/lib/i18n/review.json` for every locale whose record has no open High/Medium; run `bun run i18n:stamp`

## Phase 9: Polish & cross-cutting

- [ ] T090 Gates: `bun run check`, `bun run test:unit -- --run`, `bun run i18n:status --gate`, `bun run build` in `G/`
- [ ] T091 Link crawl of `/`, `/docs`, every doc in en/zh/ja, footer, get-started (quickstart §3)
- [ ] T092 Screenshots at 1440 and 390 px, en and zh: `/`, `/docs`, `/docs/self-hosting`, `/docs/install`, `/get-started`, `/roadmap`; read each
- [ ] T093 Adversarial expert review by an independent reviewer of the pages in SC-003; fix anything found
- [ ] T094 Write `specs/080-site-content-accuracy/results.md`: what changed, verification, the SC-004 click table, product gaps for follow-up (research D9), open items for the founder
- [ ] T095 Commit by area with explicit paths, push `080-site-content-accuracy`, open the PR

## Dependencies & execution order

- Phase 1 → Phase 2 → Phases 3–7 → Phase 8 → Phase 9.
- US1 (Phase 3) and US2 (Phase 4) are both P1; US2's guide (T030) is referenced by
  US1 pages, so T030 lands before T010/T020 are finished (they link to it).
- US3, US4, US6 are independent of each other once Phase 2 is done.
- Phase 8 needs every English + Chinese page final (Phases 3–7).
- Within a story, [P] tasks touch different files.

## Parallel opportunities

- Phase 3: T011–T021 and T024–T026 are separate files.
- Phase 8: T070–T082 are one locale each — run as parallel agents, each given the
  finalized en+zh, the claim ledger glossary and the review method.

## Implementation strategy

MVP = Phase 3 (expert-read pages + landing) + Phase 4 (self-hosting guide) in
English and Chinese: that alone removes every Fatal finding. Then findability,
README and authority docs; then the 13 locales; then the polish gates. Each phase
is committed separately so a partial state is always coherent in en and zh.
