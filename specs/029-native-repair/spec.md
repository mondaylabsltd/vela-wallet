# Feature Specification: The Drawn Screens Become Reachable, and CI Learns to Notice

**Feature Branch**: `029-native-repair`

**Created**: 2026-09-04

**Status**: Draft

**Input**: Founder description: "让 spec 022 画的 explore 与 signing 界面在 desktop/Android/iOS 三端真正可达，并补上三端缺失的 CI 闸门与结构性守卫，使同类死代码不能再次发生。"

## Why

`specs/022-explore-signing-ui/results.md` records that the browser and the signing
sheet are **"Wired, not gallery-only"** and reachable "in the real app" on all three
native clients. That is the repo's record of what shipped. It is not what shipped.

| Platform | What the tree actually contains |
|---|---|
| desktop | `src/main.rs` declares 22 modules; `src/` holds 24 directories. `diff` of the two lists is exactly two lines: `explore` and `signing`. **3,571 lines never reach the compiler**, and the five `#[test]` functions inside them have never run. The sidebar's 探索 row is built with `None` as its destination (`wallet/page.rs:565`) — a painted icon that cannot be clicked to anywhere. |
| Android | `VelaDestinations` declares ten routes; none is `EXPLORE`. **4,353 lines** compile and are referenced only by each other and by their own fixture tests. No `@Preview`, so they are not even reviewable in Android Studio. |
| iOS | `PageOverride.Page` has seven cases (`RootView.swift:384`); neither is explore or signing. **1,771 lines** compile; `ExploreScreen(` is never instantiated anywhere in the target. |

**9,695 lines of drawn, translated, fixture-complete UI that no person can open.**

The wiring is not lost. It exists in commit `969bf8fc` on the unmerged branch
`020-intro-carousel`. What reached `main` is `feecb6e4`, a rebase of the same work
whose commit message explains the loss: a second session was mid-flight in the same
i18n corpus files, so the corpus half was kept and the four wiring files were dropped.
The eight source directories are **byte-identical** between `main` and that branch —
only the wiring is missing.

### Why this is its own feature and not a line in the next one

1. **The signing sheet is a hard prerequisite for the money tier.** Specs 032, 035
   and 038 each need a mount point for a signing surface before they can put a live
   `sign_request` behind it. Wiring the mount point and the live data in one change
   is how a money spec goes over review budget.
2. **Doing it per-platform leaves the false record standing.** Folding the repair
   into each platform's first wiring cut means Android and iOS carry unreachable
   screens — and `022/results.md` keeps asserting otherwise — for two more specs.
3. **The CI decision is made once here, or three times later.** Folding means the
   platform with the most expensive runner and the highest chance of a repeat gets
   its guard last.
4. **It is the only feature in the 029–038 program that is not platform-serial.**

### Why the guards matter more than the fix

Nothing in this repository could have caught any of it. `.github/workflows/ci.yml`
has five jobs and **zero** occurrences of `gradlew` or `xcodebuild`; its only
`app-desktop` reference runs `check-windows.sh`, which type-checks a *different*,
C-free crate standalone. The desktop app crate is not a member of the `rust/`
workspace, so `cargo test --workspace` cannot see it either.

And this failure mode has already happened once before, in the same corner. From
`check-windows.sh`'s own header:

> It also checks this crate STANDALONE, which is the point (it runs anywhere) and
> also its blind spot: it cannot see how the desktop app depends on it. The app once
> declared this crate under `[target.'cfg(target_os = "macos")'.dependencies]` and
> left the Windows path unlinked, **with this gate green throughout**.

A missing link declaration, a green gate throughout. That is this bug, twice. So the
deliverable is not "add two `mod` lines" — it is **a guard that fails on today's
tree**, and then the fix that turns it green.

## Design Authority

`design/` plus `specs/022-explore-signing-ui/` remain the visual authority; **this
feature draws nothing new**. Every pixel already exists in the three clients'
`explore/` and `signing/` source. The authority for the *wiring* is commit
`969bf8fc`, used as a specification to hand-re-apply from — never as a patch to
apply, because its hunks predate specs 021 and 023 and no longer fit.

## Out of Scope

- **Live business state.** Explore and signing stay fixture-driven exactly as 022
  drew them. Wiring them to `browser_history` / `dapp_session` / `sign_request` /
  `clear_signing` belongs to the money tier (032/035/038) and to spec 027's dApp
  pairing work.
- **The spec-020 intro carousel.** Its three native implementations are stranded on
  the same branch, but recovering them is a UI backfill unrelated to reachability,
  and the branch is 26,229 deletions away from `main`. Founder decision, recorded:
  excluded from the 029–038 program, to be its own later spec.
