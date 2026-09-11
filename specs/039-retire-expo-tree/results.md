# Results: Retiring the Expo tree

Written as the work lands (per 033–038). Branch `039-retire-expo-tree`,
off `main` @ 936f1b3f.

## Baseline @ 936f1b3f — 2026-09-11

Every gate run on the branch tip *before* the first deletion commit, so a
later difference can be attributed.

| Gate | Result | Note |
| --- | --- | --- |
| app-web `pnpm check` | green | tokens drift, wasm sync, generated types, svelte-check: 0 errors |
| app-web `pnpm lint` | **red — pre-existing** | prettier: `src/lib/flows/FlowsMobile.svelte`, `src/lib/services/endpoints.ts` unformatted on `main`; eslint not reached. Not this feature's; left as found |
| app-web `pnpm test:unit` | 941 passed / **2 failed — pre-existing** | `src/lib/signing/fixtures.test.ts:48` (catalogue 35 ≠ 33) and one sibling, both carried by `main` since 038 (its results.md names the same two) |
| app-web `pnpm build` | green | prerendered all locales, built in 7.75 s |
| rust `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | green | every `test result: ok`, no FAILED |
| desktop `cargo fmt --check` | green | |
| desktop `cargo clippy --all-targets` | green with warnings | 40 warnings, the same families the CI header records as the known baseline; not `-D warnings` by design |
| desktop `cargo test` | green | 395 passed, 37 ignored |
| root tooling gates (`build-web --check`, `verify-web`, `dump:vectors` + diff, identicon table + diff, `gen-i18n` + diff, corpus lint, Lottie self-test + lint, native reachability, identicon parity, i18n parity, `gen-core-types --check`, `vela-sdk` build) | green | no error output from any of the thirteen; `git status` clean after the three regenerate-and-diff gates. Exit codes were not captured by the runner (zsh `pipestatus`), so each is re-run with its code after the cut |
| android `./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild` | green | BUILD SUCCESSFUL in 57 s (host dylib + `rust/bindings/kotlin` already on disk) |
| ios `xcodebuild build -scheme VelaWallet -destination 'generic/platform=iOS Simulator'` | green | exit 0, no errors (xcframework already on disk) |

## After the cut — 2026-09-11

The working tree with `src/`, `e2e/`, `modules/`, `plugins/`, `targets/`,
`packages/safari-extension/`, `.eas/`, the root toolchain files, the
Expo-only assets and public files gone (1,097 tracked files), the root
`package.json` reduced to the tooling package (5 dependencies, `npm ci`
installs 7 packages), and the generators re-pointed (`gen-core-types.mjs`
one mirror per family, `gen-i18n.mjs` without Stage 3, `gen-app-icons.sh`
without its Expo section).

| Gate | Result | Compared with baseline |
| --- | --- | --- |
| app-web `pnpm check` | green (1386 files, 0 errors) | same |
| app-web `pnpm lint` | red — the same two prettier files | same |
| app-web `pnpm test:unit` | 941 passed / 2 failed — `explore/fixtures.test.ts` (7 phone states), `signing/fixtures.test.ts` (33 scenarios) | same two |
| app-web `pnpm build` | green, 7.73 s | same |
| desktop fmt / clippy / test | green / green (warnings) / 395 passed, 37 ignored | same |
| rust workspace tests | green, every `test result: ok` | same |
| android `testDebugUnitTest` | BUILD SUCCESSFUL | same |
| ios simulator build | exit 0 | same |
| `gen-core-types --check` | exit 0 | (was 2 mirrors, now 1) |
| `gen-i18n` + artefacts diff | exit 0 / 0 | same |
| `dump:vectors` + diff | exit 0 / 0 | same |
| `gen-identicon-features` + diff | exit 0 / 0 | same |
| `lint-i18n-corpus`, Lottie self-test + lint, native reachability | exit 0 ×4 | same |
| identicon parity, i18n parity | exit 0 / 0 | same |
| `vela-sdk` build | exit 0 (root `typescript`) | same |
| `verify-web` | exit 0 | same |
| `build-web --check` | **red until the wasm fingerprint is refreshed** — the check hashes every Rust file and the two generator bins were edited; `npm run build:wasm` is run and its fingerprint committed with the generator change | expected |
| residue self-test | ok (15 expectations) | new |

Two things found on the way, neither caused by the cut:

- `scripts/gen-app-icons.sh` fails on this machine at its Android section
  (`declare -A` needs bash ≥ 4; macOS ships 3.2) and, when the iOS section
  runs, re-renders the three `AppIcon.appiconset` PNGs with different bytes
  (ImageMagick version). The three PNGs were reverted, not committed. The
  script's Expo section is removed regardless; its remaining sections were
  not touched.
- `npm run lint:passkey-providers` reports the catalog stale. It is not a
  CI step and reads nothing under `src/`; left as found.

**One incident to record**: at 20:09:51 the root `package.json`,
`package-lock.json` and `node_modules/` disappeared from the working tree
between two of this session's commands. No `npm` invocation is logged at
that time, the only Claude process with this cwd is this session, and
nothing else at the root was touched. They were recreated (same content,
`npm install` → `npm ci` green) and the gates re-run. Cause unknown; noted
so a repeat is recognised.

## Generators — 2026-09-11

`gen-core-types.mjs` writes one mirror per family (all three under
`app-web/vela-wallet/src/lib/*/generated`); `--check` exit 0. The two bins
default to the same directories. `gen-i18n.mjs` without Stage 3: exit 0, zero
diff under `rust/crates/vela-core/src/{i18n,i18n_catalogs,l10n}` and
`public/i18n`. `gen-app-icons.sh` without its Expo section: the iOS/Android/
site sections are untouched (see the bash-3.2 note above). `build-web.mjs`
rebuilt the artifact so `--check` is green again: the asset name moved from
`d19da7455477` to `672f6881b303` (the fingerprint covers every Rust file, the
bins included); the wasm is the same 3,734,673 bytes; `sync-wasm --check` in
app-web green.

Commits: `972446ba` (generators), `2d848947` (the cut), `5861cb7b` (CI).

## The gates still bite (SC-393)

Deliberately deferred until the drift proofs can be run against the pushed
branch alongside CI; recorded below once run.

## Residue — before the doc pass

First real run of `scripts/check-expo-residue.mjs` (after the cut, before any
document moved): rules 1–2 ok; rule 3 flagged its own CI step and npm script
(the checker's name contains the term — now the one allowed self-reference);
rule 4 flagged 11 lines, of which three were the checker being too broad
(`npm run test:e2e` is alive in both web apps; "Metro's double-bundle" is
prose) — the regex was narrowed to root-only dead commands — and the rest
were real: the README and runbook naming `npm run build:web` as a dead
command, `docs/test-plan.md`'s root Playwright commands, an error string in
`build-web.mjs`, and my own BUG-4 note. Each was rewritten, not exempted.

## Residue — after (exit 0)

```
expo-residue self-test: ok (18 expectations)
expo-residue: rule 1 (deleted paths)       ok
expo-residue: rule 2 (dependencies)        ok
expo-residue: rule 3 (executable config)   ok
expo-residue: rule 4 (dead commands)       ok
expo-residue: prose mentions (report only) 165 files, 431 lines
```

The 431 prose lines are history banners, dated errata, past-tense provenance
comments ("ported from the Expo executors"), and the answer key's questions;
the top of the list is `11-interview-answer-key.md` (34), the Safari runbooks
(18, 13, …), `KNOWN-BUGS.md` (13). None is a command or a path a reader could
follow into nothing.

## Docs — disposition applied

- **Rewritten**: `README.md`; `docs/project-takeover/{01,02,05}`; the single
  lines in `NATIVE-LAUNCH-CHECKLIST.md`, `store-submission/privacy-and-review.md`,
  `fiat-price.md`, `marketing/100-marketing-leads.md`, `KNOWN-BUGS.md` (BUG-4),
  `app-desktop/vela-wallet/README.md`, `agent-rules/CLAUDE-AUTO-TEST.md`,
  `rust/README.md`, `packages/vela-sdk/README.md`; comments in
  `rust/scripts/build-web.mjs`, `rust/crates/vela-core-wasm/{Cargo.toml,src/lib.rs}`,
  `app-web/vela-wallet/{tsconfig.json,scripts/sync-wasm.mjs}`,
  `app-web/getvela.app/src/routes/pay/+server.ts`.
- **History banner**: `docs/safari-extension/*.md` (9), `docs/dapp-browser/ARCHITECTURE.md`,
  `docs/qr-scanner-web.md`, `docs/dynamic-amount-display.md`,
  `docs/text-scale-architecture.md`, `docs/https-web-wallet.md` (+ source
  map re-pointed), `docs/localization.md`, `DESIGN_SYSTEM.md`.
- **Dated erratum** (PR #169 precedent): `docs/project-takeover/{03,04,06,07,08,09,10,11,12,13,14}`,
  `docs/CONTENT-SOURCE-100-CLUES.md`, `docs/MANUAL-TEST-100-CLUES.md`,
  `docs/TEST-OUTLINE.md`, `docs/test-plan.md`, `docs/requirements/{A04,E06,M02,M04}`.
- **Deleted**: `docs/PARALLEL-SPACE-E2E-PLAYBOOK.md` (its one inbound link was
  in `02`, which is rewritten).
- **Left as they are** (past-tense provenance): the `app-web/**` and
  `app-desktop/**` code comments, `rust/crates/vela-core/Cargo.toml` and
  `vela-core-uniffi/Cargo.toml` (both already describe the Expo rule as the
  old rule), `rust/crates/vela-core/src/i18n/mod.rs` (Hermes as the reason a
  degraded plural path exists), `docs/requirements/M05`.
- **Our own links**: six marketing-site hrefs → `https://wallet.getvela.app/`;
  `bun run check` in `app-web/getvela.app`: 1749 files, 0 errors.

Commits: `6da6c41d`, `1864a536`, `23543d79`.

## Clean tree (SC-394)

- Test 1 — root listing: `.github .gitignore .mcp.json .specify DESIGN_SYSTEM.md
  LICENSE README.md ROADMAP.md WHITEPAPER.md agent-rules app-android
  app-browser-extension app-desktop app-ios app-web assets design
  design-system.md docs node_modules package-lock.json package.json packages
  public rust scripts specs` (+ the machine's `.DS_Store`, `.VSCodeCounter`,
  `.claude`, all ignored). Matches plan.md §Source Code.
- Test 2 — `git status --ignored` lists no `dist/`, `test-results/`, `.expo/`,
  `web-build/`, `e2e/`; `.gitignore` names none of them.
- Test 3 — the committed script, exit 0 (above).
- `git ls-files`: 3,209 tracked files (was 4,291); 0 under the deleted paths.

## Register — final state

Every row as in spec Part B, with the two *verify* rows resolved:

- **EIP-7708 sentinel-address feed path** → **present**: the rule lives in
  the core — `rust/crates/vela-core/src/app/token_trust.rs:80` (*"Contract-
  address sentinels marking a log as a native (EIP-7708) transfer"*) — and
  both shells read it from there (`recipient-identity.ts:193`,
  `executor/identity.rs:254` treat the zero address as the mint/burn
  counterparty). Not owed.
- **`[locale]/import` placeholder** → nothing links to it (`git grep` finds
  no `/import` href in `app-web/vela-wallet/src`); reachable only by typing
  the URL. Owed, trivial: route it to the login or delete the page.

Rows marked *owed* are unchanged: gas-account funding sheet; receive request
mode + `/pay`; in-app bug-report submit; the HTTPS SDK wallet half. Rows
marked *dropped* carry their rulings (WalletPair / remote-inject: spec 027;
analytics: default, unconfirmed; dev harnesses: Assumptions).

## Owed — final state (all *still owed*, by the founder's order)

| Owed | Status |
| --- | --- |
| Gas-account funding sheet | still owed (money path; first on the list before the hostname moves) |
| Receive request mode + `/pay` page | still owed |
| In-app bug-report submit | still owed |
| HTTPS SDK wallet half (`/web-request`) | still owed; package + CI step kept |
| `[locale]/import` placeholder | still owed (trivial) |
| Our own links | **done** (this feature) |
| EIP-7708 feed path | **not owed** — present in the core |

## Rulings, with dates

- 2026-09-11 (founder, spec Input): delete first; the hostname may stay on
  the frozen Pages build or move — either way the deletion does not wait.
- 2026-09-11 (founder, spec Input): no compatibility with the Expo web build
  — client cache and data may be lost; no migration, no notice; no redirect
  shims for Expo-only paths (our own links are updated regardless).
- 2026-09-04 (founder, spec 027 Input): WalletPair and the remote-inject
  bridge are not supported on the web ("不成熟").
- Defaults applied, reversible in one line: `targets/safari` +
  `packages/safari-extension` deleted (re-home under `app-ios` is its own
  spec); `packages/vela-sdk` kept with its CI step; the third-party
  analytics script not carried over.

## The gates still bite (SC-393) — run 2026-09-11, both reverted

- **i18n**: one corpus leaf (`send.send.selectTokenTitle` in
  `rust/crates/vela-core/i18n/locales/en/send.json`, `"…"` → `"…!"`),
  `node scripts/gen-i18n.mjs` (exit 0), then the gate's diff:
  `public/i18n/en.json | 2 +-`, `rust/…/i18n_catalogs/en.rs | 44 ++++----` →
  **exit 1**. Reverted; `git status` clean. (A first attempt that wrote a
  malformed top-level key made `gen-i18n` itself exit 1 — the generator's
  structural assertions are a second layer of the same gate.)
- **identicon**: one colour literal in the pinned oracle
  (`node_modules/identicons-esm/dist/core-*.mjs`, `#010101` → `#010100`),
  then `npm run dump:vectors` and the gate's diff:
  `tests/vectors/identicon.json | 8 ++++----` → **exit 1**; and
  `verify-identicon-parity.mjs` → **exit 1** (`legacy: … fill="#010100"` vs
  `core: … fill="#010101"`). Oracle restored from backup, vectors regenerated
  and diffed clean, parity green again; `git status` clean.

## Fresh clone (SC-395) — 2026-09-11

`git clone --branch 039-retire-expo-tree` into a scratch directory, then
`docs/project-takeover/02-local-development.md` top to bottom: `npm ci`
(7 packages, 0.4 s); every root gate exit 0 (gen-i18n + diff, dump:vectors +
diff, identicon table + diff, corpus lint, Lottie, reachability, both parity
gates, verify-web, the residue check, the SDK build); `gen-core-types
--check` — 11 + 326 types current in one mirror each; app-web `pnpm install
--frozen-lockfile` + `pnpm build` (6.6 s) green; desktop `cargo check` green.
No command in the document was "not found".

One sentence in the document misled and was fixed on the spot: on a fresh
clone `pnpm check` fails BEFORE the first `pnpm dev`/`pnpm build`, because
the `static/` wasm copy is gitignored and `sync-wasm --check` only verifies
it; after the build it passes (1386 files, 0 errors). `02` now says to run
`pnpm sync:wasm` (or dev/build) first. Not a regression — the same is true
on `main` — but the doc walk is what surfaced it.

## PR and CI

Draft PR **#193** — https://github.com/mondaylabsltd/vela-wallet/pull/193
(the workflow triggers on `pull_request`, not on a branch push). CI run
34599194167 on `49bad427`; job results appended below when the run
finishes.

## CI, first run — one red, and why

Run 34599463891 on `472ac163`: the `rust` job's "Web build is current" step
failed — the committed fingerprint (`672f6881b303`) was taken at the
generator commit, and the docs pass afterwards edited a doc comment in
`rust/crates/vela-core-wasm/src/lib.rs` and the `description` line of its
`Cargo.toml`. `sourceFingerprint()` hashes every `.rs` and `Cargo.toml`
under the crate roots, comments included, so a comment-only edit is a new
fingerprint. Rebuilt (`npm run build:wasm`, asset `69fea6785243`, same
3,734,673 bytes), `--check` exit 0, app-web `sync-wasm --check` exit 0,
committed. Lesson for the memory notes: any edit under `rust/`, prose or
not, means a rebuild before the push.
