# Results — 029 native-repair

Delivery ledger in the 019 format: baselines recorded **before** any change (the
question "did this grow?" has no answer after the fact), then one section per
phase, then the SC verdict table, consolidated deviations, and carried debts.

## Baselines — recorded 2026-09-04, branch point `f9bcb278` (`origin/main`)

### The defect, measured

| Platform | Unreachable source | Dead tests | Evidence |
|---|---|---|---|
| desktop | **3,571 lines** (`src/explore/` 925 + `src/signing/` 2,646) | 5 `#[test]`, never compiled | `main.rs` declares 22 mods; `src/` holds 24 dirs; the `diff` is exactly `explore`, `signing` |
| Android | **4,353 lines** (`feature/explore/` + `feature/signing/`) | 0 (fixture tests only, which do run) | `VelaDestinations` declares 10 routes; none is `EXPLORE` |
| iOS | **1,771 lines** (`Features/Explore/` + `Features/Signing/`) | 0 (fixture tests only, which do run) | `PageOverride.Page` has 7 cases (`RootView.swift:384`); `ExploreScreen(` never instantiated |
| **total** | **9,695 lines** | | |

### Test baselines

| Platform | Static count | Runtime baseline | Reconciliation |
|---|---|---|---|
| desktop | 100 `#[test]` | **88 passed · 0 failed · 5 ignored** (93 compiled, macOS, 2.44s) | 100 − 5 unreachable − 2 `#[cfg(target_os = "linux")]` in `executor/proxy.rs:308,317` = 93 |
| Android | 99 `@Test` | *(pending first CI run)* | |
| iOS | 130 `@Test` (Swift Testing macro, not `func test`) | *(pending first CI run)* | |

The 5 desktop `#[ignore]`d tests need hardware or the live registry:
`a_plugged_in_key_answers_get_info`, `the_deployed_registry_answers_its_health_probe`,
`an_unknown_public_key_is_simply_unregistered`, `register_then_assert`,
`excluded_credential_is_refused`.

### Recovery source, verified

| Check | Result |
|---|---|
| `969bf8fc` exists and carries the wiring | ✅ its `main.rs` declares `mod explore; mod signing; mod intro;` |
| The 8 explore/signing source dirs vs `main` | ✅ **byte-identical** — `git diff --stat main 020-intro-carousel -- <dir>` is empty for each |
| `git merge` is safe? | ❌ **no** — `git diff --stat main 020-intro-carousel -- app-desktop app-android app-ios` = 97 files, 3,775 ins / **26,229 del** (branch predates 021/023; a merge deletes iOS Flows and Settings) |
| `git apply --3way` is safe? | ❌ **no** — `git show 969bf8fc -- .../wallet/page.rs \| git apply --check --3way -` → *"Applied patch … with conflicts"* (`page.rs` was 2,082 lines then, 3,746 now) |
| Therefore | hand re-apply, using `969bf8fc` as a **specification** (FR-004) |

### CI baseline

`.github/workflows/ci.yml`: 5 jobs (`app`, `web`, `site`, `rust`, `rust-macos`).
**0 occurrences of `gradlew`. 0 of `xcodebuild`.** The only `app-desktop` reference
is line 241, `check-windows.sh`, which type-checks the C-free `vela-passkey-win`
crate *standalone* — its own header documents having missed exactly this class of
bug once before ("left the Windows path unlinked, with this gate green throughout").
The desktop app crate is not a `rust/` workspace member, so `cargo test --workspace`
cannot reach it either.

Blockers found for the CI work itself:
- **iOS has no shared scheme.** `app-ios/VelaWallet/VelaWallet.xcodeproj/xcshareddata/xcschemes/`
  does not exist, so `xcodebuild -scheme VelaWallet` cannot resolve today.
- **gpui is an unpinned git dependency** (`gpui = { git = ".../zed" }`, no `rev`/`tag`).
  Reproducible only via the committed `Cargo.lock`; the first `cargo update` moves
  desktop to an arbitrary Zed commit.

## Phase 1 — the guards, red (in progress)

`scripts/check-native-reachability.mjs` — static, no toolchain, runs in the existing
`app` job in well under a second. Against the unmodified tree it exits 1 and reports:

```
desktop: 2 module(s) on disk that src/main.rs never declares, so rustc never
         compiles them: explore, signing
android: 2 feature package(s) no VelaNavHost destination reaches: explore, signing
ios: 2 Features/ folder(s) RootView.swift never instantiates: Explore, Signing
```

