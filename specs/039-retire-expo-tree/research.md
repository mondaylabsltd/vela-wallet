# Research: Retiring the Expo tree

Every decision below was checked against the tree on 2026-09-11 (branch
`039-retire-expo-tree` @ 936f1b3f). "Verified" means a command was run, not
a file was read.

## D1 — Delete first; the Pages deployment freezes

**Decision**: the deletion does not wait on the hostname move. After it
merges, the Cloudflare Pages project (git-connected to `main`, build command
`npm run build:web`) cannot build; its last successful deployment keeps
serving `wallet.getvela.app`. The README and `05-deployment-runbook.md` say
so and name the frozen commit; the PR asks the founder to pause the Pages
project's automatic deployments.

**Rationale**: founder ruling (spec Input: *"我现在就想先删除掉…线上先不切换。或者也可以切换"*).
Verified: `curl -sL https://wallet.getvela.app/en/wallet` returns
`/_expo/static/...` assets today; `app-web/vela-wallet/wrangler.jsonc` has no
`routes`/custom domain, so the Worker's binding to the hostname (if any) is
outside the repo.

**Alternatives**: cutover first (the spec's original order) — overruled;
delete and also move the hostname in this feature — offered as a checklist
(spec Part B), not scheduled.

## D2 — The root becomes a tooling package

**Decision**: `package.json` keeps exactly the scripts the CI `app` job and
the developer docs need and exactly the dependencies those scripts resolve:

| Keep | Why (verified consumer) |
| --- | --- |
| `i18next` | `scripts/dump-vectors/i18n.dump.mjs`, `scripts/verify-i18n-parity.mjs` (`createRequire` from the root `package.json`) |
| `identicons-esm` | `scripts/dump-vectors/identicon.dump.mjs`, `scripts/gen-identicon-features.mjs`, `scripts/verify-identicon-parity.mjs` |
| `@noble/curves`, `@noble/hashes` | `scripts/onchain/multikey-safe-gnosis/src/{core,regress,userop}.ts` — that package declares no dependencies and runs with `bun` resolving from the root |
| `typescript` | `packages/vela-sdk` builds with `tsc`; CI runs `npm run build --prefix packages/vela-sdk` with no install step for that package, so `tsc` resolves from the root |

Everything else is dropped: all `expo*`, `react*`, `@react-*`,
`@expo-google-fonts/*`, `lucide-react-native`, `@bacons/apple-targets`,
`eslint-config-expo`, `eslint`, `jest*`, `ts-jest`, `@playwright/test`,
`@types/*`, `esbuild` (only `packages/safari-extension/build.mjs` — deleted),
`ws` (only `e2e/support/relay.js` — deleted), `js-sha3`, `jpeg-js`, `jsqr`,
`qrcode`, `xlsx`, `@undecaf/zbar-wasm`, `buffer`, `base-64`,
`dom-to-image-more`, `@noble/ciphers`. Verified by `git grep` of each
package name outside the deleted paths (the hits that remain are Cargo
crates of the same name, lockfiles of sub-packages, and one design HTML —
none resolve from the root).

Scripts kept: `gen:i18n`, `lint:i18n`, `verify:i18n`, `verify:identicon`,
`dump:vectors`, `gen:identicon-features`, `gen:passkey-providers`,
`lint:passkey-providers`, `lint:lottie`, `lint:lottie:self-test`,
`build:wasm`, `gen:onboarding-types`, `gen:core-types`, `test:core`,
`verify:wasm`, `bench:core`, plus the new `check:expo-residue`. Dropped:
`start`, `reset-project`, `android`, `ios`, `web`, `build:web`, `typecheck`,
`lint`, `test:unit`, `test`, `test:watch`, `test:e2e`, `test:e2e:headed`,
`test:live`.

The lockfile is regenerated with `npm install` (not edited), in the same
commit as the deletion, and `npm ci` from a fresh clone is part of the
final verification.

**Alternatives**: keep `package.json` as is minus Expo — leaves ~30 dead
packages and a `test` script that runs nothing; a separate `tooling/`
package — moves every script path the docs and CI name, for no gain.

## D3 — Root `tsconfig.json` and `eslint.config.js` go

**Decision**: delete both. After the cut there is no TypeScript at the root
for `tsc` to find (`scripts/onchain` runs under `bun`; `packages/*` and
`app-web/*` have their own configs, which the root config already
excluded); `eslint.config.js` exists only to lint `src/` with the Expo
preset.

**Alternatives**: keep a minimal root tsconfig — it would check nothing and
mislead the next reader.

## D4 — One mirror per wire-type family

**Decision**: `rust/scripts/gen-core-types.mjs` drops the `src/services/…`
entries from `onboarding` and `wallet-state` `outDirs`; each family writes
to its `app-web/vela-wallet/src/lib/{onboarding,session,core}/generated`
directory only. The two bins (`generate_onboarding_bindings.rs:23`,
`generate_wallet_state_bindings.rs:75`) default to those same directories
when called without an argument. The `onboarding` barrel header string is
kept byte-identical (016 FR-017 — the committed `app-web` copy carries it).
`rust/scripts/gen-onboarding-types.mjs` (the shim) stays. The CI step in the
`rust` job is renamed to name the `app-web` path.

**Verified**: `app-web/vela-wallet`'s `check` script already runs
`gen-core-types.mjs wallet-state session --check`; after the change,
regenerating must produce a zero diff under `app-web` (it is the same
generator writing the same bytes to one fewer place).

**Alternatives**: keep writing to a `src/` path that no longer exists —
`--check` would fail on every run.

## D5 — `gen-i18n` loses Stage 3; `public/` stays

**Decision**: remove `RESOURCES_FILE` (line 40), the Stage 3 emitter
(`:451–~520`) and its log line (`:656`); rewrite the header stage list, the
`ASSET_DIR` comment (`:42–44`, currently *"Expo copies `public/*` to the
export root…"*) and the two `resources.ts` mentions (`:48`, `:383`) to name
the consumers that remain. `public/i18n/` and `public/vela_core_bg.*.wasm`
stay at their paths.

**Rationale**: `public/i18n` is read at that path by `app-ios`
(`project.pbxproj:272`, `bundle-catalogs.sh`, `catalogs-input.xcfilelist`),
`app-android` (`build.gradle.kts:81,127-128`, six fixture tests),
`app-web/vela-wallet` (`engine.server.ts:29`), `scripts/verify-i18n-parity.mjs:87`,
`rust/scripts/verify-web.mjs:55` and the CI drift gate. Moving it is ~20
edits across four toolchains for a name. The wasm is written by
`rust/scripts/build-web.mjs:35` and read by `app-web/.../sync-wasm.mjs:44`,
`rust/scripts/load-wasm-node.mjs:21`, `scripts/onchain/.../core.ts:29`.

**Alternatives**: rename `public/` to `generated/` — rejected for the reason
above; keep Stage 3 writing to a path that does not exist — the script
would throw.

## D6 — CI's `app` job keeps its name

**Decision**: the job stays `app`; its `Typecheck`, `Lint` and `Unit tests`
steps are removed; the i18n drift gate's `git diff` path list loses
`src/i18n/resources.ts`; the header comment (`ci.yml:5-13`, *"The Expo
sources under src/ are still typechecked, linted and tested here — they are
still the shipping native app"*) and the "Deliberately NOT included" block
(`:22-24`, Playwright + Metro) are rewritten; a `Check for Expo residue`
step is added after the i18n gates. The `rust` job's "Onboarding wire types
are current (src/services/onboarding-core/generated)" step is renamed.

**Rationale**: branch protection requires a check named `app` (the header
says so: *"branch protection requires app+site 两 job 绿才能 merge"* in the
runbook); renaming the job is a dashboard action the founder would have to
take before any PR could merge. The job's *content* is what changes.

**Alternatives**: rename to `tooling` — blocked on the dashboard; split into
per-gate jobs — more runners for no coverage gain.

## D7 — Two packages: the defaults

**Decision**: delete `targets/safari/` and `packages/safari-extension/`;
keep `packages/vela-sdk/` and its CI build step.

**Rationale**: `targets/safari` is an `@bacons/apple-targets` Expo target;
`app-ios/VelaWallet/VelaWallet.xcodeproj` has no Safari-extension target
(verified: targets are `VelaWallet`, `VelaWalletTests`, `VelaWalletUITests`);
`packages/safari-extension/build.mjs` writes into `targets/safari/assets/`
and nothing else consumes it (`app-web/vela-wallet/extension/` has its own
`build.mjs`). A source directory with no build is Expo-shaped residue.
`docs/safari-extension/ARCHITECTURE.md` stays as the design record with a
history banner; the eight runbooks are banner'd too (they are the
`create-target` procedure). `packages/vela-sdk` has no Expo dependency
(`package.json`: devDependency `typescript` only) and is the contract a
dApp integrates; its wallet half is owed (spec Part B).

**Alternatives**: keep `packages/safari-extension` as source — rejected
(see spec "Two packages"); retire the SDK — the founder did not ask for it,
and it costs nothing to keep.

## D8 — Documents: rewrite / history / delete

**Decision**: every tracked file outside the deleted tree with an Expo /
React Native / Metro / EAS / Hermes mention gets one of three
dispositions, recorded per file in data-model.md:

- **Rewrite** — the file describes the product as it is after the cut. For
  the takeover interview documents (`10`, `11`, `12`, `13`, `07`, `08`),
  the precedent from PR #169 applies: a **dated erratum block at the top**
  naming what changed, rather than line-by-line edits of a question bank —
  the answer key alone has 33 mentions, and the questions are still valid
  history of what the takeover candidate was asked.
- **History** — the file opens with a banner:
  `> **History (2026-09-11).** This document described the Expo / React
  Native app, retired in spec 039. It is kept as the design record. The
  living description is <link>.` The body is left as it was.
- **Delete** — the file exists only to operate something deleted.

The README's `:227` sentence and the runbook's web-wallet row are rewritten
to the truth of the day (D1). Code comments (`app-web/**`, `rust/**`,
`app-desktop/**`) that cite the Expo tree as provenance (*"ported from…"*,
*"the Expo executors were written against…"*) are past-tense rationale and
stay; a comment that explains a constraint that no longer exists (e.g.
`gen-i18n.mjs:42` *"Expo copies public/* to the export root"*) is rewritten.

**Alternatives**: grep-and-replace the word — rejected by the spec ("a doc
scrubbed of the word but not the meaning"); delete the takeover docs —
rejected, they are the handover authority.

## D9 — The residue check is a script

**Decision**: `scripts/check-expo-residue.mjs`, run by `npm run
check:expo-residue` and by the CI `app` job. Rules in
[contracts/expo-residue-check.md](contracts/expo-residue-check.md). In one
line: hard-fail on deleted paths reappearing, on Expo/RN dependencies in
any `package.json`, on Expo terms in CI steps or `package.json` scripts, and
on **dead commands** (`npx expo …`, `expo start|export|run|prebuild|lint`,
`eas build`, `npm run web|build:web|test:e2e`, `metro`) anywhere in any
tracked file; report (not fail) prose mentions per file so the count is
visible.

**Rationale**: the spec's clean-tree test 3 has to be runnable by the next
person. A regex over prose cannot tell history from a stale instruction; a
regex over *commands* can, and a doc that tells you to run a command that
no longer exists is exactly the failure the spec names.

**Alternatives**: a plain `grep -ri expo` in CI — fails on every history
banner forever; an allowlist of files — rots.

## D10 — Assets and public files

**Decision**: delete `assets/expo.icon/`,
`assets/images/{splash-icon,android-icon-background,android-icon-foreground,android-icon-monochrome,expo-badge,expo-badge-white,expo-logo,react-logo,react-logo@2x,react-logo@3x,logo-glow,tutorial-web}.png`,
`assets/images/tabIcons/`, `assets/fonts/Inter-*.ttf`,
`assets/templates/payroll-template.csv`, `public/__vela-opener.html`,
`public/qr-test.html`, `public/test-qr.jpg`, `public/og-image.png`,
`public/zbar.wasm`, **and** `assets/images/icon.png` + `favicon.png` (see the
correction below). Keep `assets/fonts/PlusJakartaSans_*.ttf`,
`assets/fonts/README.md` (Inter row removed), `public/i18n/`,
`public/vela_core_bg.*.wasm`.

**Verified — with one correction to the spec's first draft**: the inventory
had `assets/images/icon.png` and `favicon.png` down as the icon script's
*source art*. Reading `gen-app-icons.sh:106-118` shows the opposite: the
script renders them FROM `design/icon/*.svg` for `app.json`'s `icon` /
`favicon` / adaptive-icon fields, and `git grep` finds no other reader. So
the whole "Expo app" section of the script goes (`render_inset`, `solid`,
`android_fg_scale`, the `$images` outputs, the Android safe-zone guard that
only ever measured the Expo foreground layer), the alpha guard is re-pointed
at `app-ios`'s `icon-1024.png`, and `assets/images/` is deleted entirely.
`app-desktop/theme.rs:437-440`
`include_bytes!` the four PJS TTFs. `app-web/.../qr-decode.ts:20` — *"One
deliberate divergence from Expo: it loads zbar from a CDN"* — so
`public/zbar.wasm` has no reader after `src/components/QRScanner.tsx` goes.
`assets/fonts/README.md` already records Inter as *"legacy; not loaded by
any shell since 038"*.

## D11 — Our own links

**Decision**: the six marketing-site links to `wallet.getvela.app/onboarding`
and `/onboarding?mode=create` (`+page.svelte:299,321,329,932`,
`SiteHeader.svelte:45`, `docs/install.md:12`, `docs/create-wallet.md:13`)
become `https://wallet.getvela.app/` — the Worker's `/` negotiates the
locale and the Expo build's `/` is its front door too, so the link is right
on both the frozen build and the future one. The marketing site's `/pay`
redirect (`pay/+server.ts:12`) stays with a comment naming the owed route;
the SDK's default `/web-request` stays with a note in its README; app-web's
`PAY_LINK_FALLBACK` (`eip681.ts:200`) stays — it is the shell's own future
route.

**Rationale**: FR-398 — our links must not name an Expo-only path *or* must
say the path is owed. No redirect shim is written (founder ruling).

**Alternatives**: deep-link create to `/en/create` — wrong for every
non-English visitor; leave the links — they 404 the day the hostname moves.

## D12 — Verification matrix

| Gate | Local (this machine) | CI |
| --- | --- | --- |
| `app-web/vela-wallet` check / lint / unit / build | yes | `web` job (build) |
| `app-desktop` fmt / clippy / test | yes | `desktop` job |
| `rust/` workspace tests + wasm canary + web artifact + Kotlin/Swift corpus | tests + `build-web.mjs --check` locally | `rust`, `rust-macos` |
| `app-android` compile + unit test | if `rust/bindings/kotlin` exists on disk | `android` job |
| `app-ios` build + test | if `app-ios/VelaCoreKit/Artifacts` exists on disk | `ios` job |
| root tooling gates (`dump:vectors` diff, identicon table, i18n artefacts, corpus lint, Lottie, reachability, two parity gates) | yes, all | `app` job |
| residue check | yes | `app` job |
| drift proofs (SC-393) | yes, recorded, reverted | not in CI (deliberate one-line breakage) |

The founder's constraint is met when the first five rows are green on the
branch tip; the PR is not offered before the `android` and `ios` jobs have
run on it.

## D13 — Stray files and residue on disk

**Decision**: `git rm` `.verify-rescan.mjs` and `.verify-rescan-ja.mjs`
(one-off Playwright probes against `localhost:8081`, committed in 31a350ba);
`rm -rf dist/ test-results/` (untracked, 38 MB, Expo output); `.gitignore`
loses `.expo/`, `dist/`, `web-build/`, `expo-env.d.ts`, `.metro-health-check*`,
`/ios`, `/android`, `e2e/screenshots/`, `test-results/` (nothing else writes
those; `app-desktop/vela-wallet/dist/` is covered by its own ignore rules —
verified the root rule was not what ignores it, since it is listed as
`!!` under its own path).

## D14 — What the README and runbook say about production

**Decision**: README "Build for Web" replaces the `:227` sentence with:
*"`wallet.getvela.app` is, as of 2026-09-11, still served by the last
Cloudflare Pages deployment of the retired Expo build (commit `<sha>`); the
Worker `vela-wallet-web` is the live build. The move is the checklist in
`specs/039-retire-expo-tree/spec.md` §"The move itself"."*
`05-deployment-runbook.md`'s web-wallet row says the same and its rollback
section names the Pages last-deployment fallback. Both carry the date so a
later reader knows when it was last true.
