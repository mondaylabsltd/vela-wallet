# Feature Specification: 087 — Mobile beta device pass (Android + iOS)

**Feature Branch**: `087-mobile-beta-pass` (docs) · fixes ride `fix/087-*` branches | **Created**: 2026-10-01 | **Status**: In progress
**Input**: tomorrow (2026-10-02) the owner submits the Android app to a Google Play testing track and the
iOS app to TestFlight. "This is effectively a beta — users should get an experience close to stable."
Test every feature and every UI detail on the connected Xiaomi (Android) and iPhone 11 (iOS 26.5.2), via
source reading, unit tests, device runs and screenshot comparison. Fix what falls short.

Program: 086 (issues) · **087 (this)** · 088 (store readiness) · 089 (Chrome extension pass).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Every everyday flow works on both phones (Priority: P1)

A beta tester can complete each of these on Android and iPhone without a dead end, a wrong state, or a
confusing word:
- create a wallet, sign in, and switch accounts;
- see the balance and assets;
- receive (QR, save image, explorer);
- send (single, multi-recipient, change token, fee coin, speed);
- scan;
- see activity and its details;
- manage contacts (add, edit, import, export);
- use Explore and the dApp browser (connect, sign, send, refuse dangerous requests);
- change settings (language, text size, theme, region formats, keys and backup, feedback, sign out,
  erase).

**Independent Test**: the device matrix in `matrix.md`, each cell ✅/❌ with a screenshot. Real sends use
the parallel space's golden Safe on Gnosis (dust). The owner's own wallet is only read, never signed with.

### User Story 2 — An unstable network never leaves a wrong or frozen screen (Priority: P1)

Slow or unreachable RPCs, a lost relay reply, or a failed page load always show:
- a true state (pending, not confirmed yet, failed, unknown);
- a plain sentence;
- a way forward.

A record is never pending forever, and an internal id is never shown as a hash.

### User Story 3 — UI details hold at every size and in every language (Priority: P2)

Every screen holds at the default and the largest text size, and in zh / en and the longest locale (ru):
- no clipped or mid-word-wrapped labels;
- no test ids read aloud by screen readers;
- the right biometric word for the device (Face ID on a Face ID iPhone).

## Requirements *(mandatory)*

- **FR-001**: Each finding in `findings.md` is fixed on every shell that has it, or recorded with the owner's decision.
- **FR-002**: A dApp record that has neither an op hash nor a tx hash MUST NOT stay "pending" indefinitely, and MUST NOT display its record id as a hash.
- **FR-003**: Onboarding method subtitles MUST name the authenticator the device actually has, and the sign-in sheet MUST NOT say "create".
- **FR-004**: The notification permission MUST be asked at most once, in context.
- **FR-005**: Every fix comes with a test where one is possible, and device evidence.

## Success Criteria *(mandatory)*

- **SC-001**: 0 open S1 findings and 0 open S2 findings without an owner decision, at submission time.
- **SC-002**: The device matrix is complete on both phones.
- **SC-003**: Full suites green on the integration build.