**`EXEMPT` ships empty, and that is a finding.** Every candidate exemption was
checked and turned out to be genuinely reachable — desktop `ui`/`ctap`/`executor`
are all declared in `main.rs`; iOS `Gallery` is instantiated at `RootView.swift:70`.
With zero exemptions the guard reports exactly the two real orphans per platform and
no false positives. A guard that ships pre-populated with exemptions nobody needs is
a guard the next orphan hides behind.

## Phase 2 — desktop wired

`main.rs` +7 lines (portable verbatim from `969bf8fc`), `wallet/page.rs` (14 hunks
hand re-applied), `icons.rs` (+9 variants), and the 379-line explore/signing render
body appended into `impl WalletPage`.

| Gate | Result |
|---|---|
| `cargo build` | ✅ |
| `cargo test` | ✅ **93 passed · 0 failed · 5 ignored** (baseline 88/0/5) |
| `cargo fmt --all --check` | ✅ clean |
| clippy, files this phase touched | ✅ 0 warnings in `wallet/`, `explore/`, `signing/` |
| `scripts/check-native-reachability.mjs` | ✅ desktop no longer listed (Android + iOS still red — phases 3 and 4) |
| `VELA_PAGE=explore` | ✅ window opens, log reads `section Explore`, survives 6s; screenshot on file |
| FR-003 fixtures | ✅ **every string literal byte-identical** in both `fixtures.rs` (only rustfmt reflow) |

### The five tests that had never run

```
explore::tests::explore_strings_resolve_without_echo ... ok
signing::tests::signing_strings_resolve_without_echo ... ok
signing::tests::fill_replaces_named_vars ... ok
signing::fixtures::tests::every_scenario_builds ... ok
signing::fixtures::tests::unlimited_approval_cannot_be_confirmed_as_requested ... ok
```

The last one is one of the two product contracts 022's own commit message says the
desktop asserts — *"an unlimited approval can never be signed as requested"*. It has
been asserting nothing for two days.

### Three tools were blind, not one

Worth recording because it widens the lesson. The baseline `cargo fmt --all --check`
was **clean**, and the moment `mod explore; mod signing;` landed it reported diffs in
four files. rustfmt walks the module tree, so an undeclared directory is invisible to
it for exactly the same reason it is invisible to rustc and to the test harness.
Compiler, formatter and test runner all silently skipped 3,571 lines. A reachability
check is not a nicety here; it is the only instrument that could see this.

### Deviations from `969bf8fc` (the drift FR-004 predicted)

1. **`ExternalLink` already exists.** The commit adds ten `Icon` variants; one of
   them landed on `main` since. Nine added, no duplicate.
2. **`Identity::display()` now returns `SharedString`**, not `String`. The branch's
   `.child(SharedString::from(identity.display()))` is a useless conversion today and
   fails `clippy -D warnings`. Rewritten to `.child(identity.display())`.
3. **`Section` has a third variant now.** `Section::Wallet => GalleryTab::D1` became
   `Section::Wallet | Section::Explore => GalleryTab::D1`, and the match also had to
   keep covering `Section::Settings`, which did not exist on the branch.
4. **Insertion point moved.** The 379-line body went in before `fn wallet_columns`,
   which is at line 3,449 today against 1,886 then.

Each is a place `git apply` would have produced a conflict or a silent wrong result.

### Discovered defect — handed off, not fixed here

With `VELA_LANG=en` the Explore screen renders **13 hardcoded CJK string literals**
(11 in `explore/fixtures.rs`, 2 in `signing/fixtures.rs`): category titles 交易 /
预测市场, relative timestamps 刚刚 / 昨天, subtitles 稳定币兑换 / 永续合约交易.
These are wallet chrome, not site content, so 022's "the stand-in page's words are
the site's" rule does not cover them — they should resolve from the corpus.

**Not fixed in this feature, deliberately.** The fix needs corpus keys, and FR-007
forbids regenerating the corpus while 026 is open — a corpus collision between two
in-flight sessions is the documented root cause of the very loss this feature is
repairing. Doing it twice would be the joke writing itself. Carried to the handoff.

### Recorded debt — the desktop crate has never been clippy-clean

