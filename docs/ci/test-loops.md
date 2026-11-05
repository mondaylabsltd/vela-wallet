# Test loops in CI (native parity Phase 5)

Fast checks gate every pull request. Heavy suites run nightly, or on demand, and
gate nothing until each has a proven-stable record on a runner.

| Workflow | When | Blocks a merge? |
|---|---|---|
| `.github/workflows/ci.yml` | every pull request, every push to `main`, by hand | yes, for the jobs branch protection names (below) |
| `.github/workflows/nightly.yml` | 03:17 UTC daily, and `gh workflow run nightly.yml` | no |

## Required on pull requests (`ci.yml`)

| Job | What it runs |
|---|---|
| `app` | The tooling gates: corpus oracles, generated artefacts, the i18n lint (`lint-i18n-corpus`) and verify (`verify-i18n-parity`), the identicon parity, Lottie, reachability, events, dead controls, Expo residue. Also the **scenario manifest check** (`scripts/ci/check-scenarios.mjs`, with its matrix in the job summary). |
| `rust` | `cargo fmt --check`. Clippy `-D warnings` and `cargo test --workspace`, both with `vela-core/i18n-all,dev-fixtures,crux`. The wasm canary. **`build-web.mjs --check`** (wasm freshness) and `verify-web.mjs`. Onboarding types. The Swift bindings. The corpus through the Kotlin bindings. |
| `rust-macos` | The corpus through the Swift bindings. |
| `web` | `sync:wasm`, then `pnpm check` and `pnpm lint`. `pnpm build`, `build:extension` and `build:extension:safari`. **vitest server + client** (Chromium). Then an assertion that the vector-replay files ran with nothing skipped. Then **one Playwright spec, `e2e/extension-site-start.e2e.ts`** (below). |
| `desktop` (Linux) | `cargo fmt --check` and **`cargo check --all-targets --locked`**. Clippy and `cargo test`, each run without and with `dev-fixtures`. |
| `desktop-windows` | `cargo check --all-targets --locked` on Windows. |
| `android` | **`check-native-wire --strict`**. Assemble the debug build and run the JVM suite (this includes **CoreWireDriftTest**). Then an assertion that CoreWireDriftTest and every vector-replay class ran with nothing skipped. |
| `ios (unit)` | **`check-ios-wire-parity --strict`** (with its self-test). The unit suite on a simulator (macos-15). An assertion that the vector-replay suites and the manifest's required tests ran with nothing skipped. **`check-ios17-function-metadata`** (with its self-test). |
| `ios (ui)` | The XCUITest leg. It runs on a push to `main`, and on a PR only when the PR carries the `ios-ui` label. |
| `site` | getvela.app's `bun run check`. |

### The one e2e that gates every pull request

`e2e/extension-site-start.e2e.ts` is the only Playwright spec in `ci.yml`
(job `web`, after vitest). It guards where the Chrome extension starts a new
site — on a network the account lives on, never the space's first — which
regressed three rounds running (r9, r10, r12). Its second test is round 12's
pass: an account funded on Arc Testnet after the wallet last read it, a slow
node, the real side panel; without the fix it fails exactly as the round did
("Network Sepolia"). It runs headless (`--headless=new`, no display) on the
extension built earlier in the job; the preview it needs is started from the
bundle `pnpm build` made, never built a second time. It was added straight to
`ci.yml` (coordinator, round 12) without the nightly streak below: a
regression this often is a merge blocker, and the spec is hermetic (stub
nodes, a local dApp, no outside network).

```sh
cd app-web/vela-wallet
pnpm exec playwright test --project=chromium e2e/extension-site-start.e2e.ts
```

### The shared core vectors are never silently skipped

The core's vectors (`rust/crates/vela-core/tests/vectors`) are replayed inside
each shell's own suite. `docs/scenarios/multisig.yaml` → `vector_replays` lists
those suites per shell. `scripts/ci/assert-test-results.mjs` then reads what
each job actually ran:

- **Android** from Gradle's JUnit XML;
- **web** from vitest's JUnit XML;
- **iOS** from the xcresult (`xcresulttool get test-results tests`).

The job fails if any listed suite ran nothing, or skipped or failed anything.
The same goes for every scenario test that the manifest marks `ci: required`
on `android-jvm`, `web-unit` and `ios-unit`.

Rust has no machine-readable test report on stable. There the static check
stands in: no required test may carry `#[ignore]`. The only ignored vela-core
tests are these:

- the fork test, which runs nightly;
- three that write or print vectors;
- a bench.

## Nightly (`nightly.yml`, non-blocking)

Run it by hand like this:

```sh
gh workflow run nightly.yml --ref <branch>
gh workflow run nightly.yml --ref <branch> -f only=trusted-signer,web-e2e
gh workflow run nightly.yml --ref <branch> -f only=multisig-fork -f genesis_ref=<sha>
```

A schedule fires only on the default branch. Until `nightly.yml` is on `main`,
it runs only by hand.

