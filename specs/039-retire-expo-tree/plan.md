# Implementation Plan: Retiring the Expo tree

**Branch**: `039-retire-expo-tree` | **Date**: 2026-09-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/039-retire-expo-tree/spec.md`

## Summary

Delete the React Native / Expo tree and everything that existed only for it,
in an order that keeps the four shells (`app-web`, `app-desktop`, `app-ios`,
`app-android`) building and testing at every commit — the founder's one
hard constraint (*"删除代码不能影响 app-web app-desktop app-ios app-android"*).
Then leave the repository true: the root `package.json` becomes a tooling
package, the two generators write one mirror each, CI's `app` job runs only
what has a subject, every document either describes the post-cut repo, is
banner'd as history, or is gone, and a committed script proves no Expo
residue remains in code, config or commands. The hostname is **not** moved
in this feature (founder: *"线上先不切换"*); the Pages deployment freezes at
the last Expo commit and the README and runbook say so. The parity
register's owed rows are recorded, not built.

Risk level under `agent-rules/AI-CODING-RULES.md` §3: **High** — data
deletion at repository scale and a production-facing consequence (the
frozen Pages build). So: risk description here, test evidence in
`results.md`, rollback in §Rollback, and the PR asks for a deep human
review.

## Technical Context

**Language/Version**: Node 22 (root tooling, `.mjs`); Rust 1.97.1 (generator
bins under `rust/crates/vela-core/src/bin/`); Bash (`gen-app-icons.sh`); YAML
(`.github/workflows/ci.yml`). No TypeScript remains at the root after the
cut.

**Primary Dependencies (root, after)**: `i18next` and `identicons-esm` (the
npm oracles the corpus gates replay against — `scripts/dump-vectors/*.mjs`,
`scripts/verify-*-parity.mjs`, `scripts/gen-identicon-features.mjs`),
`@noble/curves` + `@noble/hashes` (`scripts/onchain/multikey-safe-gnosis/src/*.ts`
declares no dependencies of its own and resolves from the root),
`typescript` (`packages/vela-sdk`'s `tsc` resolves from the root in CI —
there is no install step for that package). Everything else in today's
`package.json` is dropped (research D2).

**Storage**: none touched. `public/i18n/` and `public/vela_core_bg.*.wasm`
stay where they are (three shells and two gates read them at those paths —
research D5).

**Testing**: the four shells' own gates, unchanged in content —
`app-web/vela-wallet`: `pnpm check && pnpm lint && pnpm test:unit && pnpm
build`; `app-desktop/vela-wallet`: `cargo fmt --check && cargo clippy
--all-targets && cargo test`; `rust/`: `cargo test --workspace --features
vela-core/i18n-all,vela-core/dev-fixtures` + the wasm/Kotlin/Swift steps;
`app-android`: `./gradlew :app:assembleDebug :app:testDebugUnitTest
-PvelaSkipRustBuild`; `app-ios`: `xcodebuild test`. Plus the root tooling
gates (every `scripts/*.mjs` the CI `app` job keeps) and the new
`scripts/check-expo-residue.mjs`. Local vs CI: web, desktop, rust and the
root tooling run locally; Android and iOS run locally where the gitignored
bindings/xcframework already exist on this machine, and on CI regardless
(research D12).

**Target Platform**: the repository itself (a developer's clone, CI runners,
the Cloudflare build of `app-web/vela-wallet`).

**Project Type**: monorepo housekeeping — deletion, generator/CI reshaping,
documentation.

**Performance Goals**: the CI `app` job gets faster, not slower (it loses
`tsc`, `eslint src`, `jest` over 961 files); `npm ci` at the root installs
five packages instead of ~80.

**Constraints**: every commit on the branch leaves all four shells building
(the founder's constraint, SC-392); the `app` CI job keeps its **name**
(branch protection requires a check called `app` — renaming it is a
dashboard action, research D6); no corpus edit (the i18n path-count pin and
the 15-locale gate are untouched); the frozen goldens are untouched; the
onboarding `generated/` barrel header under `app-web` stays byte-identical
(016 FR-017).

**Scale/Scope**: ~1,090 tracked files deleted (`src/` 961, `e2e/` 39,
`modules/` 16, `targets/` 4, `plugins/` 2, `packages/safari-extension/`,
root config ~20, assets/public ~25); ~55 documents touched (data-model.md
carries the disposition list); 1 new script; 4 generator/CI files edited.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template — there is no
ratified constitution. The gates applied are the repository's standing
rules, from `agent-rules/AI-CODING-RULES.md` and the spec's "standing
rules":

| Gate | Status | Note |
| --- | --- | --- |
| One PR solves one problem | pass, with a justified size | one problem (retire the tree), many files; the branch is a **series** of separable commits (spec SC-396), each leaving the shells green |
| Never bypass tests/CI | pass | every CI job that exists today still runs; three steps are removed because their subject is gone, none because it is red |
| Never touch unrelated code | pass | the owed rows (funding sheet, `/pay`, receive request, SDK wallet half) are explicitly NOT built here |
| High-risk changes carry risk + evidence + rollback | pass | this plan §Risk, `results.md`, §Rollback |
| The core decides, the shell performs | n/a | no core rule changes |
| `docs/project-takeover/` rewritten, never deleted | pass | research D8 |
| Corpus edit + regenerated artefacts in one commit | pass | no corpus edit; the generator change and its regenerated (unchanged) outputs land together |

Re-check after Phase 1: unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/039-retire-expo-tree/
├── spec.md
├── plan.md                         # this file
├── research.md                     # D1–D14
├── data-model.md                   # register schema, shared floor, doc disposition list, owed table
├── quickstart.md                   # the validation walk
├── contracts/
│   └── expo-residue-check.md       # rules the committed script enforces
├── checklists/requirements.md
├── tasks.md                        # /speckit-tasks
└── results.md                      # written as the work lands
```

### Source Code (repository root, after the cut)

```text
.
├── agent-rules/
├── app-android/  app-desktop/  app-ios/  app-web/     # the four shells (+ getvela.app, clearsigning under app-web)
├── assets/
│   ├── fonts/PlusJakartaSans_*.ttf                   # app-desktop include_bytes! (assets/images/ is gone: every raster there was an Expo output)
│   ├── i18n/<locale>.json                            # gen-i18n Stage 4 → ios, android, app-web, gates (was public/i18n)
│   └── wasm/vela_core_bg.<hash>.wasm                 # build-web.mjs → sync-wasm, load-wasm-node, scripts/onchain (was public/)
├── design/                                           # icon SVG source, Lottie + illustrations (gen-app-icons, lint-lottie-assets)
├── docs/
├── rust/                                             # vela-core + wasm + uniffi + scripts   (packages/ is gone: vela-sdk and safe-recovery-extension deleted, founder 2026-09-11)
├── scripts/                                          # THE tooling package: package.json + lockfile live here (+ check-expo-residue.mjs)
├── specs/
├── README.md  LICENSE                                # the only markdown at the root; ROADMAP/WHITEPAPER/design-system moved under docs/ (founder ruling)
└── .github/  .specify/  .gitignore                   # (.mcp.json deleted — founder, 2026-09-11; the two web apps keep their own)
```

Gone from the root: `src/`, `e2e/`, `modules/`, `plugins/`, `targets/`,
`dist/`, `test-results/`, `.eas/`, `app.json`, `app.config.js`, `eas.json`,
`metro.config.js`, `index.js`, `index.web.js`, `expo-env.d.ts`,
`fingerprint.config.js`, `.fingerprintignore`, `jest.config.js`,
`jest.setup.js`, `playwright.config.ts`, `tsconfig.json`, `eslint.config.js`,
`keystore.properties.example`, `.verify-rescan.mjs`, `.verify-rescan-ja.mjs`.

**Structure Decision**: no new directories. The one new file is
`scripts/check-expo-residue.mjs`, beside the other gates the CI `app` job
runs.

## Phase 0 — research

Fourteen decisions, each with rationale and the alternative rejected, in
[research.md](research.md). The ones that shape the work:

- **D1** Delete first; the Pages deployment freezes; nothing here moves the
  hostname.
- **D2** The root becomes a five-dependency tooling package; the lockfile is
  regenerated with `npm install`, not hand-edited.
- **D4/D5** One mirror per wire-type family, under `app-web`; `gen-i18n`
  loses Stage 3; `public/` stays.
- **D6** CI's `app` job keeps its name; three steps go; the header is
  rewritten.
- **D7** `targets/safari` + `packages/safari-extension` go; `vela-sdk` stays.
- **D8** Docs: rewrite / history banner / delete, with the dated-erratum
  precedent for the interview docs.
- **D9** The residue check is a script with hard rules for code, config,
  CI and commands, and a report for prose.
- **D12** Verification matrix: what runs locally, what CI proves.

## Phase 1 — design

- [data-model.md](data-model.md): the register's schema, the shared floor
  with its proving commands, the **document disposition list** (every
  tracked file with an Expo/RN/Metro/EAS mention outside the deleted tree,
  each with its disposition), the owed table.
- [contracts/expo-residue-check.md](contracts/expo-residue-check.md): what
  the script fails on, what it reports, its exit codes and output shape.
- [quickstart.md](quickstart.md): the validation walk — baseline, per-commit
  shell gates, the drift-proof runs, the residue check, the fresh-clone walk
  of the local-development doc.

## Execution order (what `/speckit-tasks` will sequence)

1. **Baseline** — record the four shells' gates and the root gates green on
   the branch tip before any deletion (`results.md` §Baseline).
2. **Cut the tree** — `git rm` the directories and root files; delete the
   untracked `dist/` and `test-results/`; strip the dependencies; `npm
   install` to regenerate the lockfile; delete root `tsconfig.json` and
   `eslint.config.js`. One commit. Shells still green (they never read any
   of it — proven, not assumed, by running their gates).
3. **Re-point the generators** — `gen-core-types.mjs` (one mirror each),
   the two bins' defaults, `gen-i18n.mjs` (Stage 3 out, comments true),
   `gen-app-icons.sh` (Expo section out), `.gitignore`. Regenerate; the
   committed `app-web` mirrors and `public/i18n` must not change. One
   commit.
4. **Reshape CI** — the `app` job's three dead steps out, the i18n diff
   paths fixed, the header rewritten, the `rust` job's step renamed. One
   commit. Push; CI green is the evidence for Android and iOS.
5. **Prove the gates still bite** — the two drift runs (i18n, identicon),
   recorded and reverted. No commit.
6. **The residue check** — write `scripts/check-expo-residue.mjs`, wire it
   into `package.json` and the `app` job. It will fail at this point on the
   docs; that is the work list for step 7.
7. **Docs and links** — the disposition list, tier by tier; our own links;
   `assets/fonts/README.md`; the README and runbook's statement of where
   production is. The residue check goes green. Commits per tier.
8. **`results.md`** — baseline, per-step gate output, drift proofs, residue
   check output, the register and owed table final state, the rulings with
   dates. PR.

## Risk

- **The Pages deployment freezes** (by design). Mitigation: the README and
  `05-deployment-runbook.md` state it, name the frozen commit, and point at
  the move checklist; the founder pauses Pages auto-deploys in the
  dashboard (their action; asked for in the PR).
- **A shared-floor consumer nobody grepped.** Mitigation: step 2 runs every
  shell's gate before committing, and CI runs Android/iOS on the push; the
  shared-floor table in data-model.md names the proving command per row.
- **`typescript` / `@noble/*` at the root.** Two packages resolve from the
  root without declaring it (D2). Mitigation: kept, and the reason is
  written into the README's tooling section so the next cleanup does not
  drop them.
- **Lockfile churn.** `npm install` after stripping ~75 packages rewrites
  most of `package-lock.json`. Mitigation: its own commit; `npm ci` proven
  green from a fresh clone in step 8.
- **A doc scrubbed of the word, not the meaning.** Mitigation: D8's rule is
  disposition-per-file with the deciding line recorded; the residue check
  hard-fails on dead commands in any doc, so a rewritten doc that still says
  `npx expo start` fails CI.

## Rollback

`git revert` of the step-2 commit restores the tree, the config, the
dependencies and the lockfile as one unit; the step-3/4 commits revert
independently (the generators and CI are written so that both the
one-mirror and the two-mirror states are internally consistent). The
hostname is untouched by this feature, so there is nothing to roll back in
production; the Pages project's last deployment remains the fallback for
`wallet.getvela.app` until the Worker takes it.

## Complexity Tracking

No constitution violations to justify. The one size exception (a large PR)
is justified above under Constitution Check.
