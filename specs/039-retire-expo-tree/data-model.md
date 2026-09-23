# Data model: Retiring the Expo tree

No runtime data changes. The "entities" here are the working lists the
feature is executed and verified against.

## 1. Parity register (schema)

Lives in `spec.md` Part B; its final state is copied into `results.md`.

| Field | Values |
| --- | --- |
| capability | free text, one user-facing thing |
| expo_evidence | `path:line` under `src/` |
| appweb_evidence | `path:line` under `app-web/vela-wallet/src/`, or none |
| status | `present` · `ahead` · `partial` · `verify` · `owed` · `dropped` |
| ruling | required when `dropped`: date + where (spec 039 Input / spec 027 Input) |

Invariant (FR-397): no row may read `MISSING` without `owed` or a ruling.

## 2. Shared floor (kept paths and the command that proves each consumer)

| Path | Consumer | Proving command |
| --- | --- | --- |
| `assets/i18n/*.json` (was `public/i18n`) | app-ios | `cd app-ios/VelaWallet && xcodebuild test …` (CI `ios`) / `app-ios/scripts/bundle-catalogs.sh` |
| | app-android | `./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild` (CI `android`) |
| | app-web | `cd app-web/vela-wallet && pnpm build` |
| | gates | `npm run verify:i18n`; `node rust/scripts/verify-web.mjs` |
| `assets/wasm/vela_core_bg.<hash>.wasm` (was `public/`) | app-web | `cd app-web/vela-wallet && pnpm run sync:wasm -- --check` |
| | rust scripts | `node rust/scripts/verify-web.mjs` |
| | scripts/onchain | `bun run src/status.ts` (manual; network) |
| `assets/fonts/PlusJakartaSans_*.ttf` | app-desktop | `cd app-desktop/vela-wallet && cargo build` |
| `design/icon/*.svg` | icon pipeline (app-ios, app-android, getvela.app) | `./scripts/gen-app-icons.sh` then `git diff --stat` (zero) — `assets/images/*` were the script's Expo outputs and are deleted |
| `scripts/node_modules` (`i18next`, `identicons-esm`) | gates | `npm run dump:vectors && git diff --exit-code rust/crates/vela-core/tests/vectors/`; `npm run verify:identicon`; `npm run gen:identicon-features && git diff --exit-code` |
| `scripts/node_modules` (`@noble/curves`, `@noble/hashes`) | scripts/onchain | `bun run src/generate.ts` (manual) |
| `design/`, `scripts/__fixtures__/` | Lottie lint | `npm run lint:lottie:self-test && npm run lint:lottie` |
| `app-android`, `app-ios`, `app-desktop` navigation roots | reachability | `node scripts/check-native-reachability.mjs` |

## 3. Document disposition list

Every tracked file outside `src/ e2e/ modules/ plugins/ targets/
packages/safari-extension/` with a match for
`\bexpo\b|react-native|react native|\bmetro\b|eas build|\bhermes\b`
(case-insensitive), ranked by hit count on 2026-09-11. Disposition:
**R** rewrite · **H** history banner · **E** dated erratum at top · **D**
delete · **C** comment stays (past-tense rationale) · **G** goes with the
deletion (file itself deleted).