| Job | Runner | What | Artifacts |
|---|---|---|---|
| `web-e2e` ×3 | ubuntu-24.04 | `playwright test --project=chromium|firefox|webkit`. Chromium runs every suite, extensions included. Firefox and WebKit run the storage suites. | JUnit, HTML report, traces, screenshots |
| `trusted-signer` | ubuntu-24.04 | `scripts/ci/trusted-signer-suites.sh`: the 22 suites in `app-web/trusted-signer/HANDOVER.md`, run in Chrome for Testing (`@puppeteer/browsers`) with bun 1.4.2. `$SB` is generated per run by `scripts/ci/trusted-signer-sb.sh` (see below). | a log per suite, `summary.md` |
| `multisig-fork` | ubuntu-24.04 | Starts `scripts/onchain/multisig-fork/up.sh` (two anvil forks, the genesis service, the PeerConnect relay, the bundler relay). Then runs `playwright test -c playwright.onchain.config.ts` against `pnpm dev`, then starts the harness again and runs vela-core's `multisig_fork` test (`--ignored`) on it. Each suite has a harness of its own (r12): the core test sends from the fixture accounts' addresses (`anvil_impersonateAccount`), which gives them a nonce, so a web test that deploys one of their counterfactual accounts afterwards fails (AA13); and the web suite spends the service sponsor's five records per owner a day, so the core test's create is not recorded after it. | harness logs, Playwright results and screenshots |
| `android-macrobench` | ubuntu-24.04 + KVM | The core for x86_64 only (`scripts/ci/android-core-emulator.sh`) and the two `benchmarkRelease` APKs. Then, on an API 34 `google_apis` emulator: `run-macrobenchmarks.sh`, `summarize-macrobenchmarks.py` and PSS. | benchmark JSON, Perfetto traces, summary |
| `android-instrumented` | ubuntu-24.04 + KVM | The core and the parallel space's fixtures for x86_64 only (`android-core-emulator.sh x86_64 --dev-fixtures`), then `:app:connectedDebugAndroidTest` on the same API 34 emulator, animations off. The run summary lists every failed test. | JUnit XML and the HTML report (`androidTest-results`, `reports/androidTests`) |
| `ios-perf` | macos-15 | `app-ios/scripts/perf/run-perf.sh` with the **release** core (what ships), then `report-perf.mjs --json`. | `perf.json`, the report, the zipped xcresult |
| `desktop-perf` | macos-15 | `cargo build --release --features dev-fixtures`, then `scripts/perf-run.sh --runs 5`. | logs, `summary.txt` |

### The Android instrumented suite

`app/src/androidTest` (169 tests on 2026-11-05) ran nowhere until
`android-instrumented`. When it was first run on an emulator, 52 tests failed.
The same 52 failed at `8a71f5f46`, the start of the native-parity rounds. None
of them was a regression from those rounds:

| Cluster | Tests | What it was | Kind |
|---|---|---|---|
| Chrome's first-run screen, and then the wallet restarting itself | 45 signing-sheet, rescue-sheet, site-lock and settings tests, and the Custom Tab probe | The probe's first tab on a fresh emulator opened Chrome's first-run screen, which then covered every later test. The probe now starts Chrome with `--disable-fre` and closes it afterwards (`DeviceBrowser`). Under that sat a product bug: `TrustedSignerChannel.endFlow()` brought the wallet back even when no flow had been open, so pressing Back on Welcome restarted the app (fixed). | environmental, plus a product bug |
| Persistence phases | 3 (contacts, currency) | They ran last phase first in one invocation. They now run in a fixed step order. | test |
| Launch-animation fade | 1 | `setContent` played the whole Lottie animation before the test took control of the clock. With animations off, Lottie also jumps straight to its last frame. The clock now stops first, the asset is parsed first, and the test steps to the dissolve frame by frame. | test |
| Status-line cut | 1 | A fixed 85 % + 3 % ratio was wrong. The line is now measured against its own floor. | test |
| Screenshot ✕ after a scroll | 1 | The fling carried the ✕ out of the window, so the tap missed. The test now scrolls it back first. | test, device-dependent |
| Escape probe (P2b) | 1, about half the runs | The WebView was destroyed while the page's icon fetch was still in flight, so the fetch escaped the interceptor. The page is now kept alive while the test listens. | flaky |
| Multisig reject flow | 1, now and then | A recomposition ran on the hub's IO thread under the unconfined test dispatcher. It now uses `StandardTestDispatcher`. | flaky |
| Explore tab, TalkBack (found while getting to green) | 1, now and then | The node was read before Compose had passed the change to the accessibility layer. The read now waits for the state it expects. | flaky |

A red night here means a regression, or a flake to make deterministic. Never
rerun it to get green, and never `@Ignore` a test to get green.

