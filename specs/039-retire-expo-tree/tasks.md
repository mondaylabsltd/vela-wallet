# Tasks: Retiring the Expo tree

**Input**: Design documents from `/specs/039-retire-expo-tree/`

**Prerequisites**: plan.md, spec.md, research.md (D1–D14), data-model.md (shared floor, doc disposition list), contracts/expo-residue-check.md, quickstart.md

**Tests**: no new test suites are requested. The "tests" of this feature are the four shells' existing gates, run after every commit (the founder's constraint), plus the drift proofs and the residue check the spec names.

**Organization**: the spec has two Parts rather than user stories. Tasks are grouped by the plan's execution order; the story labels map as: **US1** = Part A, the cut (the tree, the toolchain, the dependencies); **US2** = Part A, the generators and CI; **US3** = Part A, docs, links and the residue check; **US4** = Part B, the records (register, owed table, `results.md`). US1 alone is the MVP: after it, the repository has no Expo code and all four shells are green.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4 as above

## Path Conventions

Repository root unless stated. `app-web` = `app-web/vela-wallet`; `desktop` = `app-desktop/vela-wallet`.

---

## Phase 1: Setup — baseline (before any deletion)

**Purpose**: record what green looks like on the branch tip, so every later gate has a comparison (quickstart §0).

- [ ] T001 Create `specs/039-retire-expo-tree/results.md` with a `## Baseline @ <sha>` section and the date
- [ ] T002 [P] Run the app-web gates (`pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build` in `app-web/vela-wallet`) and record pass/fail counts in `specs/039-retire-expo-tree/results.md`
- [ ] T003 [P] Run the desktop gates (`cargo fmt --all --check && cargo clippy --all-targets && cargo test` in `app-desktop/vela-wallet`) and record in `specs/039-retire-expo-tree/results.md`
- [ ] T004 [P] Run the rust workspace tests (`cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` in `rust/`) plus `node rust/scripts/build-web.mjs --check && node rust/scripts/verify-web.mjs` and record in `specs/039-retire-expo-tree/results.md`
- [ ] T005 [P] Run the root tooling gates in quickstart §0 (`dump:vectors` diff, `gen-identicon-features` diff, `gen-i18n` diff, `lint-i18n-corpus`, Lottie self-test + lint, `check-native-reachability`, `verify-identicon-parity`, `verify-i18n-parity`) and record in `specs/039-retire-expo-tree/results.md`
- [ ] T006 [P] Run the Android unit tests locally (`./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild` in `app-android/vela-wallet`; `rust/bindings/kotlin` exists on this machine) and record in `specs/039-retire-expo-tree/results.md`
- [ ] T007 [P] Build the iOS app locally (`xcodebuild build -project VelaWallet.xcodeproj -scheme VelaWallet -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO` in `app-ios/VelaWallet`; `VelaCoreKit/Artifacts` exists on this machine) and record in `specs/039-retire-expo-tree/results.md`

**Checkpoint**: `results.md` §Baseline lists every gate with its result; any pre-existing red is named so it is not blamed on the cut.

---

## Phase 2: Foundational — nothing to build

There is no shared infrastructure to create; the cut itself is the first increment. (The residue-check script is written in US3 because it is only meaningful once the docs pass begins.)

---

## Phase 3: US1 — the cut (Priority: P1) 🎯 MVP

**Goal**: the Expo tree, its toolchain, its assets/public files and its dependencies are gone, in one revertible commit, and all four shells are still green.

**Independent test**: quickstart §1 — every §0 gate repeats with identical results; `git ls-files src e2e modules plugins targets packages/safari-extension | wc -l` prints 0; `npm ci` installs the tooling package clean.