- **Any change under `rust/`.** No core, no bindings, no corpus. See FR-007.

## User Scenarios & Testing

### User Story 1 - The guards fail on today's tree (Priority: P1)

A reviewer runs the new checks against `main` untouched. Each one fails, naming the
exact screens nothing routes to.

**Why this priority**: A guard added after the fix proves nothing — it has never
been observed to fail. Landing the guards first, red, is the only way to know they
bite. This story alone is a viable deliverable: it converts an invisible defect into
a visible one on three platforms.

**Independent Test**: On an unmodified checkout, run the reachability guard. It
exits non-zero and names the orphans on all three platforms.

**One guard, not three.** The three platforms fail differently — an undeclared
module, a missing route constant, a missing enum case — but they fail *identically*
in the only way that matters: a screen family exists and its navigation root does not
name it. One instrument that reads all three navigation roots is therefore the honest
shape, and it has a decisive practical advantage: it needs **no toolchain**, so it
runs in the `app` job in under a second rather than behind a 25-minute native build.
A guard that is expensive to run is a guard somebody eventually skips — which is the
failure mode it exists to prevent. The per-platform build jobs (US3) catch a
different class: code that is routed but does not compile.

**Acceptance Scenarios**:

1. **Given** an unmodified `main`, **When** the guard runs, **Then** it exits
   non-zero and reports, for desktop, `explore` and `signing` as present on disk but
   absent from `main.rs`'s `mod` list.
2. **Given** an unmodified `main`, **When** the guard runs, **Then** it reports
   `explore` and `signing` as Android feature packages no `VelaNavHost` destination
   reaches.
3. **Given** an unmodified `main`, **When** the guard runs, **Then** it reports
   `Explore` and `Signing` as iOS `Features/` folders `RootView.swift` never
   instantiates.
4. **Given** the guard, **When** its exemption list is read, **Then** it is empty —
   and any future entry names both the screen and the reason it has no route.

---

### User Story 2 - A person can open the browser and the signing sheet (Priority: P1)

Somebody signed in on any of the three clients reaches Explore from the place the
design puts it, and reaches the signing sheet from Explore. The screens render the
fixture states 022 drew, and the guards from US1 go green.

**Why this priority**: It is the feature. It also unblocks three later specs.

**Independent Test**: Sign in on each client, click the 探索 entry, confirm the
browser renders; open a signing request from it, confirm the sheet renders.

**Acceptance Scenarios**:

1. **Given** a signed-in desktop wallet, **When** the sidebar's 探索 row is clicked,
   **Then** the Explore section renders — and the row is no longer built with a
   `None` destination.
2. **Given** a signed-in Android wallet, **When** the Explore tab is selected,
   **Then** `ExploreScreen` renders, and the Settings tab still opens Settings
   rather than signing out (the regression `VelaNavHost.kt:270-278` warns about).
3. **Given** a signed-in iOS wallet, **When** Explore is selected, **Then**
   `ExploreScreen` renders.
4. **Given** any client, **When** the signing sheet is opened from Explore, **Then**
   it renders 022's fixture states.
5. **Given** the desktop tree after wiring, **When** `cargo test` runs, **Then** the
   five previously-dead `#[test]` functions execute, and the total is strictly
   greater than the recorded baseline of 100.
6. **Given** any client, **When** every gallery state is swept, **Then** it renders
   exactly as before — this feature adds routes, it does not change pixels.

---

### User Story 3 - The three clients are built by CI (Priority: P2)

A pull request that breaks the desktop app, the Android app, or the iOS app is red
before a human looks at it.

**Why this priority**: Without it, US1's guards are one `git rebase` from being
dropped again — which is precisely the history that produced this feature. It is P2
only because US1+US2 deliver the user-visible value; this delivers the durability.

**Independent Test**: Push a branch that deletes a `mod` line, or renames a route
constant. CI is red.

**Acceptance Scenarios**:

1. **Given** a PR touching `app-desktop/**` or `rust/crates/vela-core/**`, **When**
   CI runs, **Then** a desktop job runs `fmt`, `clippy -D warnings` and the test
   suite, and fails on any of them.
2. **Given** a PR touching `app-android/**`, **When** CI runs, **Then** a Gradle job
   assembles the debug variant and runs the JVM unit tests.
3. **Given** a PR touching `app-ios/**`, **When** CI runs, **Then** an xcodebuild
   job builds and tests against a simulator destination.
4. **Given** the repository as it stands, **When** the iOS job is added, **Then** a
   shared `VelaWallet.xcscheme` is committed alongside it — because
   `xcshareddata/xcschemes/` does not exist today and `xcodebuild -scheme VelaWallet`
   cannot resolve.

