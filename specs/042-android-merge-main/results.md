# 042 — Results: the Android line on the post-Expo base

**Branch**: `042-android-merge-main` · **Date**: 2026-09-12 · **Status**: merged
locally, gates run locally, not pushed.

The founder's three instructions, in the order they arrived: the merge's
changes go on a 042 branch, not on 041; the Kotlin unit tests must move to
`assets/i18n`; and speckit gets a 042 feature too. This document is the record
FR-008 asks for: every conflicted file with its resolution, every change beyond
the conflicts with its reason, and what each gate said.

## The merge

| Side | Tip | Commits since the fork (`28d25ae9`) |
| --- | --- | --- |
| Android line (041) | `d35ab422` | 28 |
| `origin/main` (039) | `aa91fab4` | 199 |

Five files were touched on both sides; two auto-merged (the app's
`build.gradle.kts` — main renamed paths, 041 added the serialization plugin
and the wire-mirror test input; the balance test file — 041's four
`first_grouped_quote_price` tests beside main's four #188 tests, no name
clash). Three conflicted:

| File | What each side did | Resolution |
| --- | --- | --- |
| `navigation/VelaNavHost.kt` | main: 探索 became a *section* of the wallet route (`section` state, Back unwinds it, rotation keeps it), `select` lambda over every tab, 通讯录 still `Unit`. 041: the wallet body became live (`WalletLive.home`), `onFlow` grew the row id, the 通讯录 tab navigates. | **Union.** One `select` carries all four tabs — Settings pushes, Contacts pushes (041), Explore/Wallet flip the section (main). The wallet branch keeps 041's live model and two-argument `onFlow`; the explore branch is main's, untouched. |
| `app/balance_dashboard.rs` | Both sides ported the same `first_grouped_quote_price` from the web's TypeScript, independently, with identical semantics. main's copy carries `#[must_use]` and sits beside the #188 fields. 041's only change to the file was this function. | **main's file, whole.** Nothing of 041's is lost: the function is there, and 041's four tests for it merge cleanly and pass against main's copy. |
| `.specify/feature.json` | Each side points at its own spec. | 041 for the merge commit; the speckit step then moved it to `specs/042-android-merge-main`. |

Merge commit: `6cd679a5`.

## Beyond the merge — what a pure merge would have left broken

Each item here is something the merged tree needed and the merge alone did
not do. The column on the right says whether it wires a main behaviour onto
Android or only keeps a gate green.

| # | Change | Why | Kind |
| --- | --- | --- | --- |
| 1 | Four Kotlin unit tests (`FlowLiveTest`, `WalletLiveTest`, `ContactsLiveTest`, `SettingsLiveTest`) read locale catalogs from `assets/i18n` | main (039) moved `public/i18n` there; the tests would have read a directory that no longer exists. No other Android source or build file named the old path — the Gradle wiring had auto-merged onto the new one. | gate |
| 2 | `cargo fmt` on `vela-core-uniffi/src/lib.rs` and the balance test file | 041 left five format diffs; the Rust gate's first step is `cargo fmt --check`. | gate |
| 3 | The web wasm rebuilt (`rust/pkg-web`, `assets/wasm/vela_core_bg.<fp>.wasm`) | The `build-web.mjs --check` fingerprint hashes **every `.rs` under `rust/crates`** — tests and the uniffi crate included — so 041's Rust edits had already moved it, and `cargo fmt` moved it again. Rebuilt last, after every Rust edit. | gate |
| 4 | `NetWizardErrorKind.CheckFailed(chain_id)` in `NetWire.kt` | main added the variant. The drift gate checks this family **exhaustively**, and for a reason the gate's own comment gives: an unknown discriminator on a sealed class is not a wrong message, it is a thrown exception in the add-network wizard. No Kotlin `when` consumes the kind, so no rendering site changed. | main behaviour → Android |
| 5 | `BalanceView.unreachable` mirrored; `WalletLive.home` renders it as a skeleton with a reason | main's #188 / spec 038 finding 15. The core sets `display_total_usd = 0.0` in this state — not null — so without the flag the Android hero showed a settled-looking `$0.00` over an unreadable chain. The reason string is the one the web and desktop heroes already bind (`onboarding.common.networkBody`), so no corpus change. New test: *an unreachable first load shows no number*. | main behaviour → Android |
| 6 | `wrapped_native_is_the_native` exported through uniffi; the balance walk asks it before adding the wrapped slot | spec 038, the founder's Celo report. Android's walk *did* list a wrapped-native slot for every chain that names one, so a Celo account would have shown CELO and WCELO for one holding and counted it twice. The chain list stays the core's. New test: `WrappedNativeTest` (GoldToken true in both cases; WETH/WBNB false). | main behaviour → Android |
| 7 | Kotlin bindings regenerated from the merged core | Gitignored, consumed in place; stale bindings would have compiled against 041's core. | gate |
| 8 | Spec 041's "spec 042" pointer for send/sign/browser/explore annotated | FR-009: two documents must not disagree about what 042 is. | record |

