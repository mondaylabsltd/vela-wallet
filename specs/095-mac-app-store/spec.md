# Feature Specification: 095 — Vela Wallet for Mac on the Mac App Store (TestFlight first)

**Feature Branch**: `095-mac-app-store` | **Created**: 2026-10-02 | **Status**: Implemented (owner steps open)
**Input**: make the macOS desktop app (`app-desktop/vela-wallet`, Rust + gpui +
wry/WKWebView) uploadable to the Mac App Store as a TestFlight build, with a
beta that feels close to stable. A read-only audit of `main` @ `ec033f231`
listed the blockers and the should-fixes this spec answers.

**Owner decisions (2026-10-02)**
1. The Mac app joins the existing iOS record as a **Universal Purchase**:
   bundle id `app.getvela.VelaWallet` stays.
2. The owner creates the **Mac App Store Connect** provisioning profile; the
   build takes it as a single input. Until then the existing development
   profile validates the sandboxed build.
3. Export compliance: **`ITSAppUsesNonExemptEncryption` = false** (standard
   HTTPS and authentication only); the rationale is recorded in the store doc.
4. Release and store builds expose **no developer features**.

## User Scenarios & Testing

### User Story 1 — The owner produces a package App Store Connect accepts (P1)

One command turns the source into a signed, sandboxed, universal `.pkg` and has
Apple validate it, given the MAS profile. It refuses — in words, before the
compile — a wrong profile, a missing certificate or missing credentials.

**Independent Test**: `build-macos-mas.sh` with the MAS profile ends in
`altool --validate-app` success; with the development profile (`--dev`) it ends
in a sandboxed bundle whose signature and entitlements verify.

**Acceptance Scenarios**
1. **Given** the MAS profile and an API key, **When** the script runs, **Then**
   the `.pkg` is signed by the installer identity and validates; nothing is
   uploaded.
2. **Given** the development profile, **When** `--dev` runs, **Then** the bundle
   is signed by the certificate the profile names, its entitlements equal
   `entitlements-mas.plist`, and it runs sandboxed on this Mac.
3. **Given** any release binary, **When** `check-store-binary.sh` runs, **Then**
   it finds no private WindowServer import, none of gpui's three private
   selectors and no developer switch name (and it fails on `main`'s binary).

### User Story 2 — Sandboxed, the app still does everything (P1)

A tester who installs from TestFlight can sign in, keep their state, use every
file panel, the camera, Bluetooth, a USB key, the system proxy and the browser.

**Independent Test**: the development-signed sandboxed bundle, launched from
`/Applications`, walked through each subsystem (results.md §Sandbox).

**Acceptance Scenarios**
1. **Given** a sandboxed launch, **When** the app writes its state and is
   relaunched, **Then** the state is there (in the container).
2. **Given** a save panel, **When** it opens, **Then** it starts in Downloads,
   not in the container.
3. **Given** a tester who used the `.dmg`, **When** the App Store copy first
   launches, **Then** their wallet list moves into the container.

### User Story 3 — A web page behaves as in any browser (P2)

**Acceptance Scenarios**
1. **Given** a dApp page, **When** a person clicks a `target=_blank` link or a
   button that calls `window.open`, **Then** the address opens in a new tab; a
   timer's `window.open` opens nothing.
2. **Given** a `mailto:`/`tel:` link the person clicks in the page's own
   document, **Then** it goes to the system's app; the same address from a
   script or a cross-origin frame does not.
3. **Given** an `http` page on the internet or LAN, **Then** it loads, without
   the wallet.
4. **Given** a field in a web page, **Then** ⌘C/⌘V/⌘X/⌘A/⌘Z work (Edit menu).

### User Story 4 — Nothing a reviewer would flag (P1)

1. A release build ignores every `VELA_*` developer switch.
2. Settings → About links the privacy policy, terms and support (5.1.1(i)) —
   on every shell.
3. The menu bar has About and Edit, in the app's language.
4. The store documents (listing, review notes, TestFlight text, privacy
   evidence, export-compliance rationale) are true and grounded in code.

### Edge Cases

- A Mac on macOS 11: excluded (`LSMinimumSystemVersion` 12.0; platform
  passkeys need 12).
- A `.dmg` copy on the same Mac after migration: starts signed out; signing
  in brings the wallet back (contacts/settings stay with the App Store copy).
- The development sandbox run on a developer's Mac must not move their real
  wallet: the `--dev` bundle carries no migration file.

## Requirements

- **FR-001** `scripts/build-macos-mas.sh`: profile as the one input; identities
  derived; universal release build; `CFBundleVersion` = commit count;
  `LSMinimumSystemVersion` 12.0; `ITSAppUsesNonExemptEncryption` false;
  privacy manifest; migration plist (store only); embedded profile;
  `xattr -cr`; `codesign --options runtime` without `--deep`; `productbuild`;
  `altool --validate-app`; never uploads; `--dev` for a sandboxed development
  build.
- **FR-002** `entitlements-mas.plist`: sandbox, app/team identifiers,
  associated domains, network client, user-selected files, camera, Bluetooth,
  USB, smart card — nothing more.
- **FR-003** No private API in the binary: vendored `gpui_macos` without
  `CGS*` and the three private selectors; `check-store-binary.sh` in CI.
- **FR-004** Developer switches compile out of release builds (one gate:
  debug, test or `dev-fixtures`).
- **FR-005** Save panels start in `$HOME/Downloads`.
- **FR-006** `container-migration.plist` for the `.dmg`-era state folder.
- **FR-007** About links privacy/terms/support on desktop, iOS, Android, web.
- **FR-008** macOS menu bar: About, Edit (responder-chain clipboard, Undo/Redo
  forwarded), View, Window; localized from the corpus.
- **FR-009** macOS browser: new windows → tabs (gesture enforced by WebKit);
  `mailto:`/`tel:` → system on a person's link in the top document.
- **FR-010** `NSAllowsArbitraryLoadsInWebContent` + `NSAllowsLocalNetworking`
  (as iOS).
- **FR-011** `docs/store-submission/mac-app-store.md` and desktop rows in
  `privacy-evidence.md`.

## Success Criteria

- **SC-001** `check-store-binary.sh` passes on the universal release binary and
  fails on `main`'s.
- **SC-002** The `--dev` bundle runs sandboxed; every subsystem in US2 works
  (or the denial is recorded).
- **SC-003** Desktop, core, web, iOS and Android suites green.
- **SC-004** With the owner's MAS profile, the script's `.pkg` validates
  (owner step).
