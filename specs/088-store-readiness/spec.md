# Feature Specification: 088 — Store submission readiness (Google Play testing + TestFlight)

**Feature Branch**: `088-store-readiness` | **Created**: 2026-10-01 | **Status**: Draft
**Input**: the owner submits the Android app to a Google Play testing track and the iOS app to TestFlight
tomorrow (2026-10-02). The builds must be accepted, installable, and safe, and the documents must be
complete and true. The research is in [research-requirements.md](research-requirements.md) (official
sources, read 2026-10-01). The audit of the native projects is in [audit.md](audit.md).

Program: 086 (issues) · 087 (mobile beta device pass) · **088 (this)** · 089 (Chrome extension pass).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — The owner uploads builds the stores accept (Priority: P1)

The owner runs the documented release steps and gets:
- a signed Android App Bundle that Play accepts on the internal testing track, with no 16 KB page-size
  rejection;
- an iOS archive that uploads to App Store Connect and processes for TestFlight.

**Independent Test**:
- Build the release AAB with the upload key from the documented place.
- `apksigner`/`jarsigner` verify it, and every 64-bit `.so` is 16 KB-aligned.
- An `xcodebuild archive` + export (app-store method) dry run succeeds with no entitlement mismatch.
- Build numbers are above any previously uploaded build.

**Acceptance Scenarios**:
1. **Given** the upload keystore is configured outside the repo, **When** the release bundle is built,
   **Then** it is signed with that key, and nothing secret is committed.
2. **Given** a 16 KB-page device or Play's checker, **When** the bundle is inspected, **Then** every native
   library is 16 KB-aligned, and the JNA version is one that runs on 16 KB pages.
3. **Given** the iOS archive, **When** it is exported for the App Store, **Then** no entitlement is refused.

---

### User Story 2 — Testers can create a wallet and sign on store-installed builds (Priority: P1)

A tester who installs from Play or TestFlight can create a wallet and sign. The owner has a clear
checklist for the one step only they can do: adding Play's app-signing certificate to `assetlinks.json`
and deploying the site.

**Independent Test**: a release-signed build on the Xiaomi creates a passkey through the release key's
asset links. The checklist names the exact file and line to add Play's key to.

---

### User Story 3 — Nothing a reviewer would flag (Priority: P1)

A reviewer, or a malicious link, cannot do any of the following:
- reach debug-only screens in a release build;
- load an arbitrary page into the wallet browser with the provider injected, without the person's
  confirmation;
- crash the app on Android 10/11 through background work.

The export-compliance statement and the deletion explanation are true.

**Acceptance Scenarios**:
1. **Given** a release build, **When** it is launched with a debug start-destination extra, **Then** the extra
   is ignored.
2. **Given** a `velawallet://open?url=…` link, **When** it is opened, **Then** the person sees the site's host
   and must confirm. Only `https` pages open, and the provider is offered only to secure origins.
3. **Given** Android 10–11, **When** background tracking starts, **Then** it runs without a foreground-info
   crash.
4. **Given** the erase screen, **When** it is read, **Then** it says what is deleted from the device, what the
   servers keep and for how long, and that the on-chain public-key record cannot be deleted by anyone.

---

### User Story 4 — The documents are complete and true (Priority: P1)

The owner has, ready to paste:
- review notes / App access instructions (no testnet that doesn't exist, the current tester count);
- listing copy that follows the claim ledger: no audit claim, no "your face is your key", no "no
  accounts" contradiction;
- the data-safety and privacy answers;
- a support page and a deletion page on getvela.app (built, not deployed);
- a one-page console checklist for both stores.

**Independent Test**: every claim in the listing and review notes is traced to code or to the privacy
evidence. The support and deletion pages render locally.

### Edge Cases

- Play's first internal-track release is reviewed (hours to 7 days); the checklist says so.
- Expo-era builds may exist in App Store Connect or Play; build numbers must exceed them.
- iPhone landscape: lock iPhone to portrait (matching Android), or verify the layouts. Default: portrait on
  iPhone, iPad unchanged.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Android release build MUST be signed with an upload key configured outside the repo (env or a gitignored properties file). It MUST fail clearly when the key is missing, and MUST NOT fall back to debug.
- **FR-002**: Every native library in the bundle MUST be 16 KB page-aligned. JNA MUST be ≥ 5.17.
- **FR-003**: Release builds MUST ignore debug launch extras (`vela.startDestination`, fixture galleries).
- **FR-004**: An external `velawallet://open` link MUST ask before loading, MUST accept `https` only, and the EIP-1193 provider MUST be injected only into secure origins. This applies on Android and iOS.
- **FR-005**: Background tracking MUST NOT crash on API 29–30.
- **FR-006**: iOS MUST archive and export for the App Store without entitlement errors. Unused entitlements are removed.
- **FR-007**: The export-compliance comment MUST state the encryption actually used.
- **FR-008**: The iPhone MUST be portrait-only, or every screen verified in landscape.
- **FR-009**: `NSLocalNetworkUsageDescription` MUST be present if local networking is used.
- **FR-010**: The erase screen MUST explain deletion truthfully, and a public deletion page MUST exist (source in the repo; the owner deploys it).
- **FR-011**: The review notes, listing copy and privacy answers MUST match the code and the claim ledger.
- **FR-012**: Version and build numbers MUST be monotonic, with a documented bump step.
- **FR-013**: The owner's console steps for both stores MUST be in one checklist, in the order they must happen.

## Success Criteria *(mandatory)*

- **SC-001**: A signed release AAB and an exportable iOS archive are produced from the documented steps on this machine.
- **SC-002**: 0 native libraries fail the 16 KB alignment check.
- **SC-003**: 0 blockers from `audit.md` remain in code. Every owner action is in the checklist.
- **SC-004**: The release build passes a smoke run on the Xiaomi: launch, create/sign-in screens, Home, Send form, Receive and Settings.

## Assumptions

- The upload keystore is `~/.android/vela-release.keystore`, alias `vela-release` (to be confirmed against `assetlinks.json`'s `A3:8E…`). Its passwords are the owner's and never committed.
- The site pages are built and committed; deploying getvela.app is the owner's.
- Production-only items (R8, localized InfoPlist strings, all-locale listing copy) wait for after the test tracks.