- [ ] T008 [US1] `git rm -r` the directories `src/`, `e2e/`, `modules/`, `plugins/`, `targets/`, `packages/safari-extension/`, `.eas/`
- [ ] T009 [US1] `git rm` the root files `app.json app.config.js eas.json metro.config.js index.js index.web.js fingerprint.config.js .fingerprintignore jest.config.js jest.setup.js playwright.config.ts tsconfig.json eslint.config.js keystore.properties.example scripts/reset-project.js scripts/fix-cf-pages-assets.js scripts/jest-skipped-reporter.js .verify-rescan.mjs .verify-rescan-ja.mjs` (research D3, D13)
- [ ] T010 [US1] `git rm` the Expo-only assets and public files listed in research D10: `assets/expo.icon/`, `assets/images/{splash-icon,android-icon-background,android-icon-foreground,android-icon-monochrome,expo-badge,expo-badge-white,expo-logo,react-logo,react-logo@2x,react-logo@3x,logo-glow,tutorial-web}.png`, `assets/images/tabIcons/`, `assets/fonts/Inter-*.ttf`, `assets/templates/payroll-template.csv`, `public/__vela-opener.html`, `public/qr-test.html`, `public/test-qr.jpg`, `public/og-image.png`, `public/zbar.wasm` — after confirming with `git grep` that no surviving file names each one
- [ ] T011 [US1] Remove the Inter row from `assets/fonts/README.md`
- [ ] T012 [US1] Rewrite `package.json` as the tooling package per research D2: scripts kept/dropped as listed; `dependencies` = `@noble/curves`, `@noble/hashes`, `i18next`, `identicons-esm` at their current pins; `devDependencies` = `typescript` at its current pin; add `"check:expo-residue": "node scripts/check-expo-residue.mjs"` (the script arrives in T031; the entry is harmless until then — or add it in T033, either is fine); remove `"main"`
- [ ] T013 [US1] Delete `node_modules/` and `package-lock.json`, run `npm install` at the root to regenerate the lockfile, then `npm ci` to prove it installs clean
- [ ] T014 [US1] Delete the untracked `dist/` and `test-results/` at the root (`rm -rf dist test-results`)
- [ ] T015 [US1] Edit `.gitignore`: remove `.expo/`, `dist/`, `web-build/`, `expo-env.d.ts`, `.metro-health-check*`, `/ios`, `/android`, `e2e/screenshots/`, `test-results/` and the `# Expo` / `# Metro` / `# generated native folders` headings (research D13); keep everything else
- [ ] T016 [US1] Run every quickstart §0 gate again plus quickstart §1's three lines; record in `specs/039-retire-expo-tree/results.md` §"After the cut"; any difference from §Baseline stops the work and is recorded as a missed shared-floor consumer
- [ ] T017 [US1] Commit: `039: the Expo tree, its toolchain, its assets and its dependencies — gone; four shells green` (one commit for T008–T015)

**Checkpoint**: MVP. The repository has no React Native / Expo code; the shells did not notice.

---

## Phase 4: US2 — one mirror each, and a CI that runs what exists (Priority: P2)

**Goal**: the generators write to the surviving places only; CI's `app` job runs only steps with a subject; the `rust` job's step is named for the path it checks.

**Independent test**: quickstart §2 — `gen-core-types.mjs --check` zero diff; `gen-i18n.mjs` zero diff under `rust/…` and `public/i18n`; `gen-app-icons.sh` zero diff; `pnpm check` in app-web green; then quickstart §3 — a pushed CI run with all eight jobs green.