`cargo clippy --all-targets -- -D warnings` fails on the **baseline** with 10
warnings this feature did not introduce: 7 in `src/ctap/cable.rs`, 1 in
`src/ctap/cable/l2cap.rs` (`Arc` that is not `Send`/`Sync`), 1 in `src/hardware.rs`,
1 in `src/onboarding.rs` (both "very complex type"). Measured by stashing this
feature's changes and re-running.

Nothing here ever ran clippy, so nothing here was ever clean. The CI job in the next
phase has to decide between gating at `-D warnings` (which means fixing 10 unrelated
warnings in caBLE/Noise transport code) and gating lower. That decision is recorded
in the phase that makes it, not smuggled into this one.

## Phase 3 — Android wired

`VelaNavHost.kt` only: the `EXPLORE` route constant, `ALL`, `DEVELOPER_ROUTES`, a
direct `composable` for review, and — the substance — a `section` switch inside the
signed-in `WALLET` route so 探索 selects a body rather than doing nothing.

| Gate | Result |
|---|---|
| `./gradlew :app:compileDebugKotlin -PvelaSkipRustBuild` | ✅ (only two pre-existing `LocalClipboardManager` deprecations) |
| `./gradlew :app:testDebugUnitTest` | ✅ **118 run · 0 failed · 0 skipped** |
| `DeveloperRoutesTest` with `EXPLORE` added | ✅ still green |
| `check-native-reachability.mjs` | ✅ Android no longer listed |

Re-expressed against today's shell rather than the branch's, which is why FR-004
forbids patching. Three collisions:

1. The branch's `WalletScreen(model, onSelectTab)` predates spec 021's
   `FlowHost` / `rememberFlowNavState`, so the section switch had to be composed
   *around* the flow stack rather than replacing the call.
