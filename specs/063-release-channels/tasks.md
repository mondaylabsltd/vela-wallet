# 063 tasks

`[X]` done and verified as `results.md` records · `[~]` done, verification needs the founder ·
`[ ]` waits on the founder (N-numbers are spec §6)

## Phase 1 — phones leave GitHub Releases (FR-001)

- [X] T001 `android-package.yml`: remove the `release` job; header says why the workflow stays; checksums travel inside the artifact
- [X] T002 `ios-package.yml`: the same
- [X] T003 Both: the tag comment teaches one-tag-per-push

## Phase 2 — a macOS package people can open (FR-002, FR-003, FR-004)

- [X] T010 `build-macos-app.sh --distribution`: secure timestamp, profile mandatory, image signed; preflight before the build
- [X] T011 `build-macos-app.sh --notarize`: app then image, stapled, `spctl` asked Gatekeeper's question; Apple's log printed on refusal; checksums after stapling
- [X] T012 `Info.plist.in`: `NSCameraUsageDescription`
- [X] T013 `entitlements-signed.plist`: `com.apple.security.device.camera`
- [X] T014 Rehearse T010/T012/T013 locally against a real release build and the real Developer ID identity
- [X] T015 `desktop-macos-packages.yml`: `publishable` decided first; throwaway keychain; identity by SHA-1; provisioning profile validated in words; notary credentials stored and thereby checked before the build
- [X] T016 The gate: `release` job runs only when `publishable == 'true'`; otherwise a job summary and a tag-time warning
- [X] T017 Environments by expression: `release` on `desktop-v*` tags, `build-check` elsewhere
- [X] T018 Validate the profile checks against the real profiles on the founder's Mac
- [~] T019 The negative path in CI: dispatch on the branch → builds, verifies, "NOT published" summary, `release` skipped
- [ ] T020 **N1–N4** credentials and the `release` environment (`quickstart.md` §1–5)
- [ ] T021 **N8** the first notarized dmg, downloaded through a browser: opens; "This device" makes a wallet; the scanner opens the camera
- [ ] T022 The first `desktop-v*` tag after T020 attaches three dmgs

## Phase 3 — what is published says how to install it (FR-005)

- [X] T030 `packaging/release-notes.md` (desktop: SmartScreen, macOS present-or-absent, Linux per format, phones are elsewhere)
- [X] T031 `extension/release-notes.md` (unzip, developer mode, Load unpacked, updating)
- [X] T032 All four release jobs: sparse checkout of the notes, `--notes-file` ahead of `--generate-notes`
- [X] T033 `extension/build.mjs`: the notes file is not copied into the package

## Phase 4 — the download page (FR-006)

- [X] T040 Per-card `onGithub`; the phone card's second route is the source, never Releases
- [X] T041 `en.ts`: `sourceCta`, `storeNote`
- [X] T042 Fifteen locales translated, then stamped; `i18n:status` clean; 479 unit tests; `svelte-check`; prettier
- [ ] T043 **N5** when a store listing is live: its link, the chip's wording, and the phone blurb ("in real-device testing") — all three change together
- [ ] T044 zh is the one *reviewed* locale: the two zh strings changed here want the founder's eye

## Phase 5 — the record (FR-007, FR-008)

- [X] T050 Runbook: the policy, the version rules, and the three traps (one tag per push; never delete-and-repush a released tag; a green PR says nothing about packaging)
- [X] T051 Root README: "Where to get it" and "A phone app you built yourself"
- [X] T052 Desktop README: the three signing modes, and which one may be published

## Phase 6 — the existing releases (FR-009) — destructive, waits on the founder

- [ ] T060 **N8** delete releases and tags `android-v0.9.1`, `android-v0.9.2`, `ios-v0.9.1`, `ios-v0.9.2`
- [ ] T061 **N8** from `desktop-v0.9.1` and `desktop-v0.9.2` delete the three ad-hoc `.dmg` files and `SHA256SUMS-macos` each (eight assets); the Windows and Linux packages stay
- [ ] T062 Edit the two desktop releases' notes to the new install text, so the pages that remain say what is on them

## Not this spec — decisions the founder owns

- [ ] **N6** Is the Apple enrollment an organization? (Guideline 3.1.5(i); decides whether iOS has a store at all)
- [ ] **N7** `assetlinks.json`: is `A3:8E:36:FE:…` Google's Play signing key? Then remove the debug and EAS fingerprints from production