- [ ] T018 [P] [US2] Edit `rust/scripts/gen-core-types.mjs`: remove the `src/services/onboarding-core/generated` entry from the `onboarding` target's `outDirs` and the `src/services/wallet-state-core/generated` entry from `wallet-state`'s; rewrite the header comment paragraph *"A target may mirror into SEVERAL directories…"* and the `session` target's comment about "the Expo client" to describe one mirror per family; keep the `onboarding` `barrelHeader` string byte-identical (research D4)
- [ ] T019 [P] [US2] Edit `rust/crates/vela-core/src/bin/generate_onboarding_bindings.rs` (`:23`) and `rust/crates/vela-core/src/bin/generate_wallet_state_bindings.rs` (`:75`): default `out_dir` becomes `../../../app-web/vela-wallet/src/lib/onboarding/generated` and `../../../app-web/vela-wallet/src/lib/core/generated` respectively; check `generate_session_bindings.rs` for a `src/` default and fix it the same way if present
- [ ] T020 [P] [US2] Edit `scripts/gen-i18n.mjs` per research D5: delete `RESOURCES_FILE` (`:40`), the Stage 3 emitter block (from the `// Stage 3 —` comment at `:451` through the `writeFileSync(RESOURCES_FILE, …)` call ending near `:520`) and its console line (`:656`); rewrite the header stage list (drop Stage 3, renumber or leave gaps with a note), the `ASSET_DIR` comment (`:42-44`) to name `app-ios`, `app-android`, `app-web` and the gates as the readers of `public/i18n`, and the two remaining `resources.ts` mentions (`:48`, `:383`); confirm the path-count pin and every assertion are untouched
- [ ] T021 [P] [US2] Edit `scripts/gen-app-icons.sh`: delete the `Expo app` section (`:106`–the line before the next `# ----` section) and the `assets/expo.icon/Assets/icon.png` verification entry (`:225`); rewrite the header line `:7` to name the consumers that remain (app-ios, app-android, getvela.app, desktop)
- [ ] T022 [US2] Run `node rust/scripts/gen-core-types.mjs` and `node rust/scripts/gen-core-types.mjs --check`, `node scripts/gen-i18n.mjs`, `./scripts/gen-app-icons.sh`; `git status --short` must show only the four edited files (no regenerated output changed); record in `specs/039-retire-expo-tree/results.md`
- [ ] T023 [US2] Commit: `039: one mirror per wire-type family; gen-i18n loses the resources.ts stage; the icon script forgets Expo`
- [ ] T024 [US2] Edit `.github/workflows/ci.yml` per research D6: in the `app` job remove the `Typecheck`, `Lint` and `Unit tests` steps; in the "i18n artefacts match the corpus" step remove `src/i18n/resources.ts` from the `git diff` path list and rewrite its comment sentence *"the TypeScript resources the React Native app imports"*; rewrite the file header (`:1-30`) to describe the job as the tooling gates and to state that the four shells are built by their own jobs; in the `rust` job rename the step "Onboarding wire types are current (src/services/onboarding-core/generated)" to name `app-web/vela-wallet/src/lib/onboarding/generated`; leave the `Build HTTPS wallet SDK` step and every other job untouched; do not rename the `app` job
- [ ] T025 [US2] Commit `039: CI — the app job runs the gates that have a subject` and push the branch; wait for the Actions run; record the run URL and the eight job results in `specs/039-retire-expo-tree/results.md` §CI (quickstart §3)

**Checkpoint**: CI proves Android and iOS on a runner; the generators are single-mirror.

---

## Phase 5: US3 — the repository tells the truth (Priority: P3)

**Goal**: every document is rewritten, banner'd or deleted per the disposition list; our own links no longer name Expo-only paths; a committed script fails CI on any Expo residue in code, config or commands.

**Independent test**: `node scripts/check-expo-residue.mjs --self-test && node scripts/check-expo-residue.mjs` exit 0; quickstart §6 clean-tree tests 1–2; quickstart §7 fresh-clone walk of `02-local-development.md`; quickstart §8 production statement present in README and runbook.

### 5a — the drift proofs (SC-393), before any doc moves

- [ ] T026 [US3] Run the two drift proofs in quickstart §4 (one i18n locale value, one identicon vector), capture the two `exit=1` lines and the clean `git status` after reverting, and record them in `specs/039-retire-expo-tree/results.md` §"The gates still bite"

### 5b — the residue check