| Hits | File | Disp. | Deciding line |
| --- | --- | --- | --- |
| 44 | `package.json` | R | D2 — tooling package |
| 33 | `docs/project-takeover/11-interview-answer-key.md` | E | question bank; PR #169 precedent |
| 19 | `README.md` | R | §Architecture (Expo) out; §Why past tense; `:227` truth of the day (D14); Get Started rewritten for the shells; Platform Support table rewritten |
| 16 | `docs/safari-extension/R1-INCREMENT-1-RUNBOOK.md` | H | `create-target safari` procedure |
| 12 | `docs/project-takeover/02-local-development.md` | R | 从零启动 / 原生构建 / 常用命令 sections → the four shells' commands |
| 12 | `.fingerprintignore` | G | |
| 11 | `docs/safari-extension/ARCHITECTURE.md` | H | design record |
| 11 | `docs/KNOWN-BUGS.md` | R | BUG-4 (Metro polyfill race) marked resolved-by-retirement; other Expo mentions dated |
| 11 | `.github/workflows/ci.yml` | R | D6 |
| 10 | `app.json` | G | |
| 9 | `docs/project-takeover/05-deployment-runbook.md` | R | web row → frozen Pages + Worker (D14); Android/iOS EAS sections → history note pointing at the native shells' packaging |
| 8 | `docs/localization.md` | R | `src/i18n/*` citations → corpus + `public/i18n` + app-web engine |
| 7 | `scripts/gen-i18n.mjs` | R | D5 |
| 7 | `scripts/gen-app-icons.sh` | R | D10 |
| 7 | `rust/README.md` | R | `:104` Metro/CF Pages rationale → past tense; other mentions checked |
| 7 | `docs/project-takeover/01-system-overview.md` | R | 技术栈 / 仓库布局 / CI/CD tables |
| 7 | `docs/dapp-browser/ARCHITECTURE.md` | H | describes `modules/vela-wallet-webview` |
| 6 | `rust/scripts/build-web.mjs` | C/R | comments about why `public/` — rewrite the ones that name Metro as a live constraint |
| 6 | `docs/qr-scanner-web.md` | H | documents the Expo `QRScanner` + `public/qr-test.html`; living code is `app-web/.../qr-decode.ts` |
| 6 | `docs/project-takeover/13-architecture-decisions.md` | E | |
| 6 | `docs/project-takeover/08-open-issues.md` | E | |
| 6 | `app-web/vela-wallet/src/lib/services/records.ts` | C | provenance comments |
| 5 | `docs/dynamic-amount-display.md` | H | Expo component design |
| 5 | `app-web/vela-wallet/src/lib/settings/core/network-admin-executor.ts` | C | |
| 5 | `app-web/vela-wallet/src/lib/services/preferences.svelte.ts` | C | |
| 5 | `.gitignore` | R | D13 |
| 4 | `scripts/fix-cf-pages-assets.js` | G | |
| 4 | `rust/crates/vela-core/Cargo.toml` | C/R | check each |
| 4 | `public/__vela-opener.html` | G | |
| 4 | `index.js` | G | |
| 4 | `docs/safari-extension/R1-INCREMENT-3-RUNBOOK.md` | H | |
| 4 | `docs/safari-extension/PHASE-3-RUNBOOK.md` | H | |
| 4 | `docs/project-takeover/12-manual-coding-drills.md` | E | |
| 4 | `docs/project-takeover/10-interview-question-bank.md` | E | |
| 4 | `docs/project-takeover/07-maintenance-guide.md` | E | |
| 4 | `docs/project-takeover/04-production-readiness.md` | R | Metro warnings / `expo lint` / `@expo/*` audit rows → removed or dated |
| 4 | `docs/PARALLEL-SPACE-E2E-PLAYBOOK.md` | D | manual for `e2e/` |
| 4 | `docs/NATIVE-LAUNCH-CHECKLIST.md` | R | `:3`, `:119-120` → the native shells' device builds |
| 4 | `docs/CONTENT-SOURCE-100-CLUES.md` | E | fact bank; erratum |
| 4 | `app-web/vela-wallet/src/routes/[locale]/wallet/+page.svelte` | C | |
| 4 | `app-web/vela-wallet/src/lib/services/safe-transaction.ts` | C | |
| 4 | `app-web/vela-wallet/src/lib/services/approval-guard.ts` | C | |
| 3 | `tsconfig.json` | G | D3 |
| 3 | `scripts/reset-project.js` | G | |
| 3 | `rust/scripts/gen-core-types.mjs` | R | D4 |
| 3 | `fingerprint.config.js` | G | |
| 3 | `docs/store-submission/privacy-and-review.md` | R | `:186,195,199` → the native shells |
| 3 | `docs/safari-extension/R1-INCREMENT-4-RUNBOOK.md` | H | |
| 3 | `docs/safari-extension/PHASE-B-RUNBOOK.md` | H | |
| 3 | `docs/safari-extension/PHASE-A-RUNBOOK.md` | H | |
| 3 | `docs/requirements/M02-text-scale-locale-formatting.md` | E | |
| 3 | `docs/requirements/E06-display-currency-formatting.md` | E | |
| 3 | `app.config.js` | G | |
| 3 | `app-web/vela-wallet/tsconfig.json` | C | |
| 3 | `app-web/vela-wallet/src/lib/wallet/qr.ts` | C | |
| 3 | `app-web/vela-wallet/src/lib/services/qr-decode.ts` | C | |
| 3 | `app-web/vela-wallet/src/lib/services/preferences-store.test.ts` | C | |
| 3 | `app-web/vela-wallet/src/lib/services/chain-registry.ts` | C | |
| 3 | `app-web/getvela.app/bun.lock` | C | lockfile |
| 3 | `app-desktop/vela-wallet/src/executor/storage.rs` | C | |
| 2 | `rust/crates/vela-core/src/i18n/mod.rs` | C | |
| 2 | `rust/crates/vela-core-wasm/src/lib.rs` | C | |
| 2 | `rust/crates/vela-core-wasm/Cargo.toml` | C | |
| 2 | `rust/crates/vela-core-uniffi/Cargo.toml` | C | |
| 2 | `playwright.config.ts` | G | |
| 2 | `packages/safari-extension/expo-target.config.template.js` | G | |
| 2 | `index.web.js` | G | |
| 2 | `eslint.config.js` | G | |
| 2 | `docs/safari-extension/R1-INCREMENT-2-RUNBOOK.md` | H | |
| 2 | `docs/project-takeover/14-human-progress.md` | R | learning-map rows D8/U0/U5 |
| 2 | `docs/MANUAL-TEST-100-CLUES.md` | E | |
| 2 | `app-web/**` (11 more files), `app-desktop/**` (2 more) | C | provenance comments |
| 1 | `DESIGN_SYSTEM.md:230` | R | `lucide-react-native` → the shells' icon sources |
| 1 | `app-desktop/vela-wallet/README.md:540,555` | R | icon pipeline prose |
| 1 | `agent-rules/CLAUDE-AUTO-TEST.md:79` | R | RN device section → native shells |
| 1 | `docs/TEST-OUTLINE.md:54` | E | |
| 1 | `docs/https-web-wallet.md` | H+R | banner: wallet half owed; source map re-pointed |
| — | `assets/fonts/README.md` | R | Inter row removed |
| — | `packages/vela-sdk/README.md:4` | R | note: wallet half owed (spec 039 Part B) |
| — | `app-web/getvela.app/src/routes/pay/+server.ts` | R | comment naming the owed route |
| — | the six marketing links | R | D11 |

Files with zero hits that the spec named as "untouched" are confirmed:
`ROADMAP.md`, `docs/test-plan.md`, `docs/PARALLEL-SPACE.md`,
`agent-rules/AI-CODING-RULES.md`, `LAUNCH_AUDIT.md`, `SECURITY-AUDIT.md`,
`app-web/vela-wallet/{CLAUDE,README}.md`, `app-web/trusted-signer/HANDOVER.md`.

## 4. Owed table (recorded, not built)

Copied from spec Part B; final state goes into `results.md` with each row
marked *still owed* (expected for all, per the founder's order).

## 5. Residue-check result (shape)

See [contracts/expo-residue-check.md](contracts/expo-residue-check.md).