**Not changed, on purpose.** The held figure during refresh and the switcher's
row for the active account (both #188/038) live entirely in the core and reach
Android by the merge alone; the contacts import/export events and A–Z sections
(028) arrive in the core and the TS mirrors, and the Android contacts mirrors
remain a valid subset — wiring those screens is not this feature's business.
The desktop, iOS and web trees are untouched beyond the rebuilt artefact.

## Gates

Run locally as CI runs them, on the final tree.

| Gate | Command | Result |
| --- | --- | --- |
| Expo residue | `node scripts/check-expo-residue.mjs --self-test && …` | exit 0 |
| Native reachability | `node scripts/check-native-reachability.mjs` | every family reachable |
| Format | `cargo fmt --all --check` | clean |
| Clippy | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| Tests | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 0 failed (every suite `ok`) |
| Web artefact | `node rust/scripts/build-web.mjs --check` | fingerprint matches (`b4dd5a57ceeb`) |
| Onboarding wire types | `node rust/scripts/gen-onboarding-types.mjs --check` | current |
| Kotlin bindings | `cargo build --release -p vela-core-uniffi` + `uniffi-bindgen … --language kotlin` | generated |
| Android | `./gradlew :app:assembleDebug :app:testDebugUnitTest -PvelaSkipRustBuild` | BUILD SUCCESSFUL; 369 unit tests, 0 failures, 0 skipped (run under the Android Studio JBR — the shell's default JDK has no `jlink`, and a daemon started under it must be `--stop`ped first) |

Not run here: the `web` job (`pnpm build`, main's concern, unchanged beyond
the artefact), `rust-macos` (Swift bindings, no Swift-facing change), the
desktop and iOS jobs (their trees are untouched).

## Owed

- **Device pass (SC-004) — partly done, 2026-09-12 08:36, Xiaomi `alioth`.**
  The debug APK with the merged core (full NDK build; the `-PvelaSkipRustBuild`
  APK carried a stale `.so` without `wrapped_native_is_the_native`) installed
  and opened cold: own account `0x7687…D141`, total **£3.73 · GBP**, holdings
  ETH/Arbitrum 0.002 (£3.71) and POL/Polygon 0.152784 (£0.01), four tabs.
  探索 opens as a section (favourites + recents, the E2 fixture), Back returns
  to the wallet; 通讯录 opens this device's book — the rows Alice/Bob/Carol are
  real stored rows written by `ContactsPersistenceTest` in 040, confirmed by
  reading the DataStore file with `run-as`, not a fixture leak. No exception
  in logcat. **Not driven**: the held figure during refresh and the
  unreachable skeleton (both need a network fault staged on the device).
- **The wizard's `check_failed` has no Android rendering site** because no
  `when` over `NetWizardErrorKind` exists yet; it decodes and the wizard's
  generic error line shows. A per-kind message is 043 material if the
  founder wants one.
- **028's contacts import/export and A–Z sections** exist in the core and are
  not on any Android screen. Recorded, not scheduled.