- [ ] T027 [P] [US3] Write `scripts/check-expo-residue.mjs` implementing the four hard rules, the prose report and the `--self-test` mode exactly as `specs/039-retire-expo-tree/contracts/expo-residue-check.md` specifies (Node 22, `node:` imports only, reads `git ls-files`)
- [ ] T028 [P] [US3] Create the self-test fixtures under `scripts/__fixtures__/expo-residue/`: `rule1-path.txt` (a path list containing `src/x.ts`), `rule2-package.json` (a `dependencies` entry `expo`), `rule3-ci.yml` (a `run: npx expo lint` step), `rule4-doc.md` (a living doc with `npm run build:web`), `banner-doc.md` (a `> **History (2026-09-11).**` banner in its first 20 lines followed by a fenced ```` ```sh npx expo start ```` block that must NOT trip rule 4)
- [ ] T029 [US3] Add the `check:expo-residue` script to `package.json` (if not already in T012) and a `Check for Expo residue` step to the CI `app` job in `.github/workflows/ci.yml` after the i18n gates: `node scripts/check-expo-residue.mjs --self-test && node scripts/check-expo-residue.mjs`
- [ ] T030 [US3] Run `node scripts/check-expo-residue.mjs --self-test` (must pass) and `node scripts/check-expo-residue.mjs` (expected to FAIL on rule 4 with the list of docs that still carry dead commands); save that list into `specs/039-retire-expo-tree/results.md` §"Residue, before the doc pass" — it is the work list for 5c–5e

### 5c — front doors and the handover authority (disposition R)

- [ ] T031 [P] [US3] Rewrite `README.md`: delete `## Architecture (the Expo app, shipping today)` and its diagram (`:30-62`); rewrite `## The new architecture` / `### Why we are leaving Expo` into past tense ("why we left") and its `### Where it stands` to the post-cut state (shells shipping, Expo retired in 039); rewrite `## Get Started (the Expo app)` (`:136-154`) as "Get Started" pointing at each shell's README/commands; rewrite the `## Platform Support` table (`:155-168`) so its columns are app-web / app-ios / app-android / app-desktop with no Expo modules; rewrite the `:205` icon line and `:227` per research D14 (frozen Pages sentence with the frozen commit sha and today's date); add a short "Root tooling package" paragraph under Build/Get Started naming why `i18next`, `identicons-esm`, `@noble/*`, `typescript` live at the root
- [ ] T032 [P] [US3] Rewrite `docs/project-takeover/05-deployment-runbook.md`: the 部署单元一览 web row → "Cloudflare Worker `vela-wallet-web` from `app-web/vela-wallet` (CF builds `pnpm build`); `wallet.getvela.app` still on the frozen Pages deployment of commit `<sha>` as of 2026-09-11 — move checklist in spec 039 Part B"; `## Web 钱包发布` rewritten for the Worker; `## Android 发布` and `## iOS 发布` → a dated note that EAS is retired and packaging lives in `app-android/` and `app-ios/` (link their READMEs); add a rollback line naming the Pages last deployment; remove the `npx expo lint` / `npm audit` Expo-baseline lines in 发布前检查
- [ ] T033 [P] [US3] Rewrite `docs/project-takeover/02-local-development.md`: `## 从零启动` → root `npm ci` + the four shells' own start commands; `## 原生构建与真机运行` → `app-android` / `app-ios` commands (bindings + xcframework prerequisites from `ci.yml`); `## 常用命令与实测结果` → the surviving root scripts; `## 测试环境:Parallel Space` → `app-web` `/[locale]/parallel` and the desktop dev-fixtures; `## 故障注入` → `app-web` `vela.*` console (`lib/services/dev-console.ts`); `## 数据存储` → app-web IndexedDB + desktop JSON file; keep the getvela.app section
- [ ] T034 [P] [US3] Rewrite `docs/project-takeover/01-system-overview.md`: `## 技术栈` table (framework rows → the four shells + vela-core), `## 仓库布局` (post-cut tree from plan.md §Source Code), `## CI/CD` (jobs as in `ci.yml` after T024), and the `:34`, `:73`, `:81` sentences
- [ ] T035 [P] [US3] Rewrite `docs/project-takeover/04-production-readiness.md` rows `:44,66,72,73` (Metro warnings, `expo lint` counts, `@expo/*` audit debt, `expo config`) as resolved-by-retirement with the date, and `docs/project-takeover/14-human-progress.md` learning-map rows D8/U0/U5 (`:26,37,42`) to the shells' equivalents
- [ ] T036 [P] [US3] Add a dated erratum block (PR #169 precedent) at the top of `docs/project-takeover/11-interview-answer-key.md`, `10-interview-question-bank.md`, `12-manual-coding-drills.md`, `13-architecture-decisions.md`, `07-maintenance-guide.md`, `08-open-issues.md`, `docs/CONTENT-SOURCE-100-CLUES.md`, `docs/MANUAL-TEST-100-CLUES.md`, `docs/TEST-OUTLINE.md`, `docs/requirements/M02-text-scale-locale-formatting.md`, `docs/requirements/E06-display-currency-formatting.md`: *"勘误（2026-09-11，spec 039）：本文引用的 Expo/React Native 应用、`src/**` 路径、`npm run build:web` / `expo lint` / `jest` 等命令已随 Expo 树退役；对应实现见 app-web/vela-wallet、app-desktop、app-ios、app-android。正文保留作历史记录。"* — and wrap any fenced command block that names a dead command so rule 4 sees the banner (the banner must be in the first 20 lines)

### 5d — history banners and deletions (dispositions H, D)

- [ ] T037 [P] [US3] Prepend the `> **History (2026-09-11).** …` banner (research D8 wording, with the living-description link) to `docs/safari-extension/ARCHITECTURE.md` and the eight runbooks `docs/safari-extension/{PHASE-A,PHASE-B,PHASE-3,R1-INCREMENT-1,R1-INCREMENT-2,R1-INCREMENT-3,R1-INCREMENT-4}-RUNBOOK.md` (link: `app-ios/` and spec 039 "Two packages"), `docs/dapp-browser/ARCHITECTURE.md` (link: no shell ships an in-app browser; spec 032 Part C ruling), `docs/qr-scanner-web.md` (link: `app-web/vela-wallet/src/lib/services/qr-decode.ts`), `docs/dynamic-amount-display.md` (link: `app-web/vela-wallet/src/lib/ui/`), `docs/https-web-wallet.md` (banner says the wallet half is owed — spec 039 Part B — and the source map's three `src/` lines are re-pointed to `packages/vela-sdk/src/*` + "owed")
- [ ] T038 [P] [US3] `git rm docs/PARALLEL-SPACE-E2E-PLAYBOOK.md`; grep `docs/ README.md agent-rules/` for links to it and re-point them at `docs/PARALLEL-SPACE.md` + `app-web/vela-wallet/e2e/`
- [ ] T039 [P] [US3] Edit `docs/KNOWN-BUGS.md`: mark BUG-4 (`:175-186`, the Metro / `react-native-get-random-values` race) *resolved 2026-09-11 — the runtime it lived in was retired (spec 039)*; date the other Expo mentions (`:33` and any others) the same way

### 5e — one-line and comment fixes (dispositions R/C on the remaining files)

- [ ] T040 [P] [US3] Fix the single-line mentions: `DESIGN_SYSTEM.md:230` (`lucide-react-native` → "icons: `app-web` inline SVG via `lib/ui`, desktop `assets/`, iOS SF Symbols / asset catalog"), `app-desktop/vela-wallet/README.md:540,555` (drop "the Expo app's" / "Expo," from the icon prose), `agent-rules/CLAUDE-AUTO-TEST.md:79` (the "React Native / Mobile USB Device" section → `app-android` adb + `app-ios` simulator, referencing the device memory recipes), `rust/README.md:104` (Metro / CF Pages rationale → past tense, naming `app-web`'s `sync-wasm.mjs` as the reader), `docs/NATIVE-LAUNCH-CHECKLIST.md:3,119-120`, `docs/store-submission/privacy-and-review.md:186,195,199`, `docs/localization.md` (its eight `src/i18n/*` citations → corpus + `public/i18n` + `app-web` engine)
- [ ] T041 [P] [US3] Rewrite the comments that state a constraint that no longer exists: `rust/scripts/build-web.mjs` (the lines that say Metro cannot bundle wasm / CF Pages drops `node_modules` as the reason for `public/` → "`public/` is where `app-web`'s `sync-wasm.mjs`, `rust/scripts/load-wasm-node.mjs` and `scripts/onchain` read it"), `rust/crates/vela-core/Cargo.toml`, `rust/crates/vela-core-wasm/Cargo.toml`, `rust/crates/vela-core-uniffi/Cargo.toml`, `rust/crates/vela-core/src/i18n/mod.rs`, `rust/crates/vela-core-wasm/src/lib.rs` — each: keep if past-tense provenance, rewrite if it names Expo/Metro/Hermes as a live constraint; leave the `app-web/**` and `app-desktop/**` provenance comments as they are (disposition C)
- [ ] T042 [P] [US3] Our own links (research D11): change the six `wallet.getvela.app/onboarding[?mode=create]` hrefs in `app-web/getvela.app/src/routes/+page.svelte:299,321,329,932`, `app-web/getvela.app/src/lib/components/SiteHeader.svelte:45`, `app-web/getvela.app/src/content/docs/install.md:12`, `app-web/getvela.app/src/content/docs/create-wallet.md:13` to `https://wallet.getvela.app/`; add a comment above the redirect in `app-web/getvela.app/src/routes/pay/+server.ts` naming the owed `/pay` route (spec 039 Part B); add one sentence to `packages/vela-sdk/README.md` after `:4` saying the wallet-side surface is owed (spec 039 Part B); run `bun run check` in `app-web/getvela.app`
- [ ] T043 [US3] Run `node scripts/check-expo-residue.mjs` — exit 0 required; if rule 4 still lists a line, fix that document (rewrite, banner or delete per the disposition list) and re-run; record the final report (the prose-mention count per file) in `specs/039-retire-expo-tree/results.md` §"Residue, after"
- [ ] T044 [US3] Run quickstart §6 (clean-tree tests 1 and 2) and record; run quickstart §8 and record the README/runbook lines
- [ ] T045 [US3] Commit in tiers: `039: the front doors and the handover docs tell the truth` (T031–T036), `039: history banners, one deletion, the known-bugs ledger` (T037–T039), `039: comments, links and the residue check` (T027–T030, T040–T042)
- [ ] T046 [US3] Fresh-clone walk (quickstart §7): clone the branch into a temp dir, follow `docs/project-takeover/02-local-development.md` top to bottom, reach a running web wallet and a running desktop app and a green root gate set; record time and any misleading sentence in `specs/039-retire-expo-tree/results.md`; fix the sentence if one is found and amend the docs commit

**Checkpoint**: the residue check is green in CI; a stranger can follow the local-development doc from a fresh clone.

---

## Phase 6: US4 — the records (Priority: P4)

**Goal**: `results.md` carries everything the spec's FR-400 lists; the register and the owed table are in their final state; the rulings are dated.

**Independent test**: `results.md` has the sections Baseline, After the cut, Generators, CI (run URL), The gates still bite, Residue before/after, Clean tree, Fresh clone, Production statement, Register (final), Owed (final), Rulings, Handover.

- [ ] T047 [US4] Copy the parity register from `spec.md` Part B into `specs/039-retire-expo-tree/results.md` §Register with every row's final status (the *verify* rows checked once: EIP-7708 feed path against `rust/crates/vela-core/src/app/` feed rules; `[locale]/import` linkage via `git grep "/import"` in `app-web/vela-wallet/src`) and note any change
- [ ] T048 [US4] Copy the owed table into `results.md` §Owed with each row marked *still owed* (expected) and the rulings section listing: delete-first (2026-09-11), no Expo-web compatibility (2026-09-11), WalletPair/remote-inject out (spec 027, 2026-09-04), analytics dropped by default, Safari packages deleted / SDK kept (defaults)
- [ ] T049 [US4] Write `results.md` §Handover: the commit series with shas, the CI run URL, what the founder must do in the dashboard (pause Pages auto-deploys; the move checklist when chosen), and the rollback in one paragraph
- [ ] T050 [US4] Commit `039: results` and open the PR against `main` using the `agent-rules/AI-CODING-RULES.md` template (What changed / Why / Human owner / AI assistance / Risk: High / Tests / Reviewer focus / Rollback), asking for the `android` and `ios` job results to be read before merge

---

## Phase 7: Polish

- [ ] T051 [P] Update the memory file `~/.claude/projects/-Volumes-data-production-vela-wallet/memory/project_039_retire_expo.md` with the delivered state (commit shas, what is still owed, the dashboard action pending)
- [ ] T052 [P] Re-read `specs/039-retire-expo-tree/checklists/requirements.md` and tick the `/speckit-plan` box; add a `/speckit-implement` line with the date

---

## Dependencies

- Phase 1 (T001–T007) before T008: the baseline is the comparison.
- US1 (T008–T017) is sequential except T008–T011 which touch disjoint paths; T016 must follow T013–T015; T017 commits it all.
- US2 depends on US1 (the generators' `src/` targets must already be gone for `--check` to be meaningful). T018–T021 are parallel; T022 after them; T024 after T023; T025 pushes.
- US3 depends on US2 (the CI step in T029 edits the file T024 rewrote). T026 first (it needs the gates but not the docs). T027–T028 parallel; T029 after both; T030 after T029. 5c/5d/5e tasks (T031–T042) are parallel across files; T043 after all of them; T044–T046 after T043.
- US4 depends on US3 (the residue report and the fresh-clone result feed `results.md`).
- Phase 7 after T050.

## Parallel execution examples

- **Baseline**: T002, T003, T004, T005, T006, T007 in six shells at once (they are CPU-heavy; two cargo trees plus vitest plus gradle plus xcodebuild — run T002/T005 together, then T003/T004, then T006/T007 if the machine complains).
- **US2**: T018, T019, T020, T021 are four different files — one pass each, then T022 once.
- **US3**: T031–T042 are twelve tasks over disjoint files; the doc rewrites (T031–T036) are the long ones and can be split across agents by file; T037–T042 are mechanical.

## Implementation strategy

1. **MVP = US1.** After T017 the repository has no Expo code and the shells are proven green locally. If nothing else lands, that alone answers the founder's *"我现在就想先删除掉"*.
2. **US2 next**, because a generator that writes to a missing path and a CI job that runs `tsc` over nothing are the first things a fresh clone trips on.
3. **US3 is the bulk of the hours** (fifty-odd documents). The residue check is written *before* the doc pass so its rule-4 output is the work list and its green is the exit criterion.
4. **US4 is bookkeeping** and is written as the work lands, not at the end.