Locally, on an emulator (never a phone: the suite clears the app's data):

```sh
cd app-android/vela-wallet
ANDROID_SERIAL=emulator-5554 ./gradlew :app:connectedDebugAndroidTest -PvelaSkipRustBuild
```

The suite passes with animations on and with them off.

The perf numbers are **reported, never enforced**. Shared runners are noisy:
the figures compare nights on one runner class, not devices. Each perf job
writes its numbers to the run summary.

### What `$SB` holds for the trusted-signer suites

`test-kit.mjs` serves the page over TLS on a real `getvela.app` origin, because
a passkey's rpId check happens in the browser process. It maps `getvela.app:443`
to a local port with `--host-resolver-rules` and starts Chrome with
`--ignore-certificate-errors`. `$SB` therefore needs three files:

- `cert.pem` and `key.pem`: a self-signed certificate for `CN=getvela.app`,
  `subjectAltName=DNS:getvela.app`;
- `tls-serve.py`: the static TLS server from the handover.

`scripts/ci/trusted-signer-sb.sh <dir>` makes all three at job time. The key is
throwaway and is never uploaded or committed. `gen-fonts --check` needs the
same fontTools and brotli that made `src/fonts.css` (pinned:
`fonttools==4.54.1`, `brotli==1.1.0`).

Locally:

```sh
CHROME_BIN="$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing" \
  scripts/ci/trusted-signer-suites.sh /tmp/ts-out            # all 22
scripts/ci/trusted-signer-suites.sh /tmp/ts-out multisig-test  # one
```

### The multisig fork suite and its services

The harness runs two services that live in other repositories. CI checks both
out at **pinned** commits:

| Service | Repository | Pin |
|---|---|---|
| genesis service with the proposal mailbox (`genesis-server`, plus `contracts/` for the registry's bytecode; `forge-std` is a submodule) | `mondaylabsltd/safe-genesis-registry` (public) | `vars.SGR_MAILBOX_REF`, or the `genesis_ref` input. **If neither is set, the job skips** and says why. |
| PeerConnect relay (`peerconnect-relay`) | `atshelchin/peerconnect` (public) | `vars.PEERCONNECT_REF`, default `11f8ca81a99172b1a38f76edce682e05b6118638` (main, 2026-11-02) |

When a ref is unset or cannot be fetched, `scripts/ci/onchain-gate.sh` stops
the job green. It names exactly what is missing in the log, as a warning, and
in the summary.

## What the owner must provide

**Repository variables** (Settings → Secrets and variables → Actions →
Variables):

- `SGR_MAILBOX_REF` (needed for the fork suite to run at all). The build that
  the wallet's multisig suites pass against today is the local worktree
  `/Volumes/data/production3/sgr-mailbox`, on branch `mailbox-any-signer`
  (commit `72ad2f69f395b01046eb7b94b1289edec15c7e66`). That commit is **not on
  GitHub**: the repository has only `main` (`cb7818047`).
  1. Push the branch, or merge it.
  2. Set `SGR_MAILBOX_REF` to the pushed commit's SHA.
  3. Re-pin it whenever the wallet starts depending on a newer mailbox.
- `PEERCONNECT_REF` (optional). Set it to move the relay pin.

**Secrets** (names only; none is needed for a clean run or skip):

- `GNOSIS_FORK_RPC_URL` (optional): a Gnosis RPC, preferably an archive node,
  for anvil to fork from. Without it the harness forks
  `https://rpc.gnosischain.com`, which rate-limits a cold fork.
- `ONCHAIN_REPOS_TOKEN` (optional): a fine-grained token with read access to
  the two service repositories. It is needed only if either repository is ever
  made private.

No workflow prints a secret, and none writes one to an artifact.

**Branch protection** (Settings → Branches → `main`): add these as required
checks:

- `app`
- `site`
- `web`
- `rust`
- `rust-macos`
- `desktop`
- `desktop-windows`
- `android`
- `ios (unit)`

`ios (ui)` and everything in `nightly.yml` stay optional.

**Runners.** Only GitHub-hosted runners are used:

- `ubuntu-24.04`. The emulator needs KVM, which standard Linux runners expose;
  the job enables it with a udev rule.
- `macos-15`, for iOS and desktop-perf.
- `windows-2025`.

On a public repository none of these minutes are billed. If the repository goes
private, the two nightly macOS jobs cost about 10× Linux minutes. Drop them to
weekly (a second `cron`) or to on demand.

## Promoting a nightly job

Move a job from `nightly.yml` into `ci.yml` (and branch protection) only after
**ten consecutive green nights** on `main`, with no rerun needed. Record the
streak in the PR that promotes it.

The candidates, in order:

1. `trusted-signer` (hermetic, about 2 minutes);
2. `web-e2e` firefox and webkit (small);
3. `web-e2e` chromium;
4. `multisig-fork`, once its service pin is stable.
5. `android-instrumented` (9–22 minutes for build and suite on a local emulator). It is
   heavy for every PR, so `main`-only would be its first step.

The perf jobs stay nightly. They report numbers; they don't decide anything.

## The scenario manifest

`docs/scenarios/multisig.yaml`: when you add, rename or delete a test that
covers one of its scenarios, update the entry in the same PR. `check-scenarios`
fails the `app` job on a name it can't find. A gap filled is a `gap:` replaced
by an entry.

```sh
node scripts/ci/check-scenarios.mjs            # matrix + gaps + problems
node scripts/ci/assert-test-results.mjs --platform android \
  --junit app-android/vela-wallet/app/build/test-results/testDebugUnitTest
```
