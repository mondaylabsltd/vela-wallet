# Tasks: 087 — Mobile beta device pass

## Phase 1 — Device pass on main (US1, US2, US3)
- [x] T001 Android (Xiaomi alioth): Home, Assets, Receive, Send (real Gnosis dust send in the parallel space), Activity, Settings (language, text size, theme, region, advanced, feedback), Explore, dApp browser (connect, personal_sign, SafeTx refusal, unlimited-approval warning), scan via the photo picker, contacts import — findings F04–F25.
- [x] T002 iPhone 11 (iOS 26.5.2): Home, Settings, Receive, scan, Send via pay link, Explore, Manage groups, the trusted signer (with #318) — findings F01–F03, F18, F22.
- [ ] T003 Onboarding (create / sign-in) on an Android emulator and an iOS simulator, after the F01/F02 copy fix.

## Phase 2 — Fixes (each on its own `fix/087-*` branch, PR to main)
- [ ] T010 [US2] F04/F05: legacy pending records without a hash; no record id shown as a hash (`fix/087-activity-stale-pending`).
- [ ] T011 [US3] F01/F02: key-method subtitles per platform; no "create" in sign-in (`fix/087-key-method-copy`).
- [ ] T012 [US3] F03: iOS empty-assets state (`fix/087-ios-empty-assets`).
- [ ] T013 [US1] F14: notification permission asked once (`fix/087-notification-ask-once`).
- [ ] T014 [US3] F09/F13/F22: accessibility labels (test ids out of labels; the show-QR label) (`fix/087-a11y-labels`).
- [ ] T015 [US3] F07/F11/F15: Chainlist chip wrap, blank amount cell, refused-loopback wording (`fix/087-small-ui`).
- [ ] T016 [US1] Batch-recipient CSV decoding through the core (`fix/087-batch-import-encoding`, on top of #350).
- [ ] T017 [US2] F21: Android browser chrome hidden during a slow first load (`fix/087-browser-chrome-slow-load`).
- [ ] T018 [US2] F08: the Home RPC notice only when an unreachable network holds the person's assets (owner decision).

## Phase 3 — Re-verify
- [ ] T020 Integration build (main + every 086/087/088 branch) on both phones; replay each finding.
- [ ] T021 `results.md`: per finding, the PR, the evidence and the status.
