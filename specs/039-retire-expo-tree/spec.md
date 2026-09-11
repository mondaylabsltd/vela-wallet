# Feature Specification: Retiring the Expo tree — delete first, and leave a repository that tells the truth

**Feature Branch**: `039-retire-expo-tree` (off `main` @ 936f1b3f, after PR #192)

**Created**: 2026-09-11

**Status**: Draft

**Input**: Founder:

> 使用 github speckit 创建 039 来实现一个任务，就是我现在要删除 react native
> expo 相关的代码，并且确保我们的 app-web/vela-wallet 已经实现了，能替代 expo
> web vela wallet 了。

> 要保证我们的目录干净，文档准确，expo react native 相关代码删掉。

> 不需要去兼容 expo web 吧，客户端缓存和数据可以丢失。

> 我现在就想先删除掉明白吗？线上先不切换。或者也可以切换。

## Why

The Expo tree is 961 tracked files and ~120,000 lines that have not been
able to run on a phone since PR #168 (2026-08-13): the one-implementation cut
made the core facade pure wasm, and Hermes has no WebAssembly. What is left
is a **web-only build carried by a native toolchain** — `expo-router`, Metro,
EAS profiles, four Expo native modules, two config plugins, an Apple target,
a Jest suite and a Playwright suite that boots Metro. CI still typechecks,
lints and unit-tests all of it on every PR, and the type generator still
writes a second copy of every wire type into it so that the copy the
SvelteKit shell uses cannot drift from a copy nothing ships.

It is also, today, what serves the production hostname. The README says the
Expo bundle *"is no longer the production web build"* (`README.md:227`). A
live probe on the day this spec was written says otherwise:

```
$ curl -sL https://wallet.getvela.app/en/wallet | grep -oE '/_expo/[^"]+'
/_expo/static/css/global-2d2f804f163f1b3768b986626b1ecb5d.css
/_expo/static/js/web/index-f11a766e83d41f9dfa4b1b25e20c6dab.js
```

`wallet.getvela.app` is served by the Cloudflare Pages build of `npm run
build:web`, i.e. by the tree this feature deletes. The SvelteKit shell's
Worker (`vela-wallet-web`) has no route to that hostname in the repository
(`wrangler.jsonc` carries `workers_dev: true` and no custom domain).

**The founder's ruling sets the order: delete first.** The hostname may stay
where it is for now, or move; either way the deletion does not wait on it,
and the Expo build's client-side data owes nothing to the new shell — a
person's local cache and records may be lost. So this feature is two things:

1. **Delete, now, and leave the repository true.** Not just `src/`: the
   toolchain, the suites, the config, the assets and public files only the
   Expo build read, the second copies of generated types, the CI steps, the
   dependencies — and every sentence in the docs that still describes the
   Expo app as the system. A repository where `README.md` explains an
   architecture that is gone is not clean, however empty `src/` is.
2. **Record what the SvelteKit shell still owes before the hostname moves,
   and make the move a checklist.** The parity sweep says the shell has
   nearly all of it; the handful it lacks is written down with sizes, so the
   day the founder decides to switch — inside this feature or after it — the
   list is already there.

**What "delete first" costs, said plainly**: once the deletion merges to
`main`, the Pages project can no longer build it (`npm run build:web` is
gone). Its **last successful deployment keeps serving `wallet.getvela.app`,
frozen at the last Expo commit**, until the hostname is pointed at the Worker.
Nothing breaks for a visitor; nothing updates for one either. The runbook
and the README say exactly that, and the founder is asked to pause the Pages
project's automatic deployments in the dashboard so each later merge does
not raise a failed build.

**Standing rules this feature inherits, not re-decides**: the core decides
and the shell performs; `docs/project-takeover/` is the handover authority
and is rewritten, never deleted; budgets do not move; a corpus edit and its
regenerated artefacts land in one commit.

## Part A — the deletion

### What goes

Everything whose only reader was the Expo build. The inventory was taken
file by file on 2026-09-11 (`research.md` carries it when planned); its
shape:

- **The tree**: `src/` (961 files), `e2e/` (39 — every spec drives the Expo
  web build; `playwright.config.ts` boots `npx expo start --web`), `modules/`
  (16 — the four Expo native modules; nothing under `app-android/` or
  `app-ios/` references them), `plugins/` (2 config plugins that run only in
  `expo prebuild`), `targets/safari/` and `packages/safari-extension/` (see
  "Two packages" below).
- **The toolchain at the root**: `app.json`, `app.config.js`, `eas.json`,
  `.eas/workflows/create-production-builds.yml`, `metro.config.js`,
  `index.js`, `index.web.js`, `expo-env.d.ts`, `fingerprint.config.js`,
  `.fingerprintignore`, `jest.config.js`, `jest.setup.js`,
  `scripts/jest-skipped-reporter.js`, `playwright.config.ts`,
  `keystore.properties.example`, `scripts/reset-project.js`,
  `scripts/fix-cf-pages-assets.js`, and the two stray root scripts
  `.verify-rescan.mjs` / `.verify-rescan-ja.mjs` (one-off Playwright probes
  against `localhost:8081`, committed by accident in 31a350ba).
- **Dependencies**: every `expo*`, `react-native*`, `@expo-google-fonts/*`,
  `@react-navigation/*`, `@react-native-async-storage/*`,
  `lucide-react-native`, `eslint-config-expo`, `@bacons/apple-targets`, and
  the root Jest/Playwright/ts-jest set — from `package.json` and the
  lockfile. The root `tsconfig.json` (`extends: expo/tsconfig.base`, `@/*` →
  `src/`) and `eslint.config.js` (`eslint-config-expo/flat`) go with them or
  are rewritten to cover only what remains.
- **Assets and public files only the Expo build read**: `assets/expo.icon/`,
  `assets/images/{splash-icon,android-icon-*,expo-*,react-logo*,logo-glow,tutorial-web}.png`,
  `assets/images/tabIcons/`, `assets/fonts/Inter-*.ttf` (the fonts README
  already says *"legacy; not loaded by any shell since 038"*),
  `assets/templates/payroll-template.csv` (no consumer outside `src/`),
  `public/__vela-opener.html` (design-capture scaffolding with no consumer),
  `public/qr-test.html`, `public/test-qr.jpg`, `public/og-image.png` (the
  SvelteKit shell generates its own), `public/zbar.wasm` (the SvelteKit
  scanner resolves zbar from its own dependency — verify, then delete).
- **The second copies**: the type generator's `src/services/*/generated`
  mirrors (`rust/scripts/gen-core-types.mjs:45,70`) and the two bins'
  default output paths; `scripts/gen-i18n.mjs`'s Stage 3
  (`src/i18n/resources.ts`).
- **Ignore rules and on-disk residue**: the Expo entries in `.gitignore`
  (`.expo/`, `dist/`, `web-build/`, `expo-env.d.ts`, `/ios`, `/android`,
  `.metro-health-check*`, `e2e/screenshots/`); the untracked `dist/` (21 MB)
  and `test-results/` (17 MB) sitting at the root today.

### What stays, and must keep working — the shared floor

The Expo build was not the only reader of several things at the root. These
are kept and their consumers re-verified by running them, not by reading:

| Path | Kept for |
| --- | --- |
| `public/i18n/*.json` | `app-ios` (Xcode file lists, `bundle-catalogs.sh`, `Loc.swift`), `app-android` (`build.gradle.kts`, fixture tests), `scripts/verify-i18n-parity.mjs`, `app-web/vela-wallet` (`engine.server.ts`), the CI i18n drift gate |
| `public/vela_core_bg.<hash>.wasm` | written by `rust/scripts/build-web.mjs`; read by `app-web/vela-wallet/scripts/sync-wasm.mjs`, `rust/scripts/load-wasm-node.mjs`, `scripts/onchain/` |
| `assets/fonts/PlusJakartaSans_*.ttf` | `app-desktop` (`include_bytes!` in `theme.rs:437-440`) |
| `design/icon/*.svg` | the one vector source `scripts/gen-app-icons.sh` renders app-ios, app-android and getvela.app icons from (correction, 2026-09-11: `assets/images/icon.png` and `favicon.png` were the script's *outputs* for `app.json`, not its inputs — they go too, and `assets/images/` with them) |
| `scripts/*.mjs`, `scripts/dump-vectors/`, `rust/scripts/*` | none import from `src/`; they need root `node_modules` for `i18next` and `identicons-esm` — the npm oracles the corpus gates compare against |
| `packages/safe-recovery-extension/` | standalone; no root import, no Expo dependency |
| `packages/vela-sdk/` | kept — no Expo dependency; its wallet half is owed in Part B |
| `design/`, `docs/design-tokens.json`, `rust/pkg-web/` | untouched |

The root `package.json` becomes the **tooling package**: generators, gates
and oracles only (`gen:i18n`, `lint:i18n`, `verify:i18n`, `verify:identicon`,
`dump:vectors`, `gen:identicon-features`, `gen:passkey-providers`,
`lint:lottie`, `build:wasm`, `gen:core-types`, `test:core`, `verify:wasm`,
`bench:core`), with `i18next` and `identicons-esm` pinned as before. No
`start`, no `web`, no `build:web`, no `test`.

### Two packages — the defaults, applied until the founder overrules

- **`targets/safari/` and `packages/safari-extension/` go.** The target is
  an `@bacons/apple-targets` Expo target and the only packaging the iOS
  Safari extension ever had; `app-ios/VelaWallet.xcodeproj` has no Safari
  extension target; the package's build writes into the target's `assets/`
  and is consumed by nothing else (`app-web/vela-wallet/extension/` has its
  own build). Keeping a source directory with no way to build it is the
  Expo-shaped residue this feature exists to remove. `docs/safari-extension/`
  stays as the design record with a dated banner; re-homing the extension
  under `app-ios` is its own spec. Git history keeps the source.
- **`packages/vela-sdk/` stays**, with its CI build step. It has no Expo
  dependency and is the contract a dApp integrates; what dies with `src/` is
  the wallet half (`src/app/web-request.tsx`, `web-popup-transport.ts`), which
  Part B lists as owed. `docs/https-web-wallet.md` gets a banner saying so
  and its source map is re-pointed.

Either default is reversed by one line from the founder; neither blocks the
deletion.

### Generators and CI after the cut

- `rust/scripts/gen-core-types.mjs` writes each wire-type family to exactly
  one place, under `app-web/vela-wallet`; its `--check` diffs only that
  place; the two bins' default output paths follow. `scripts/gen-i18n.mjs`
  no longer emits a TypeScript resources file, and its `public/`
  justification comment (`:42-44`, written in Expo terms) is rewritten for
  the consumers that remain.
- The `app` job keeps what survives and drops what does not: **keeps** `npm
  ci` at the root, `dump:vectors` + the vectors diff, the identicon artwork
  regeneration diff, the i18n regeneration diff (minus the
  `src/i18n/resources.ts` path), the corpus defect register, both Lottie
  lints, native reachability, the two parity gates, the Safe extension steps
  and the SDK build; **drops** `tsc --noEmit`, `eslint src`, `jest --ci`.
  Its header comment — *"The Expo sources under src/ are still typechecked,
  linted and tested here — they are still the shipping native app"* — is
  rewritten to what the job now is.
- The `rust` job's step "Onboarding wire types are current
  (src/services/onboarding-core/generated)" checks the one remaining mirror
  and is named for it. The `web`, `site`, `rust`, `rust-macos`, `desktop`,
  `android`, `ios` jobs are unchanged.

### Docs — accurate, not merely scrubbed

The rule: after this cut a document either **describes the product as it
is**, or is **marked as history** (a dated banner at the top saying what it
described and where the living description is), or is **deleted**. Grepping
the word "Expo" out of a paragraph that still describes Expo is none of the
three.

- **Rewrite** (the handover authority and the front doors): `README.md` —
  the "Architecture (the Expo app, shipping today)" section and diagram go,
  "Why we are leaving Expo" becomes past tense under "The new architecture",
  the Cloudflare Pages sentence at `:227` is replaced by the truth of the
  day: *which build serves `wallet.getvela.app` right now, and where the
  Worker's live build can be opened*; `docs/project-takeover/01-system-overview.md`,
  `02-local-development.md`, `04-production-readiness.md`,
  `05-deployment-runbook.md` (the file that most misdescribes reality after
  the cut — it still says the web wallet is `dist/` on Pages built by
  `npm run build:web`, and describes EAS-managed keystores; it must now say
  the Pages deployment is frozen, why, and how the hostname is moved),
  `14-human-progress.md`, `10`/`11` (the answer key's `build:web` / `expo lint`
  lines); `docs/NATIVE-LAUNCH-CHECKLIST.md`; `docs/store-submission/privacy-and-review.md`;
  `DESIGN_SYSTEM.md:230` (`lucide-react-native`); `app-desktop/vela-wallet/README.md:540,555`;
  `agent-rules/CLAUDE-AUTO-TEST.md:79`; `docs/TEST-OUTLINE.md:54`;
  `rust/README.md:104`; `docs/https-web-wallet.md` (banner + source map);
  `docs/qr-scanner-web.md`, `docs/localization.md`,
  `docs/text-scale-architecture.md`, `docs/dynamic-amount-display.md`,
  `docs/fiat-price.md` and the `docs/requirements/*` files that cite `src/**`
  paths — each either re-pointed at the living implementation or banner'd.
- **Mark as history**: `docs/dapp-browser/ARCHITECTURE.md` (describes
  `modules/vela-wallet-webview`, which no shell has), `docs/KNOWN-BUGS.md`
  BUG-4 (the Metro polyfill race `index.js` existed for),
  `docs/safari-extension/*`.
- **Delete**: `docs/PARALLEL-SPACE-E2E-PLAYBOOK.md` (it is the manual for
  `e2e/`), and any doc that exists only to operate a deleted thing.
- **Untouched** (already true): `ROADMAP.md`, `docs/test-plan.md`,
  `docs/PARALLEL-SPACE.md`, `agent-rules/AI-CODING-RULES.md`,
  `LAUNCH_AUDIT.md`, `SECURITY-AUDIT.md`, the desktop packaging workflows.
- The Cargo/vite/tsconfig **comments** that mention Expo or Metro as the
  reason for a decision stay if the decision still stands and the sentence
  is still true in the past tense; they are rewritten if they now explain a
  constraint that no longer exists.

### The clean tree

"目录干净" is read as three tests, each of them runnable:

1. **Nothing at the root exists only for Expo.** The root listing after the
   cut holds the shells (`app-*`), `rust/`, `scripts/`, `docs/` (which now
   also holds `agent-rules/` and `design/`), `specs/`, `assets/`, the licence, `README.md`
   alone among markdown files (the founder's fourth ruling: every other
   document lives under `docs/`), and dotfiles that a surviving tool reads.
   `public/` is gone too (third ruling): its generated i18n catalogs and the
   wasm artifact live under `assets/`. **No `package.json`, lockfile or `node_modules` at the root**: the
   tooling package lives in `scripts/` (founder, 2026-09-11, second ruling:
   *"用一个专门的 scripts 目录来管理，而不是放到根目录"*). No `app.json`, no `metro.*`, no
   `index*.js`, no root `jest.*`, no root `playwright.config.ts`, no stray
   `.verify-*.mjs`, no `targets/`, no `modules/`, no `plugins/`, no `e2e/`.
2. **The working tree has no Expo residue.** `git status --ignored --short`
   at the root lists no `dist/`, `test-results/`, `.expo/`, `web-build/`,
   `e2e/`; `.gitignore` names none of them.
3. **The words are gone from code and config.** A case-insensitive grep for
   `expo`, `react-native`, `react native`, `metro`, `eas build`, `hermes`
   over every tracked file outside `specs/` returns hits only inside prose
   explicitly marked as history (a dated banner or a "was" sentence in a
   rationale), never in a command, a path, a dependency or a step.

### Success Criteria (Part A)

- **SC-391**: `git ls-files` after the cut contains no path under `src/`,
  `e2e/`, `modules/`, `plugins/`, `targets/`, `packages/safari-extension/`,
  and none of the root files listed in "What goes"; no `package.json`
  anywhere in the tree names a dependency matching `expo` or `react-native`;
  the lockfile agrees.
- **SC-392**: every consumer in the shared-floor table still builds and
  passes from a fresh clone: `app-web/vela-wallet` `pnpm check && pnpm build
  && pnpm test:unit`; `app-desktop` `cargo test`; `app-ios` bundle-catalogs
  + test job; `app-android` compile + unit-test job; `scripts/onchain`
  unchanged; every root tooling script runs green with the reshaped
  `package.json`; the SDK package builds.
- **SC-393**: CI is green on the branch with the reshaped `app` job, and each
  surviving gate still *fails on drift* — proven once, locally, by a
  one-line corpus edit (i18n) and a one-line vector edit (identicon) that
  each turn exactly their gate red and are then reverted; the two runs are
  recorded in `results.md`.
- **SC-394**: the three clean-tree tests pass verbatim, and the grep in test
  3 is committed as a script the next person can run.
- **SC-395**: a reader following `docs/project-takeover/02-local-development.md`
  from a fresh clone reaches a running web wallet, a running desktop app and
  a green root gate set without meeting a command that no longer exists; no
  document in `docs/` or at the root says, in the present tense, that the
  Expo app is what ships or that `src/` exists; and the README and the
  deployment runbook state which build serves `wallet.getvela.app` on the
  day they were last edited, and why.
- **SC-396**: the deletion is one revertible series — the deletion commits,
  then the generator/CI commits, then the docs — such that reverting the
  deletion commits alone restores a tree that builds; the runbook's rollback
  section records that the Pages project's last deployment is the fallback
  for the hostname until the Worker takes it.

## Part B — the web wallet after the cut

### What is true today, precisely

1. **The production hostname serves the Expo build**, and after the
   deletion it serves the last one, frozen (evidence in *Why*). Our own
   references still point at Expo Router paths: the marketing site links to
   `https://wallet.getvela.app/onboarding` and `/onboarding?mode=create`
   (`app-web/getvela.app/src/routes/+page.svelte:299,321,329`,
   `SiteHeader.svelte:45`, `docs/install.md:12`, `docs/create-wallet.md:13`);
   its `/pay` route forwards to `https://wallet.getvela.app/pay${search}`
   (`app-web/getvela.app/src/routes/pay/+server.ts:12`); the SDK's default
   is `https://wallet.getvela.app/web-request` (`packages/vela-sdk/src/index.ts:76`).
   The SvelteKit shell's URL space is `/{locale}/…` with `/` a wasm-free
   Accept-Language 307 (`app-web/vela-wallet/src/routes/+server.ts`). By the
   founder's ruling no compatibility shim for old paths is owed; what *is*
   owed is that **our own links stop naming paths that only the Expo build
   had** — that is fixing our links, not compatibility.

2. **The HTTPS wallet SDK has no wallet half.** `docs/https-web-wallet.md`
   describes a dApp path that needs no extension: `@vela-wallet/sdk` opens
   `wallet.getvela.app/web-request?session=…` in a popup, a `VELA_WEB_READY` /
   `VELA_WEB_INIT` handshake transfers a `MessagePort`, and consent and
   signing happen in the wallet tab. The wallet side is
   `src/app/web-request.tsx` and `src/services/web-popup-transport.ts` — Expo,
   deleted in Part A. `app-web/vela-wallet` has no `VELA_WEB_*`, no
   `MessagePort`, no `window.opener` handling; its `[locale]/request` route
   is the **extension** side-panel surface of spec 027, a different door.

3. **The payment-link bridge has a parser and no page.** `src/app/pay.tsx`
   turns `wallet.getvela.app/pay?to&chain&token&amount&sym&dec&net` into a
   prefilled send. `app-web/vela-wallet` carries the EIP-681 parser
   (`src/lib/services/eip681.ts`, tested) and the core's `PayRequest` type,
   and no route that reads a URL into it.

4. **The Expo document loads a third-party analytics script**
   (`src/app/+html.tsx:47-51`, `https://tj.appsdata.org/api/script.js`). The
   SvelteKit shell loads nothing from that host. Dropped unless the founder
   says otherwise (Assumptions).

5. **An underfunded gas account has no sheet to be told so.** The Expo build
   shows `BundlerFundingModal` (`src/components/ui/BundlerFundingModal.tsx:465`)
   when the bundler reports the account cannot cover a submission — the
   deposit address, the threshold, the recommended top-up, and a retry that
   busts the cached balance. The SvelteKit shell runs the same core steps —
   `check_bundler_funding` and `attempt_sponsorship` in
   `src/lib/signing/core/sign-executor.ts:160,180`, `funding: null` in the
   resident at `sign-resident.svelte.ts:49` — and renders **nothing** for the
   outcome: there is no funding block under `src/lib/signing/ui/` or
   `src/lib/flows/`. A person whose silent sponsorship is denied is left with
   a submission that will not go and no sentence about why. A money path;
   owed before the hostname moves.

6. **Receive cannot ask for an amount.** The Expo receive screen has a
   request mode (`src/screens/wallet/ReceiveScreen.tsx:14,73,102` —
   `useReceiveRequest`, `variant: 'request'`) that encodes amount, token and
   chain into the code and a `/pay` link. The SvelteKit shell carries the
   core session for it (`src/lib/flows/core/payment-request.ts:82`) and a
   `ReceiveQrModel` (`src/lib/flows/model.ts:181-201`) with no amount, no
   link and no control to set either. Items 3 and 6 are one capability seen
   from both ends and are owed together.

7. **The people already using the wallet on that origin — ruled.** The Expo
   web build keeps accounts, contacts, custom tokens, networks, endpoints,
   history and preferences in the browser's storage under
   `wallet.getvela.app` (`src/services/storage.ts:13-24`); the SvelteKit
   shell writes the same records, byte-compatible, into IndexedDB
   (`app-web/vela-wallet/src/lib/services/storage.ts:6,12`) with no
   migration. **Founder, 2026-09-11: no compatibility is owed; client cache
   and data may be lost.** So: no migration, no notice. What a visitor loses
   on the day the hostname moves is local records only — the wallet itself
   is behind their passkey and comes back by signing in. The register row
   records the ruling.

### The parity register

One row per user-facing capability the Expo build ships **on web**; the
sweep was file-by-file over both trees on 2026-09-11. Paths under *Expo* are
relative to `src/`; paths under *app-web* are relative to
`app-web/vela-wallet/src/`. Status: **present** · **ahead** (the SvelteKit
shell has more) · **partial** · **owed** (before the hostname moves) · **dropped —
ruling** (date, where) · **verify** (present on paper; one more look in
planning before it is called present).

**Onboarding**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Welcome → create shell | `screens/onboarding/OnboardingScreen.tsx:24` | `routes/[locale]/+page.svelte`, `routes/[locale]/create/+page.svelte` | present |
| Create wallet, multi-passkey founding keys | `screens/onboarding/CreateWalletScreen.tsx:49`; `hooks/use-create-wallet.ts:147` | `lib/ui/onboarding/v2/CreateFlow.svelte`, `KeysScreen.svelte:105` | present — same core machine on both sides |
| Key methods: platform / hybrid (caBLE) / security key | `services/onboarding-core/generated/KeyMethod.ts:19` | `lib/onboarding/generated/KeyMethod.ts:19`; `lib/ui/onboarding/v2/AddMethodPicker.svelte` | ahead — the Expo picker draws two of the three (`CreateWalletScreen.tsx:25`) |
| Sign in / recover with any key | `hooks/use-onboarding-login.ts`; `OnboardingScreen.tsx:32` | `routes/[locale]/+page.svelte:47,85` | present; `routes/[locale]/import/+page.svelte:11` is a `PlaceholderPage` — **verify** nothing links to it, or give it the login |
| Passkey provider names (AAGUID) | `services/authenticator-info.ts:17` | `lib/onboarding/core/passkey-directory.svelte.ts:43`; `lib/services/endpoints.ts:23` | ahead — offline catalog + icons |

**Home**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Multi-chain balances | `screens/wallet/HomeScreen.tsx:47`; `useHomeController.ts:631` | `lib/wallet/core/balance.svelte.ts`; `lib/wallet/ui/BalanceDisplay.svelte` | present |
| Hide balance | `HomeScreen.tsx:97,252`; `hooks/use-balance-privacy.ts` | `lib/wallet/core/balance.svelte.ts:62`; `BalanceDisplay.svelte:21` | present |
| Activity feed: ERC-20 + EIP-7708 native logs | `HomeScreen.tsx:238`; `services/transfer-monitor.ts:41-45` | `lib/wallet/core/feed.svelte.ts`; `lib/services/incoming-transfers.ts` | **partial** — the EIP-7708 sentinel-address path has no twin (`lib/services/recipient-identity.ts:193` names it in a comment only) → verify against the core's feed rule; port if the core lacks it (owed) |
| Pending tx / tracker | `hooks/use-tx-settlement.ts`; `services/tx-reconciler.ts` | `lib/wallet/core/tracker-resident.ts`; `lib/services/tx-reconciler.ts` | present |
| Fiat conversion | `hooks/use-display-currency.ts`; `services/fiat-rates.ts` | `lib/settings/core/currency.svelte.ts`; `lib/services/fiat-rates.ts` | present |
| Network filter | `HomeScreen.tsx:319` | `lib/wallet/chain-filter.svelte.ts`; `ChainFilterList.svelte` | present |
| Balance detail / unpriced-tokens sheet | `HomeScreen.tsx:334`; `useHomeController.ts:644,650` | `lib/settings/live.ts:1111,1166`; `BalanceDetailBody.svelte` | present |
| RPC trouble banner + fix modal | `HomeScreen.tsx:141` | `lib/settings/ui/RpcBanner.svelte`; `live.ts:1057` | present |
| Account switcher | `HomeScreen.tsx:399` | `lib/session/ui/AccountSwitcher.svelte` | present |
| Token detail | `screens/wallet/TokenDetailScreen.tsx` | `lib/flows/screens/TokenDetail.svelte`; `AssetDetailPanel.svelte` | present |
| Add custom token / manage tokens | `screens/wallet/AddTokenScreen.tsx`; `hooks/use-manage-tokens.ts` | `lib/flows/screens/AddToken.svelte`; `lib/wallet/core/manage-tokens-session.ts:26` | present |
| Add network (wizard + compatibility probe) | `screens/settings/SettingsScreen.tsx:393,1313` | `lib/settings/ui/AddNetworkPanel.svelte`; `network-admin.svelte.ts` | present |

**Send**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Single send | `screens/wallet/SendScreen.tsx`; `useSendController.ts` | `lib/flows/screens/SendForm.svelte`, `SendConfirm.svelte`; `lib/flows/core/send-session.ts` | present |
| Split (1 → N) | `components/send/MultiRecipientEditor.tsx`; `send-controller-types.ts:190` | `lib/flows/model.ts:407`; `lib/flows/live-send.ts` | present |
| Sweep (N → 1) | `send-controller-types.ts:241`; `hooks/use-token-multi-select.ts` | `lib/flows/model.ts:407`; `live-send-sweep.test.ts` | present |
| Batch / payroll import (paste, csv, xlsx, fiat → token) | `SendScreen.tsx:206`; `services/recipient-table.ts:294`; `hooks/use-batch-import.ts` | `lib/flows/screens/BatchImport.svelte`; `lib/flows/core/batch-executor.ts:37`; `lib/services/file-io.ts:162` | present |
| Fee-token selector | `components/ui/FeeTokenSelector.tsx`; `hooks/use-inband-fee-tokens.ts` | `lib/flows/screens/FeeTokenPick.svelte`; `fee-quote.svelte.ts` | present |
| Treasury bootstrap sheet | `SendScreen.tsx:188` | `routes/[locale]/wallet/+page.svelte:1351,1674`; `RelayerBody.svelte` | present |
| **Gas-account funding sheet** (bundler pre-fund, top-up, retry) | `components/ui/BundlerFundingModal.tsx:465`; `components/signing/SigningRequestModal.tsx:8` | core + executor only (`sign-executor.ts:160,180`; `sign-resident.svelte.ts:49`); no sheet | **MISSING → owed** (item 5) |
| Scan QR — camera | `components/QRScanner.tsx:513,578` | `lib/flows/core/scanner.svelte.ts`; `ScanSurface.svelte` | present |
| Scan QR — from a picked image | `QRScanner.tsx:546`; `services/image-decode.ts` | `scanner.svelte.ts:139`; `lib/services/qr-decode.ts` | present |
| EIP-681 on scan / paste | `screens/wallet/send-utils.ts`; `services/eip681.ts` | `routes/[locale]/wallet/+page.svelte:91,877`; `lib/services/eip681.ts` | present |

**Receive**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Address QR | `screens/wallet/ReceiveScreen.tsx:2` | `lib/flows/screens/ReceiveQr.svelte`; `lib/wallet/qr.ts` | present |
| Share card image | `ReceiveScreen.tsx:4,19` | `lib/flows/share-image.ts`; `ShareCard.svelte` | present |
| Incoming-deposit watch | `hooks/use-receive-watch.ts` | `lib/flows/core/receive-watch.ts` | present |
| **Request mode** (amount + pay link in the code) | `ReceiveScreen.tsx:14,73,102` | `lib/flows/core/payment-request.ts:82` (session only); `lib/flows/model.ts:181-201` has no amount/link | **MISSING → owed** (item 6) |
| **`/pay` landing page** | `app/pay.tsx`; `screens/wallet/PayScreen.tsx:48,137` | `lib/services/eip681.ts:209` builds `${origin}/pay`; no route | **MISSING → owed** (item 3) |

**Contacts**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| CRUD | `services/contacts.ts:183,211,229`; `components/contacts/ContactsManager.tsx` | `routes/[locale]/contacts/+page.svelte`; `lib/contacts/core/contacts.ts:23` | present — own route |
| Groups | `contacts.ts:337,368`; `GroupEditor.tsx` | `lib/contacts/ui/GroupEditSheet.svelte`, `GroupRail.svelte` | present |
| Import / export (JSON + CSV) | `services/contact-io.ts:290,335,339` | `contacts/+page.svelte:50,278` | present |
| Identicon avatars + viewer | `components/ui/Identicon.tsx`; `ContactAvatar.tsx`; `IdenticonViewerSheet.tsx` | `lib/wallet/ui/Identicon.svelte`; `IdenticonViewer.svelte` | present |
| Contact picker inside Send | `components/contacts/ContactPicker.tsx` | `lib/flows/screens/ContactPick.svelte` | present |

**dApps and signing**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| EIP-1193 / EIP-6963 provider via browser extension | — | `extension/inpage.js:2,244,417`; `lib/dapp/transport.ts` | app-web only (spec 027) — not a row owed |
| **HTTPS wallet SDK popup** (`/web-request`, `VELA_WEB_*`) | `app/web-request.tsx`; `services/web-popup-transport.ts`; `packages/vela-sdk/src/protocol.ts` | none — no `VELA_WEB`, `MessagePort` or `window.opener` handling under `app-web/` | **dropped — ruling** 2026-09-11 (*"packages/vela-sdk 这个能删掉了吗"* → deleted with `docs/https-web-wallet.md` and its CI step; never published to npm, no consumer) |
| WalletPair relay pairing + Connect screen | `screens/connect/ConnectScreen.tsx:38`; `services/walletpair-transport.ts:135` (*"web and mobile"*) | none | **dropped — ruling**, spec 027 input (2026-09-04): *"现在不用支持 walletpair 以及 remote inject 因为它们不成熟"*. The Expo tree is the last implementation; history keeps it |
| Remote-inject (SSE + POST) bridge | `services/dapp-transport.ts:81,262` | none | dropped — same ruling |
| Session list / disconnect | `screens/wallet/ConnectionsView.tsx:248` (live session card) | `routes/[locale]/settings/+page.svelte:52,139,257` (per-origin grants, revoke one / all) | present in a different shape — grants, not live sessions, because the transport is the extension; nothing to port |
| Clear-signing sheet (ERC-7730) | `components/signing/views/ClearSignView.tsx`; `services/clear-signing.ts` | `lib/signing/SigningSheet.svelte`; `lib/signing/core/clear-executor.ts` | present |
| Simulation / balance-change preview | `components/signing/BalanceChangePreview.tsx`; `services/tx-simulation.ts` | `lib/signing/ui/BalanceChanges.svelte`; `lib/services/sim/tx-simulation.ts` | present — both engines |
| Approval guard / editable allowance | `EditableApproveCard.tsx`; `services/approval-guard.ts` | `lib/signing/ui/AllowanceEditor.svelte`; `guard-session.ts` | present |
| Message signing (personal_sign, typed data, SIWE, eth_sign danger) | `MessageSignView.tsx`, `PermitSignView.tsx`, `EthSignDangerView.tsx`, `BlindTypedDataView.tsx` | `lib/signing/model.ts:108-146`; generated `ClearMessageView.ts`, `ClearSiweFields.ts` | present |
| Batch-calls view | `components/signing/views/BatchCallsView.tsx` | `lib/signing/core/clear-batch.ts` | present |
| Receipt / transaction detail | `components/ui/TransactionReceipt.tsx`; `TransactionDetailSheet.tsx` | `lib/flows/screens/SendReceipt.svelte`, `TxDetail.svelte`; `lib/wallet/live-detail.ts` | present |

**Settings**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Language (15 locales) | `SettingsScreen.tsx:1239`; `i18n/shared.ts:46` | `routes/[locale]/settings/+page.svelte`; `lib/i18n/locales.ts:12` | present — plus real locale URLs |
| Currency | `SettingsScreen.tsx:1269` | `lib/settings/core/currency.svelte.ts` | present |
| Number / date / time formats | `SettingsScreen.tsx:1273,1276,1279` | `lib/settings/fixtures.ts:242,249,256`; `lib/services/locale-format.ts` | present |
| Theme / avatar style / text scale | `SettingsScreen.tsx:1246-1252` | `lib/settings/model.ts:538`; `TextScaleSlider.svelte` | present |
| Networks / RPC providers / endpoints | `SettingsScreen.tsx:1306,1309,1316` | `NetworksPanel.svelte`, `RpcProvidersPanel.svelte`, `EndpointsPanel.svelte` | present |
| Storage accounting + per-row clear | — | `lib/services/device-storage.ts`; `StoragePanel.svelte` | app-web only |
| Feedback / bug report | `SettingsScreen.tsx:1218`; `services/bug-report.ts:110` (in-app submit through the site's proxy) | `lib/settings/ui/FeedbackBody.svelte:43` (a GitHub link) | **partial → owed** — the proxy (`getvela.app/api/bug-report`) already exists; the shell only has to use it |
| Erase device | `services/erase-device.ts:95`; `SettingsScreen.tsx:1479` | `lib/services/erase-device.ts:48`; `fixtures.ts:732` | present |
| Sign out | `SettingsScreen.tsx:1352` | `lib/session/ui/SignOutSheet.svelte` | present |
| About | `screens/settings/AboutScreen.tsx` | `lib/settings/ui/AboutPanel.svelte` | present |
| Rescue states (RPC down, index down, relayer) | `RpcTroubleBanner.tsx`; `RpcProvidersModal.tsx` | `lib/settings/live.ts:1007,1057,1204`; `IndexDownScreen.svelte` | ahead |

**Persistence, routing, dev surfaces**

| Capability | Expo | app-web | Status |
| --- | --- | --- | --- |
| Persisted state: accounts, custom tokens, networks, endpoints, history, prefs, contacts | `services/storage.ts:13-24` (browser storage on web) | `lib/services/storage.ts:26` (IndexedDB, byte-compatible key names); `lib/services/records.ts` | present as a capability; existing visitors' data on the origin: **ruled 2026-09-11 — may be lost, no migration** (item 7) |
| Locale routing | runtime detection, no URL locale (`i18n/shared.ts:135`) | `routes/[locale]/+layout.server.ts:17`; `routes/+server.ts:8` | app-web only |
| Third-party analytics script | `app/+html.tsx:47-51` | none | dropped unless ruled otherwise (Assumptions; item 4) |
| Parallel space | `app/parallel/_layout.tsx:18`, `parallel/index.tsx:16`; `services/dev/parallel-space.ts` | `routes/[locale]/parallel/+page.svelte`; `lib/dev/parallel-space.ts` | present; `app/parallel/connect.tsx` has no twin — it goes with WalletPair |
| `vela.*` console fault injection | `services/dev/fault-injection.ts:16-22` | `lib/services/fault-injection.ts`; `dev-console.ts:16` | present |
| Design gallery | `app/design-gallery.tsx` | `routes/[locale]/gallery/**`; `routes/dev/gallery` | ahead |
| Clear-signing harness + scenario corpus | `app/clear-signing-test.tsx:16`; `screens/settings/clear-signing-scenarios.ts` | `lib/signing/fixtures.ts` gallery states only | dropped — dev-only (Assumptions); the corpus is read only by Expo and `e2e/` |
| Receipt / batch-send harness | `app/receipt-harness.tsx:14` | none | dropped — dev-only |

**Native-only in Expo — not expected on web, not rows**

| Capability | What makes it native-only |
| --- | --- |
| In-app dApp browser (native WebView + provider injection) | `app/browser.tsx:368` bails on web; `modules/webview/index.tsx:31` `isWalletWebViewSupported = ios ‖ android` |
| Safari Web Extension setup screen | `app/safari-extension.tsx`; gated `Platform.OS === 'ios'` at `SettingsScreen.tsx:1227` |
| `velawallet://sign?rid=` trampoline + extension sign bus | `app/sign.tsx`; `services/extension-sign-bus.ts`; `extension-bridge-transport.ts` |
| iOS App-Group account sync | `services/app-group-account-sync.ts`; `services/dev/app-group-echo.ts` |
| Camera zoom / torch / scan line; native image picker | `components/QRScanner.tsx:187,460,510,546` |
| Android back handler in the browser | `app/browser.tsx:149` |
| Haptics; native pull-to-refresh gesture | `components/ui/VelaRefresh.tsx:152,223`; `services/platform.ts` |
| Save-to-photos permission path for the share card | `screens/wallet/ReceiveScreen.tsx:124,368` |

Nothing under `app-web/vela-wallet` imports from `src/`; its only
out-of-tree reads are `rust/pkg-web`, `public/i18n`, two design contracts
under `specs/`, and its own `extension/` and `scripts/`. The one generator
that writes into both trees is `rust/scripts/gen-core-types.mjs` (Part B).

### Owed before the hostname moves — not gating the deletion

The register's `MISSING`, *partial* and *verify* rows, with the size of each
as far as the sweep can tell. They are recorded here so the founder can
choose the day; none of them holds Part A back.

| Owed | Where the pieces already are | Size |
| --- | --- | --- |
| Gas-account funding sheet (item 5) | core state, executor and wire type present; one sheet in `src/lib/signing/ui/` reading `funding` and driving retry | small |
| Receive request mode + `/pay` page (items 3, 6) | `payment-request.ts` session, `eip681.ts` parser + `buildPayLink`, `PayRequest` type; an amount control on `ReceiveQr`, a `[locale]/pay` route into the send flow | small–medium |
| Our own links (item 1) | marketing site ×6, SDK default, docs | trivial; do it in Part A's doc pass |
| In-app bug report submit | `getvela.app/api/bug-report` proxy exists; `FeedbackBody.svelte` links out | small |
| ~~HTTPS SDK wallet half (item 2)~~ | dropped 2026-09-11 — the SDK package is deleted, nothing is owed | — |
| EIP-7708 sentinel-address feed path | verify the core's feed rule covers it; port if not | verify |
| `[locale]/import` placeholder | either route it to the login or remove the page | trivial |

### The move itself, whenever it happens

A checklist, not a phase — the founder may run it inside this feature or on
any later day:

1. Our own links point at the shell's paths (above).
2. The Worker `vela-wallet-web` is given the custom domain
   `wallet.getvela.app`; the Pages project is detached from the hostname
   and its automatic deployments paused.
3. Smoke on the hostname: `/` answers a 307 to a locale; `/en/wallet`
   contains no `/_expo/` path; create, sign in, balance, send confirm,
   receive, contacts, settings each open on the deployed Worker — recorded
   as a curl table plus one screenshot per screen in `results.md`.
4. Rollback: re-attach the hostname to the Pages project's last deployment
   (static, stateless, seconds).
5. The README and `05-deployment-runbook.md` are updated on the same day to
   say the Worker serves the hostname and the Pages project is retired.

### Success Criteria (Part B)

- **SC-397**: the register has a row for every route under `src/app/` that
  was reachable on web and every capability the sweep lists; every row reads
  *present*, *ahead*, *partial*, *verify*, *owed* or *dropped*, and every
  *dropped* row cites a dated founder ruling (this spec's Input, or spec
  027's); the native-only list names, per entry, the module or platform
  gate that made it native-only.
- **SC-398**: no tracked file outside `specs/` and outside history banners
  names a `wallet.getvela.app` path that exists only in the Expo build
  (`/onboarding`, `/pay`, `/web-request`, `/parallel/connect`); the
  marketing site's links, the SDK's default URL and the docs name the
  shell's paths or say the path is owed.
- **SC-399**: the "Owed before the hostname moves" table is carried into
  `results.md` in its final state, each row marked *done in 039* or *still
  owed*, with the ruling and date for anything dropped.
- **SC-400**: if the founder takes the move inside this feature: the smoke
  table in step 3 is recorded before and after, and the README and runbook
  are changed on the same day; if not: the README and runbook say the Pages
  deployment is frozen at the last Expo commit, name that commit, and point
  at the checklist.

## Requirements

### Functional Requirements

- **FR-391**: The deletion MUST remove every path in "What goes" and the two
  Safari packages, and MUST NOT remove any path in the shared-floor table;
  each shared-floor consumer MUST be re-verified by running it.
- **FR-392**: The type generator MUST write each wire-type family to exactly
  one place, under `app-web/vela-wallet`; its `--check` MUST diff only that
  place; the i18n generator MUST no longer emit a TypeScript resources file.
- **FR-393**: The root `package.json` MUST retain only tooling scripts and the
  dependencies those scripts import; the CI `app` job MUST run only steps
  that have a surviving subject, and its header MUST describe what it runs;
  the SDK build step MUST remain.
- **FR-394**: Every document in the "Rewrite" list MUST describe the post-cut
  repository; every document in the "Mark as history" list MUST open with a
  dated banner naming what it described and where the living description
  is; every document in the "Delete" list MUST be gone; the README and the
  deployment runbook MUST state which build serves `wallet.getvela.app` and
  why, as of their last edit.
- **FR-395**: The three clean-tree tests MUST pass, and test 3 MUST be a
  committed script.
- **FR-396**: The deletion MUST be committed as a revertible series in which
  the deletion commits are separable from the generator/CI and doc commits.
- **FR-397**: The parity register MUST be complete per SC-397 before the
  branch is offered for merge; a row MAY read *owed* — the deletion does not
  wait on it — but MUST NOT read `MISSING` without either *owed* or a dated
  ruling.
- **FR-398**: Our own references to Expo-only paths on `wallet.getvela.app`
  MUST be updated to the shell's paths (or to "owed") in this feature; no
  redirect shim for old paths is required (founder ruling, 2026-09-11).
- **FR-399**: No migration of, and no notice about, the Expo build's
  client-side data is required (founder ruling, 2026-09-11); the ruling
  MUST be recorded in the register and in `results.md`.
- **FR-400**: `results.md` MUST be written as the work lands (per 033–038)
  and MUST contain: the drift-gate proof runs, the clean-tree test output,
  the register's final state, the owed table's final state, the list of
  rulings with dates, and — if the move is taken — the smoke tables.

### Key Entities

- **Parity register**: one row per user-facing web capability of the Expo
  build — capability, Expo evidence (path:line), app-web evidence (path:line)
  or none, status ∈ {present, ahead, partial, verify, owed, dropped}, ruling
  (date + where) when dropped.
- **Shared floor**: the root paths with a non-Expo consumer, each with its
  consumers named and the command that proves them.
- **Doc disposition list**: every tracked document that mentions Expo, React
  Native, Metro or EAS → rewrite / history / delete / untouched, with the
  line that decided it.
- **Owed table**: what the shell lacks before the hostname moves — each with
  where its pieces already are and a size.
- **Move checklist**: the five steps above, with the smoke table's shape.

## Assumptions

- **The Expo native builds owe no parity.** They have not been able to run
  since PR #168 (the facade is wasm-only and Hermes has no WebAssembly —
  `README.md:130`), no store listing is live, and the native shells are
  their successors. The register is web-only by construction.
- **Delete first is the founder's order, and its cost is the frozen Pages
  deployment** (*Why*). Nothing here schedules the hostname move; the
  checklist is ready whenever it is taken.
- **No compatibility with the Expo web build is owed** — neither its URL
  paths nor its client-side data (founder, 2026-09-11). Updating our own
  links is not compatibility; it is correctness.
- **The analytics script is not carried over unless the founder says so.**
  The SvelteKit shell loads nothing from `tj.appsdata.org` today and the
  product's stance is trust over disclaimers; the default is to leave it out
  and record that as a ruling to confirm.
- **The dev-only Expo surfaces** (`clear-signing-test`, `receipt-harness`,
  `design-gallery`, `/parallel/connect`) are dropped where the SvelteKit
  shell has no stated equivalent; the fault-injection console, the gallery
  and the parallel space have one and are *present*.
- **The two package defaults** (Safari target and package deleted; SDK
  package kept) stand until the founder says otherwise; either is reversed
  in one line and neither blocks Part A.
- **`docs/project-takeover/` is rewritten, not deleted**, however much of it
  describes the Expo app: it is the handover authority and the
  takeover-training baseline reads it first.
- **Frozen goldens stay frozen.** `primitives/abi/eip712/safe/webauthn.json`
  have no generator (PR #168); nothing here regenerates or moves them.
- `results.md` is written as the work lands, per 033–038.