2. **The branch routes Settings to `session.signOut()`.** Spec 023 fixed exactly that
   — `VelaNavHost.kt:270-278` still carries the warning ("tapping 设置 to change your
   language logged you out instead"). Applying the hunk as written would have
   reintroduced a shipped regression. It routes to `VelaDestinations.SETTINGS`.
3. Back had to be taught the new state: it unwinds the flow stack first, then leaves
   探索 for 钱包. Backing out of a browser should land on the wallet, not on Welcome.

**Test totals do not move on Android, and that is correct.** Its explore/signing
sources always compiled — they were merely unrouted — so their fixture tests were
running all along. Only rustc skips a directory nobody declared, which is why the
five newly-live tests are a desktop-only phenomenon. FR-005 was corrected to say so
rather than asserting a uniform increase it cannot get.

### A fresh checkout cannot build Android or iOS

Recorded because it is a hard input to the CI design, and it surprised me:
`rust/bindings/kotlin/` (consumed in place as a `kotlin.srcDir`),
`app-android/.../jniLibs/` and `app-ios/VelaCoreKit/Artifacts/` are **all
gitignored**. Building Android here needed `cargo build --release -p
vela-core-uniffi` plus a `uniffi-bindgen` run first (2m24s cold). Any CI job has to
do the same, and `xcodebuild -list` fails outright without the xcframework — not with
a missing-scheme error, but with *"local binary target 'VelaCoreFFI' … does not
contain a binary artifact"*.

## Phase 4 — iOS wired, and the three CI jobs

`RootView.swift` only, plus the shared scheme the project never had.

| Gate | Result |
|---|---|
| `xcodebuild -list` | ✅ resolves `VelaWallet` — it could not before (see below) |
| `xcodebuild test` (iPhone 16 / iOS 18.2) | ✅ **130 tests in 13 suites passed**, 0 failures |
| `xcodebuild test` (iPhone 17 Pro / iOS 26.2, the CI resolver's pick) | ✅ same 130 / 13 / 0 |
| Generated Swift bindings drift | ✅ none — `build-ios-xcframework.sh` refreshed them to byte-identical |
| `check-native-reachability.mjs` | ✅ **green on all three platforms** |

### The scheme the project never had

`xcshareddata/xcschemes/` did not exist. Xcode autocreates a per-user scheme under
`xcuserdata/` the first time anybody opens the project, which is exactly why nobody
noticed: it works on every machine that has ever opened the project and on no machine
that has not. A shared `VelaWallet.xcscheme` is now committed, with
`VelaWalletUITests` skipped — `ScreenshotSweepTests` launches the app once per gallery
fixture to pull images out of the `.xcresult`, which is a review instrument for a
person, not a gate.

### Two destination traps, both hit, both now designed out

1. `-destination 'platform=iOS Simulator,name=iPhone 16'` is **ambiguous** the moment
   a machine has that device under two runtimes; xcodebuild answers by printing every
   simulator it knows and failing.
2. `OS=latest` does **not** mean "the newest runtime that has this device" — it means
   the newest runtime, which here (26.2) offers no iPhone 16 at all.

The job therefore *resolves* a UDID from `simctl list devices available -j` rather
than spelling a destination. Verified by running the resolver's own pick.

### A reporting trap worth writing down

`xcodebuild` prints **`Executed 0 tests, with 0 failures`** and then
`** TEST SUCCEEDED **`. That zero is not a failure and not a skip: `VelaWalletTests`
uses Swift Testing (`@Test`), which reports on its own line —
`✔ Test run with 130 tests in 13 suites passed`. A CI job (or a person) grepping for
the XCTest line would read a green suite of 130 as an empty one. The job comment says
so at the point of use.

### The CI jobs (FR-006)

| Job | Runner | Covers |
|---|---|---|
| `desktop` | ubuntu-24.04 | `fmt --check`, `clippy --all-targets`, `cargo test` |
| `android` | ubuntu-24.04 | generate Kotlin bindings → `assembleDebug` + `testDebugUnitTest` |
| `ios` | macos-15 | build xcframework (cached) → `xcodebuild test` on a resolved simulator |

Three things stated in the jobs' own comments rather than left for a reader to assume:

- **`sweep-gallery.sh` is not in the desktop job.** It opens real windows; a runner has
  no display. It stays a local gate, and saying so stops the next person assuming a
  coverage that does not exist.
- **Clippy is `--all-targets`, not `-D warnings`.** The baseline carries ten warnings
  in caBLE/Noise transport code because nothing ever ran clippy here. Fixing transport
  code is not this feature's business (AI-CODING-RULES §2), and a gate that starts red
  is a gate somebody disables. Raising it is a named task for whoever clears the ten.
- **Both native jobs must generate their bindings first.** `rust/bindings/kotlin/`
  and `VelaCoreKit/Artifacts/` are gitignored, so a fresh checkout cannot build either
  app. The iOS xcframework is cached on `hashFiles('rust/crates/**', 'rust/Cargo.lock')`
  because macos-15 minutes bill at ten times linux — that cache is the difference
  between a ~6-minute job and a ~25-minute one.

## Phase 5 — the guard was wrong, and proving SC-005 is what found it

SC-005 says a regression must be *proven* red, "not by assertion". Doing that
honestly broke the guard twice, and both breaks were real.

**First probe — my probe was wrong, not the guard.** Deleting Android's
`const val EXPLORE` left the guard green, which looked like a blind spot. It is not:
removing a route constant breaks *compilation*, not reachability, and the build jobs
own that. The two instruments cover different classes, which is the argument for
having both.

**Second probe — a genuine blind spot.** Deleting the `ExploreScreen(` *call* left
the guard green, because the surviving `import ...feature.explore.ExploreScreen` line
matched the package check. Kotlin warns about an unused import; it does not error. So
a dead screen looked reachable, and the guard would have re-certified the exact defect
this feature exists to repair. Imports are now stripped before anything is matched.

**Stripping imports then produced false positives** — `contacts`, `settings` and
`signing` all flagged, and all three are reachable. The heuristic was wrong at the
root: it looked for a screen whose *filename* matched, one hop from the navigation
root. But contacts renders through `ContactsRoute`, not `ContactsScreen`, and
**`signing` is never rendered by the navigation root at all — the browser raises it.**

Reachability is a graph, not a grep. The check is now a transitive walk: collect what
each family declares (`fun X(` on Kotlin, `struct X: View` on Swift), start from the
navigation root, and keep following into families already reached. A signing sheet
reachable only through Explore is correctly reachable; if Explore's call is deleted,
**both** explore and signing go red, which is the truth.

| Direction | Result |
|---|---|
| New guard vs. the original broken tree (`f9bcb278`) | ✅ finds all 6 orphans across 3 platforms |
| New guard vs. the fixed tree | ✅ green, **zero false positives** |
| Delete desktop `mod explore;` | ✅ red — `explore` |
| Delete Android's `ExploreScreen(` call | ✅ red — `explore, signing` (transitivity working) |
| Delete iOS's `ExploreScreen(` call | ✅ red — `Explore` |

The lesson is not about this script. A guard is a claim, and a claim nobody has seen
fail is a claim nobody has tested. Two of the three checks in the first version were
wrong, and the only reason that is known is that SC-005 refused to accept an
assertion.

## Closeout — SC verdicts

| SC | Claim | Verdict |
|---|---|---|
| **SC-001** | Guards fail on the unmodified tree, pass on the finished one | ✅ New guard vs. `f9bcb278`: all 6 orphans on 3 platforms. Vs. HEAD: green, zero false positives. |
| **SC-002** | A person opens Explore, then the signing sheet, on all three | ✅ desktop `VELA_PAGE=explore` opens (screenshot); Android 探索 tab switches the body; iOS `selectTab` → `.explore`. Signing reachable from Explore on all three. |
| **SC-003** | Desktop 88→93, the five dead tests named; Android/iOS do not regress | ✅ `93 passed; 0 failed; 5 ignored`, all five named. Android 118/0. iOS 130 in 13 suites/0, on two runtimes. |
| **SC-004** | Galleries unchanged; every `fixtures.*` literal identical | ✅ Both desktop fixtures: **every string literal byte-identical**. Android/iOS fixtures untouched entirely. |
| **SC-005** | A PR removing any wiring is red — proven, not asserted | ✅ Proven for all three. **The proof broke the guard twice** (§Phase 5); both defects were real and are fixed. |
| **SC-006** | Zero files under `rust/`; zero corpus delta | ✅ `git diff f9bcb278..HEAD -- rust/` is empty. Corpus untouched — FR-007 held while 026 is open. |
| **SC-007** | The 8 explore/signing source dirs unchanged | ✅ Android (2) and iOS (2) byte-identical. Desktop (2) carry rustfmt reflow **only** — every literal proven identical — and that reflow is itself evidence: rustfmt could not see the files before either. |

### Final gate, all three platforms

```
desktop   cargo fmt --check ✅   cargo test: 93 passed, 0 failed, 5 ignored
android   testDebugUnitTest: 118 tests, 0 failed
ios       xcodebuild test: 130 tests in 13 suites passed  (iOS 18.2 and 26.2)
guard     native reachability: every screen family is reachable
ci.yml    app, web, site, rust, rust-macos, desktop, android, ios
```

### Carried debts → handoff

1. **13 hardcoded CJK literals** in the newly-visible explore/signing fixtures (11
   desktop explore, 2 desktop signing): category titles 交易 / 预测市场, relative
   timestamps 刚刚 / 昨天, subtitles 稳定币兑换 / 永续合约交易. These are wallet
   chrome and should resolve from the corpus. **Blocked by FR-007 while 026 is open**
   — and doing a corpus change during another session's corpus change is the exact
   mistake that caused this feature. Take it up once 026 merges.
2. **The desktop crate has never been clippy-clean.** 10 baseline warnings (7
   `ctap/cable.rs`, 1 `cable/l2cap.rs` — an `Arc` that is not `Send`/`Sync` — 1
   `hardware.rs`, 1 `onboarding.rs`). The `desktop` CI job gates
   `clippy --all-targets` without `-D warnings` until they are cleared. Raising it is
   a named, self-contained task.
3. **`gpui` is an unpinned git dependency** (`git = ".../zed"`, no `rev`/`tag`).
   Reproducible today only via the committed `Cargo.lock`; the first `cargo update`
   moves the desktop to an arbitrary Zed commit. Pin a `rev`.
4. **No native job is in branch protection.** Following the file's own precedent for
   `rust-macos`, the three jobs are added but merge policy is not changed — that is a
   founder action.
5. **The desktop gallery sweep is not in CI** and cannot be: it opens real windows.
   It stays a local gate; the job comment says so.

### What this feature is really evidence for

Three tools were blind to the same 3,571 lines — rustc, rustfmt and the test
harness — and every gate in CI stayed green, because on all three platforms the
*compiler* was content. `check-windows.sh` had already recorded this exact shape once
("left the Windows path unlinked, with this gate green throughout"). The instrument
that sees it is not a stronger compiler or a stricter lint; it is a check that asks a
different question — **can a person get here?** — and is cheap enough that nobody
ever skips it.