---

### User Story 4 - The record is corrected (Priority: P3)

`specs/022-explore-signing-ui/results.md` stops claiming something untrue.

**Why this priority**: Low urgency, non-negotiable in kind. A results file that
misreports what shipped is worse than no results file, because the next person plans
against it — which is exactly what happened here.

**Independent Test**: Read the amended file; it states what 022 delivered (the
screens) and what it did not (their routes), with a pointer to this feature.

**Acceptance Scenarios**:

1. **Given** 022's results.md, **When** amended, **Then** the "Reachable in the real
   app" row reads false for all three natives, with the cause and this feature named.

## Requirements

- **FR-001 (The guards land first, and fail)**: The module-graph check and the two
  nav-coverage tests MUST be committed in a state where they fail against the
  unmodified tree, in a commit that precedes the wiring commit. Their failure output
  MUST name the unreachable screens.
- **FR-002 (Reachability, all three)**: Explore MUST be reachable by a signed-in
  person on desktop, Android and iOS through the entry point `design/` places it at;
  the signing sheet MUST be reachable from Explore. No screen may be reachable only
  through a gallery or a developer environment variable.
- **FR-003 (Fixtures unchanged)**: No `fixtures.*` file may lose or alter a
  constant. This feature adds routes and module declarations; it does not change a
  rendered value. Every gallery state MUST render exactly as it does today.
- **FR-004 (Recovery by re-application, never by merge or patch)**: The wiring MUST
  be hand-re-applied using `969bf8fc` as a specification. `git merge`, `git
  cherry-pick` and `git apply` of that commit are forbidden — measured cause: the
  branch's diff against `main` over the three app directories is 3,775 insertions
  against **26,229 deletions**, and `git apply --check --3way` of the desktop hunk
  reports conflicts.
- **FR-005 (Dead tests must run)**: After wiring, every previously-unreachable
  `#[test]` MUST execute in its platform's suite. **This bites on desktop only**, and
  the asymmetry is the defect's own shape: on Android and iOS the explore/signing
  sources always compiled — they were merely unrouted — so their fixture tests were
  running all along. Only rustc skips a directory nobody declared. The pin is the
  **runtime** count, not a `grep` of the source: on desktop those differ by seven and
  the difference *is* the defect stated arithmetically — 100 `#[test]` exist, 5 are in
  the unreachable modules and never compile, 2 are `#[cfg(target_os = "linux")]`,
  leaving **88 passed / 5 ignored / 93 compiled** on macOS. Wiring must take that to
  93 / 5 / 98. Android and iOS must simply not regress.

- **FR-006 (CI covers the three clients)**: `.github/workflows/ci.yml` MUST gain a
  desktop job, a Gradle job and an xcodebuild job, each path-gated and always-on for
  `main`. Where a job cannot cover something (the desktop gallery sweep needs a
  display), the job's own comment MUST say so, so no reader assumes coverage that
  does not exist.
- **FR-007 (Nothing under `rust/`)**: No file under `rust/` may change. In
  particular the i18n corpus MUST NOT be regenerated while spec 026 is open —
  a corpus collision between two in-flight sessions is the documented root cause of
  the defect this feature repairs, and repeating it would be the same mistake twice.
- **FR-008 (No new business logic)**: Explore and signing remain fixture-driven.
  This feature connects no core and adds no rule on any platform.
- **FR-009 (The record is corrected)**: `specs/022-explore-signing-ui/results.md`
  MUST be amended to state what was and was not delivered.

## Success Criteria

- **SC-001**: On the unmodified tree, all three guards fail and name `explore` and
  `signing`; on the finished tree, all three pass. Both runs recorded in results.md.
- **SC-002**: A signed-in person opens Explore and then the signing sheet on all
  three clients. Evidenced by one screenshot per client per screen.
- **SC-003**: On desktop the five previously-dead `#[test]` functions appear **by
  name** in the run output and the result line reads `93 passed; 0 failed; 5
  ignored` against the recorded baseline of `88 passed; 0 failed; 5 ignored`.
  Android and iOS do not regress. (They cannot increase — see FR-005.)
- **SC-004**: Every gallery state renders unchanged; the cumulative diff of every
  `fixtures.*` file across the branch is empty.
- **SC-005**: A PR that removes any one of the three wirings is red in CI. Proven by
  pushing such a branch and recording the failure, not by assertion.
- **SC-006**: Zero files changed under `rust/`; zero corpus delta.
- **SC-007**: The eight explore/signing source directories are byte-identical to
  `main` at branch point — the wiring changed, the screens did not. Measured by
  `git diff --stat` over those paths at close.
